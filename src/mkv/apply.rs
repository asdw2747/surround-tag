//! High-level per-target apply pipeline: stage, edit, verify, rename.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::config;
use crate::model::{ApplyItemResult, ApplyState, CoverArt, FolderArtWrite, TargetInfo};
use crate::tags::{self, TagDocument, TagPlan, TagPolicy};

use super::plan::{self, ApplyPlan, TrackFlags};
use super::runner;
use super::tools::ToolSet;
use super::undo::UndoEntry;
use super::verify;

#[derive(Clone, Debug)]
pub struct ApplyJob {
    pub target: PathBuf,
    pub target_info: TargetInfo,
    pub existing_tags: Option<TagDocument>,
    pub tag_plan: TagPlan,
    pub cover: Option<CoverArt>,
    pub flags: TrackFlags,
    pub tag_policy: TagPolicy,
    pub rename_to: Option<PathBuf>,
    pub overwrite_rename: bool,
    pub keep_backup: bool,
    pub folder_art: Option<FolderArtWrite>,
}

#[derive(Clone, Debug)]
pub struct ApplyOutcome {
    pub result: ApplyItemResult,
    pub rename_entry: Option<UndoEntry>,
}

pub fn apply_job(job: &ApplyJob, tools: &ToolSet) -> ApplyOutcome {
    let started = Instant::now();
    let mut log = String::new();
    let mut warnings = Vec::new();

    if !tools.can_apply() {
        return failed(
            &job.target,
            "MKVToolNix mkvpropedit is not available.".to_owned(),
            log,
            warnings,
        );
    }

    if let Some(parent) = job.target.parent() {
        if !parent.exists() {
            return failed(
                &job.target,
                format!("Target folder does not exist: {}", parent.display()),
                log,
                warnings,
            );
        }
    }

    if let Some(rename_to) = &job.rename_to {
        if rename_to.exists() && rename_to != &job.target && !job.overwrite_rename {
            return failed(
                &job.target,
                format!("Rename destination already exists: {}", rename_to.display()),
                log,
                warnings,
            );
        }
    }

    let job_dir = match config::unique_job_dir() {
        Ok(dir) => dir,
        Err(error) => return failed(&job.target, error, log, warnings),
    };

    if job.keep_backup {
        if let Err(error) = backup_target(&job.target) {
            return failed(&job.target, error, log, warnings);
        }
    }

    let staged_cover = match job.cover.as_ref() {
        Some(cover) => {
            let path = job_dir.join(cover.filename());
            if let Err(error) = fs::write(&path, &cover.bytes) {
                return failed(
                    &job.target,
                    format!("Could not stage cover art: {error}"),
                    log,
                    warnings,
                );
            }
            Some(path)
        }
        None => None,
    };

    if let Some(existing) = &job.existing_tags {
        let original = tags::render(existing);
        let _ = fs::write(job_dir.join("original-tags.xml"), original);
    }

    let document = tags::plan_document(job.existing_tags.as_ref(), &job.tag_plan, job.tag_policy);
    let tags_xml = job_dir.join("tags.xml");
    if let Err(error) = fs::write(&tags_xml, tags::render(&document)) {
        return failed(
            &job.target,
            format!("Could not write staged tag XML: {error}"),
            log,
            warnings,
        );
    }

    let attachment = plan::plan_attachment(
        &job.target_info,
        job.cover.as_ref(),
        staged_cover.as_deref(),
    );

    let track_uid = job.tag_plan.track_uid.filter(|uid| *uid > 0);
    let mut album_tags_xml = None;
    let mut track_tags_xml = None;
    if track_uid.is_none() {
        if let Some(album) = document.album().cloned() {
            let path = job_dir.join("album-tags.xml");
            if let Err(error) = fs::write(&path, tags::render(&TagDocument { tags: vec![album] })) {
                return failed(
                    &job.target,
                    format!("Could not write staged album tag XML: {error}"),
                    log,
                    warnings,
                );
            }
            album_tags_xml = Some(path);
        }
        if let Some(track) = document
            .tags
            .iter()
            .find(|tag| tag.target_type_value == 30 && tag.track_uid.is_none())
            .cloned()
            .or_else(|| document.track(job.tag_plan.track_uid).cloned())
        {
            let path = job_dir.join("track-tags.xml");
            if let Err(error) = fs::write(&path, tags::render(&TagDocument { tags: vec![track] })) {
                return failed(
                    &job.target,
                    format!("Could not write staged track tag XML: {error}"),
                    log,
                    warnings,
                );
            }
            track_tags_xml = Some(path);
        }
    }

    let apply_plan = ApplyPlan {
        target: job.target.clone(),
        tags_xml: Some(tags_xml.clone()),
        album_tags_xml,
        track_tags_xml,
        track_uid,
        flags: job.flags.clone(),
        audio_track_count: job.target_info.audio_tracks().len().max(1),
        attachment,
    };

    let args = plan::build_args(&apply_plan);
    log.push_str("Command:\n");
    log.push_str(&format!(
        "{} {}\n\n",
        tools
            .mkvpropedit
            .as_ref()
            .map(|tool| tool.display())
            .unwrap_or_default(),
        args.iter()
            .map(|arg| quote_arg(arg))
            .collect::<Vec<_>>()
            .join(" ")
    ));

    let Some(tool) = &tools.mkvpropedit else {
        return failed(
            &job.target,
            "mkvpropedit is not available.".to_owned(),
            log,
            warnings,
        );
    };

    let output = match runner::run(tool, &args, Duration::from_secs(600)) {
        Ok(output) => output,
        Err(error) => {
            return failed(&job.target, error, log, warnings);
        }
    };

    log.push_str(&output.combined());
    log.push('\n');

    if output.timed_out {
        return failed(
            &job.target,
            "mkvpropedit timed out after 10 minutes.".to_owned(),
            log,
            warnings,
        );
    }

    let exit_code = output.status.unwrap_or(-1);
    if exit_code != 0 && exit_code != 1 {
        let hint = mkv_error_hint(&output.combined())
            .map(|hint| format!(" {hint}"))
            .unwrap_or_default();
        return failed(
            &job.target,
            format!("mkvpropedit failed with exit code {exit_code}.{hint}"),
            log,
            warnings,
        );
    }
    if exit_code == 1 {
        warnings.push("mkvpropedit completed with warnings.".to_owned());
    }

    let report = verify::verify_target(
        &job.target,
        tools,
        &job.tag_plan,
        &job.flags,
        job.cover.as_ref().map(|cover| cover.bytes.len()),
    );
    log.push_str(&format!("\nVerification: {}\n", report.summary()));
    log.push_str(&report.details);
    log.push('\n');
    warnings.extend(report.warnings.clone());

    let verification_ok = report.skipped || report.ok();
    let mut renamed_to = None;
    let mut rename_entry = None;
    if let Some(rename_to) = &job.rename_to {
        if !verification_ok {
            warnings.push(
                "Rename skipped because verification reported metadata or stream-flag mismatches."
                    .to_owned(),
            );
        } else if rename_to != &job.target {
            if rename_to.exists() {
                if job.overwrite_rename {
                    if let Err(error) = fs::remove_file(rename_to) {
                        warnings.push(format!(
                            "Could not remove the existing rename destination: {error}"
                        ));
                    }
                } else {
                    warnings.push(format!(
                        "Rename skipped because the destination already exists: {}",
                        rename_to.display()
                    ));
                }
            }
            if !rename_to.exists() {
                if let Some(parent) = rename_to.parent() {
                    if let Err(error) = fs::create_dir_all(parent) {
                        warnings.push(format!(
                            "Tags were applied, but the rename folder could not be created: {error}"
                        ));
                    }
                }
                match fs::rename(&job.target, rename_to) {
                    Ok(()) => {
                        renamed_to = Some(rename_to.clone());
                        rename_entry = Some(UndoEntry {
                            original: job.target.clone(),
                            renamed: rename_to.clone(),
                        });
                        log.push_str(&format!("Renamed to {}\n", rename_to.display()));
                    }
                    Err(error) => warnings.push(format!(
                        "Tags were applied, but the file could not be renamed: {error}"
                    )),
                }
            }
        }
    }

    let folder_art = job.folder_art.as_ref().and_then(|write| {
        let mut actual = write.clone();
        if let Some(parent) = renamed_to
            .as_ref()
            .or(Some(&job.target))
            .and_then(|path| path.parent())
        {
            actual.directory = parent.to_path_buf();
        }
        write_folder_art(&actual, &mut warnings, &mut log)
    });

    let state = if !warnings.is_empty() || !verification_ok {
        ApplyState::Warning
    } else {
        ApplyState::Done
    };
    let message = if state == ApplyState::Done {
        format!("Applied in {:.1}s", started.elapsed().as_secs_f64())
    } else {
        format!(
            "Applied with {} warning(s) in {:.1}s",
            warnings.len(),
            started.elapsed().as_secs_f64()
        )
    };

    if state == ApplyState::Done {
        let _ = fs::remove_dir_all(&job_dir);
    } else {
        log.push_str(&format!("Job artifacts kept in {}\n", job_dir.display()));
    }

    ApplyOutcome {
        result: ApplyItemResult {
            target: job.target.clone(),
            renamed_to,
            state,
            message,
            warnings,
            log,
            folder_art,
        },
        rename_entry,
    }
}

