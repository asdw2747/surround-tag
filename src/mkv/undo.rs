use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UndoEntry {
    pub original: PathBuf,
    pub renamed: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UndoManifest {
    pub version: u32,
    pub created_at_unix: u64,
    pub entries: Vec<UndoEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct UndoReport {
    pub restored: usize,
    pub failed: Vec<(PathBuf, String)>,
}

pub fn save_manifest(entries: Vec<UndoEntry>) -> Result<(), String> {
    save_manifest_at(&config::undo_path(), entries)
}

fn save_manifest_at(path: &Path, entries: Vec<UndoEntry>) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }
    let manifest = UndoManifest {
        version: 1,
        created_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default(),
        entries,
    };
    let json = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("Could not serialize undo manifest: {error}"))?;
    config::atomic_write(path, &json)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))
}

pub fn has_manifest() -> bool {
    config::undo_path().is_file()
}

pub fn undo_last() -> Result<UndoReport, String> {
    undo_last_at(&config::undo_path())
}

fn undo_last_at(path: &Path) -> Result<UndoReport, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let manifest: UndoManifest = serde_json::from_str(&text)
        .map_err(|error| format!("Undo manifest {} is invalid: {error}", path.display()))?;
    if manifest.entries.is_empty() {
        return Err("There is no rename batch to undo.".to_owned());
    }

    let mut report = UndoReport::default();
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();

    for entry in &manifest.entries {
        if entry.original == entry.renamed {
            continue;
        }
        match fs::metadata(&entry.renamed) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => {
                report
                    .failed
                    .push((entry.renamed.clone(), "not a file".to_owned()));
                continue;
            }
            Err(error) => {
                report.failed.push((
                    entry.renamed.clone(),
                    format!("missing renamed file: {error}"),
                ));
                continue;
            }
        }
        if entry.original.exists() {
            report.failed.push((
                entry.original.clone(),
                "original path is occupied".to_owned(),
            ));
            continue;
        }
        let temp = temp_sibling(&entry.renamed, staged.len() as u64);
        match fs::rename(&entry.renamed, &temp) {
            Ok(()) => staged.push((entry.original.clone(), temp)),
            Err(error) => report
                .failed
                .push((entry.renamed.clone(), format!("could not stage: {error}"))),
        }
    }

    for (original, temp) in &staged {
        match fs::rename(temp, original) {
            Ok(()) => report.restored += 1,
            Err(error) => {
                report
                    .failed
                    .push((original.clone(), format!("could not restore: {error}")));
            }
        }
    }

    if report.failed.is_empty() {
        let _ = fs::remove_file(path);
    }

    Ok(report)
}

fn temp_sibling(path: &Path, counter: u64) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("file");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    parent.join(format!(".{stem}.surroundtag-undo-{stamp}-{counter}"))
}
