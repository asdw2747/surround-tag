//! Drop routing and folder scanning for stereo masters and .mka targets.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

const SOURCE_EXTENSIONS: &[&str] = &["flac", "mp3", "m4a", "wav", "ogg"];
const TARGET_EXTENSIONS: &[&str] = &["mka"];

#[derive(Clone, Debug, Default)]
pub struct ScanOutcome {
    pub sources: Vec<PathBuf>,
    pub targets: Vec<PathBuf>,
    pub notes: Vec<String>,
}

pub fn is_source_audio(path: &Path) -> bool {
    extension_in(path, SOURCE_EXTENSIONS)
}

pub fn is_target_mka(path: &Path) -> bool {
    extension_in(path, TARGET_EXTENSIONS)
}

fn extension_in(path: &Path, allowed: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            allowed
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
}

pub fn expand_paths(paths: &[PathBuf], recursive: bool) -> ScanOutcome {
    let mut outcome = ScanOutcome::default();
    let mut seen_sources = HashSet::new();
    let mut seen_targets = HashSet::new();

    for path in paths {
        if path.is_file() {
            classify_file(
                path.clone(),
                &mut outcome,
                &mut seen_sources,
                &mut seen_targets,
            );
        } else if path.is_dir() {
            let depth = if recursive { 8 } else { 1 };
            let mut files = Vec::new();
            for entry in WalkDir::new(path)
                .max_depth(depth)
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| {
                    !entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.starts_with('.'))
                })
                .filter_map(Result::ok)
            {
                if entry.file_type().is_file() {
                    files.push(entry.into_path());
                }
            }
            files.sort();
            for file in files {
                classify_file(file, &mut outcome, &mut seen_sources, &mut seen_targets);
            }
        } else {
            outcome
                .notes
                .push(format!("Skipped missing path: {}", path.display()));
        }
    }

    outcome.sources.sort();
    outcome.targets.sort();
    outcome
}

fn classify_file(
    path: PathBuf,
    outcome: &mut ScanOutcome,
    seen_sources: &mut HashSet<PathBuf>,
    seen_targets: &mut HashSet<PathBuf>,
) {
    let key = path.canonicalize().unwrap_or_else(|_| path.clone());
    if is_target_mka(&path) {
        if seen_targets.insert(key) {
            outcome.targets.push(path);
        }
    } else if is_source_audio(&path) {
        if seen_sources.insert(key) {
            outcome.sources.push(path);
        }
    } else {
        outcome
            .notes
            .push(format!("Skipped unsupported file: {}", path.display()));
    }
}

pub fn classify_paths(paths: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let outcome = expand_paths(paths, false);
    (outcome.sources, outcome.targets)
}