fn failed(target: &Path, message: String, mut log: String, warnings: Vec<String>) -> ApplyOutcome {
    log.push_str(&message);
    log.push('\n');
    ApplyOutcome {
        result: ApplyItemResult {
            target: target.to_path_buf(),
            renamed_to: None,
            state: ApplyState::Failed,
            message,
            warnings,
            log,
            folder_art: None,
        },
        rename_entry: None,
    }
}

fn write_folder_art(
    write: &FolderArtWrite,
    warnings: &mut Vec<String>,
    log: &mut String,
) -> Option<PathBuf> {
    if write.directory.as_os_str().is_empty() {
        return None;
    }
    if let Err(error) = fs::create_dir_all(&write.directory) {
        warnings.push(format!(
            "Could not create folder-art directory {}: {error}",
            write.directory.display()
        ));
        return None;
    }

    let path = write.directory.join(&write.filename);
    if let Ok(existing) = fs::read(&path) {
        if existing == write.bytes {
            log.push_str(&format!("Folder art already current: {}\n", path.display()));
            return Some(path);
        }
        if let Err(error) = backup_folder_art(&path) {
            warnings.push(error);
        }
    }

    let opposite = if write.filename.ends_with(".jpg") {
        write.directory.join("folder.png")
    } else {
        write.directory.join("folder.jpg")
    };
    if opposite.exists() {
        warnings.push(format!(
            "Kodi folder art: {} also exists; Kodi may prefer it over {}.",
            opposite.display(),
            path.display()
        ));
    }

    let temp = path.with_extension(format!(
        "{}.surroundtag-tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("img")
    ));
    let result = fs::write(&temp, &write.bytes).and_then(|()| fs::rename(&temp, &path));
    if let Err(error) = result {
        let _ = fs::remove_file(&temp);
        if let Err(direct_error) = fs::write(&path, &write.bytes) {
            warnings.push(format!(
                "Could not write Kodi folder art {}: {error}; direct write also failed: {direct_error}",
                path.display()
            ));
            return None;
        }
    }
    log.push_str(&format!("Wrote Kodi folder art: {}\n", path.display()));
    Some(path)
}

