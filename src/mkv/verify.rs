//! Post-edit verification using mkvextract and mkvmerge.

use std::path::Path;

use crate::model::{MkvAttachment, TargetInfo};
use crate::tags::{TagDocument, TagPlan};

use super::identify;
use super::plan::TrackFlags;
use super::tools::ToolSet;

#[derive(Clone, Debug, Default)]
pub struct VerifyReport {
    pub skipped: bool,
    pub tags_ok: bool,
    pub attachment_ok: bool,
    pub flags_ok: bool,
    pub warnings: Vec<String>,
    pub details: String,
}

impl VerifyReport {
    pub fn ok(&self) -> bool {
        self.tags_ok && self.attachment_ok && self.flags_ok
    }

    pub fn summary(&self) -> String {
        if self.skipped {
            return "Verification skipped (mkvmerge/mkvextract unavailable).".to_owned();
        }
        let mut parts = Vec::new();
        parts.push(if self.tags_ok {
            "tags ok".to_owned()
        } else {
            "tags mismatch".to_owned()
        });
        parts.push(if self.attachment_ok {
            "cover ok".to_owned()
        } else {
            "cover mismatch".to_owned()
        });
        parts.push(if self.flags_ok {
            "flags ok".to_owned()
        } else {
            "flags mismatch".to_owned()
        });
        if !self.warnings.is_empty() {
            parts.push(format!("{} warning(s)", self.warnings.len()));
        }
        parts.join(" · ")
    }
}

pub fn verify_target(
    path: &Path,
    tools: &ToolSet,
    plan: &TagPlan,
    flags: &TrackFlags,
    cover_size: Option<usize>,
) -> VerifyReport {
    if tools.mkvmerge.is_none() || tools.mkvextract.is_none() {
        return VerifyReport {
            skipped: true,
            ..Default::default()
        };
    }

    let mut report = VerifyReport::default();
    let info = match identify::identify(path, tools) {
        Ok(info) => info,
        Err(error) => {
            report.warnings.push(error);
            report.details = "Could not re-read the target.".to_owned();
            return report;
        }
    };

    report.flags_ok = check_flags(&info, flags, &mut report.warnings);
    report.attachment_ok = check_attachment(&info, cover_size, &mut report.warnings);

    match identify::extract_tags(path, tools) {
        Ok(Some(document)) => {
            report.tags_ok = check_tags(&document, plan, &info, &mut report.warnings);
            report.details = "Tags re-extracted and compared.".to_owned();
        }
        Ok(None) => {
            report.tags_ok = false;
            report
                .warnings
                .push("No tags were found after applying.".to_owned());
            report.details = "mkvextract returned no tags.".to_owned();
        }
        Err(error) => {
            report.tags_ok = false;
            report.warnings.push(error);
            report.details = "Tag verification could not run.".to_owned();
        }
    }

    report
}

fn check_flags(info: &TargetInfo, flags: &TrackFlags, warnings: &mut Vec<String>) -> bool {
    let Some(track) = info.first_audio() else {
        warnings.push("No audio track was found after applying.".to_owned());
        return false;
    };
    let mut ok = true;
    if flags.set_default && !track.default {
        warnings.push("Audio track 1 is not marked default.".to_owned());
        ok = false;
    }
    if !flags.language.trim().is_empty()
        && !track.language.eq_ignore_ascii_case(flags.language.trim())
    {
        warnings.push(format!(
            "Audio track 1 language is '{}' instead of '{}'.",
            track.language,
            flags.language.trim()
        ));
        ok = false;
    }
    if !flags.name.trim().is_empty() {
        let name = track.name.as_deref().unwrap_or_default();
        if name != flags.name.trim() {
            warnings.push(format!(
                "Audio track name is '{}' instead of '{}'.",
                name,
                flags.name.trim()
            ));
            ok = false;
        }
    }
    ok
}

fn check_attachment(
    info: &TargetInfo,
    cover_size: Option<usize>,
    warnings: &mut Vec<String>,
) -> bool {
    let Some(expected_size) = cover_size else {
        return true;
    };
    let Some(attachment) = best_cover(&info.attachments) else {
        warnings.push("No cover attachment was found after applying.".to_owned());
        return false;
    };
    if attachment.size != expected_size as u64 {
        warnings.push(format!(
            "Cover attachment '{}' is {} bytes, expected {}.",
            attachment.name, attachment.size, expected_size
        ));
        return false;
    }
    true
}

fn best_cover(attachments: &[MkvAttachment]) -> Option<&MkvAttachment> {
    attachments
        .iter()
        .find(|attachment| attachment.looks_like_cover())
        .or_else(|| {
            attachments
                .iter()
                .find(|attachment| attachment.mime_type.starts_with("image/"))
        })
}

fn check_tags(
    document: &TagDocument,
    plan: &TagPlan,
    info: &TargetInfo,
    warnings: &mut Vec<String>,
) -> bool {
    let mut ok = true;
    let album = document.album();
    if album.and_then(|tag| tag.value("TITLE")) != Some(plan.album.title.as_str()) {
        warnings.push("Album TITLE does not match the applied value.".to_owned());
        ok = false;
    }
    let uid = info.first_audio().map(|track| track.uid).or(plan.track_uid);
    let track = document.track(uid);
    if track.and_then(|tag| tag.value("TITLE")) != Some(plan.track.title.as_str()) {
        warnings.push("Track TITLE does not match the applied value.".to_owned());
        ok = false;
    }
    if let Some(number) = plan.track.track_number {
        let expected = number.to_string();
        if track.and_then(|tag| tag.value("PART_NUMBER")) != Some(expected.as_str()) {
            warnings.push("Track PART_NUMBER does not match the applied value.".to_owned());
            ok = false;
        }
    }
    ok
}