fn backup_folder_art(path: &Path) -> Result<PathBuf, String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("folder.jpg");
    let backup_dir = parent.join(".surroundtag-backup");
    fs::create_dir_all(&backup_dir)
        .map_err(|error| format!("Could not create {}: {error}", backup_dir.display()))?;
    let backup = backup_dir.join(name);
    fs::copy(path, &backup).map_err(|error| {
        format!(
            "Could not back up existing folder art {} to {}: {error}",
            path.display(),
            backup.display()
        )
    })?;
    Ok(backup)
}

fn backup_target(target: &Path) -> Result<PathBuf, String> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("track.mka");
    let backup_dir = parent.join(".surroundtag-backup");
    fs::create_dir_all(&backup_dir)
        .map_err(|error| format!("Could not create {}: {error}", backup_dir.display()))?;
    let backup = backup_dir.join(name);
    fs::copy(target, &backup).map_err(|error| {
        format!(
            "Could not back up {} to {}: {error}",
            target.display(),
            backup.display()
        )
    })?;
    Ok(backup)
}

fn mkv_error_hint(log: &str) -> Option<&'static str> {
    let lower = log.to_ascii_lowercase();
    if lower.contains("open file error")
        || lower.contains("being used by another process")
        || lower.contains("sharing violation")
        || lower.contains("device or resource busy")
    {
        return Some(
            "Close the target in any player or editor, then retry. On SMB shares, make sure the file is not held open by another client.",
        );
    }
    if lower.contains("permission denied")
        || lower.contains("access is denied")
        || lower.contains("read-only")
    {
        return Some(
            "Check the file and folder permissions. SMB shares may require a writable mount and an account with write access.",
        );
    }
    if lower.contains("no space left") {
        return Some("Free up disk space and retry.");
    }
    None
}

fn quote_arg(arg: &std::ffi::OsStr) -> String {
    let value = arg.to_string_lossy();
    if value.contains(' ') {
        format!("\"{value}\"")
    } else {
        value.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_folder_art_and_backs_up_existing() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("album");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("folder.jpg"), b"old-art").unwrap();

        let write = FolderArtWrite {
            directory: folder.clone(),
            filename: "folder.jpg".to_owned(),
            bytes: b"new-art".to_vec(),
        };
        let mut warnings = Vec::new();
        let mut log = String::new();
        let path = write_folder_art(&write, &mut warnings, &mut log).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new-art");
        assert_eq!(
            std::fs::read(folder.join(".surroundtag-backup/folder.jpg")).unwrap(),
            b"old-art"
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn identical_folder_art_is_not_rewritten() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("album");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("folder.png"), b"same-art").unwrap();

        let write = FolderArtWrite {
            directory: folder.clone(),
            filename: "folder.png".to_owned(),
            bytes: b"same-art".to_vec(),
        };
        let mut warnings = Vec::new();
        let mut log = String::new();
        let path = write_folder_art(&write, &mut warnings, &mut log).unwrap();
        assert_eq!(path, folder.join("folder.png"));
        assert!(!folder.join(".surroundtag-backup").exists());
        assert!(warnings.is_empty());
    }

    #[test]
    fn classifies_locked_file_errors() {
        assert!(
            mkv_error_hint("The file could not be opened for reading: open file error.").is_some()
        );
        assert!(mkv_error_hint("Permission denied").is_some());
        assert!(mkv_error_hint("something else").is_none());
    }

    #[test]
    fn opposite_extension_produces_a_warning() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("album");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("folder.jpg"), b"old-jpg").unwrap();

        let write = FolderArtWrite {
            directory: folder.clone(),
            filename: "folder.png".to_owned(),
            bytes: b"new-png".to_vec(),
        };
        let mut warnings = Vec::new();
        let mut log = String::new();
        let path = write_folder_art(&write, &mut warnings, &mut log).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"new-png");
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("also exists")));
    }
}
