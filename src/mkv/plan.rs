//! Pure mkvpropedit argument planning.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::model::{CoverArt, MkvAttachment, TargetInfo};

#[derive(Clone, Debug)]
pub struct TrackFlags {
    pub set_default: bool,
    pub language: String,
    pub name: String,
}

impl Default for TrackFlags {
    fn default() -> Self {
        Self {
            set_default: true,
            language: "eng".to_owned(),
            name: crate::config::DEFAULT_TRACK_NAME.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AttachmentEdit {
    pub deletes: Vec<String>,
    pub action: AttachmentAction,
}

#[derive(Clone, Debug, Default)]
pub enum AttachmentAction {
    #[default]
    None,
    Add {
        path: PathBuf,
        name: String,
        mime: String,
        description: String,
    },
    Replace {
        selector: String,
        path: PathBuf,
        name: String,
        mime: String,
        description: String,
    },
}

#[derive(Clone, Debug)]
pub struct ApplyPlan {
    pub target: PathBuf,
    pub tags_xml: Option<PathBuf>,
    pub album_tags_xml: Option<PathBuf>,
    pub track_tags_xml: Option<PathBuf>,
    pub track_uid: Option<u64>,
    pub flags: TrackFlags,
    pub audio_track_count: usize,
    pub attachment: AttachmentEdit,
}

pub fn build_args(plan: &ApplyPlan) -> Vec<OsString> {
    let mut args = Vec::new();
    args.push(plan.target.as_os_str().to_os_string());

    if plan.track_uid.filter(|uid| *uid > 0).is_some() {
        if let Some(tags_xml) = &plan.tags_xml {
            let mut tags = OsString::from("all:");
            tags.push(tags_xml.as_os_str());
            args.push(OsString::from("--tags"));
            args.push(tags);
        }
    } else if let (Some(album), Some(track)) = (&plan.album_tags_xml, &plan.track_tags_xml) {
        let mut album_selector = OsString::from("global:");
        album_selector.push(album.as_os_str());
        args.push(OsString::from("--tags"));
        args.push(album_selector);

        let mut track_selector = OsString::from("track:a1:");
        track_selector.push(track.as_os_str());
        args.push(OsString::from("--tags"));
        args.push(track_selector);
    } else if let Some(tags_xml) = &plan.tags_xml {
        let mut tags = OsString::from("all:");
        tags.push(tags_xml.as_os_str());
        args.push(OsString::from("--tags"));
        args.push(tags);
    }

    args.push(OsString::from("--edit"));
    args.push(OsString::from("track:a1"));
    args.push(OsString::from("--set"));
    args.push(OsString::from(format!(
        "flag-default={}",
        if plan.flags.set_default { 1 } else { 0 }
    )));
    if !plan.flags.language.trim().is_empty() {
        args.push(OsString::from("--set"));
        args.push(OsString::from(format!(
            "language={}",
            plan.flags.language.trim()
        )));
    }
    if !plan.flags.name.trim().is_empty() {
        args.push(OsString::from("--set"));
        args.push(OsString::from(format!("name={}", plan.flags.name.trim())));
    }

    if plan.flags.set_default {
        for index in 2..=plan.audio_track_count {
            args.push(OsString::from("--edit"));
            args.push(OsString::from(format!("track:a{index}")));
            args.push(OsString::from("--set"));
            args.push(OsString::from("flag-default=0"));
        }
    }

    for selector in &plan.attachment.deletes {
        args.push(OsString::from("--delete-attachment"));
        args.push(OsString::from(selector));
    }

    match &plan.attachment.action {
        AttachmentAction::None => {}
        AttachmentAction::Add {
            path,
            name,
            mime,
            description,
        } => {
            push_attachment_metadata(&mut args, name, mime, description);
            args.push(OsString::from("--add-attachment"));
            args.push(path.as_os_str().to_os_string());
        }
        AttachmentAction::Replace {
            selector,
            path,
            name,
            mime,
            description,
        } => {
            push_attachment_metadata(&mut args, name, mime, description);
            let mut value = OsString::from(selector);
            value.push(":");
            value.push(path.as_os_str());
            args.push(OsString::from("--replace-attachment"));
            args.push(value);
        }
    }

    args
}

fn push_attachment_metadata(args: &mut Vec<OsString>, name: &str, mime: &str, description: &str) {
    args.push(OsString::from("--attachment-name"));
    args.push(OsString::from(name));
    args.push(OsString::from("--attachment-description"));
    args.push(OsString::from(description));
    args.push(OsString::from("--attachment-mime-type"));
    args.push(OsString::from(mime));
}

pub fn plan_attachment(
    info: &TargetInfo,
    cover: Option<&CoverArt>,
    staged_path: Option<&Path>,
) -> AttachmentEdit {
    let (Some(cover), Some(staged_path)) = (cover, staged_path) else {
        return AttachmentEdit::default();
    };

    let name = cover.filename();
    let mime = cover.mime.as_str().to_owned();
    let description = "Front cover".to_owned();

    let covers: Vec<&MkvAttachment> = info
        .attachments
        .iter()
        .filter(|attachment| attachment.looks_like_cover())
        .collect();

    if covers.is_empty() {
        return AttachmentEdit {
            deletes: Vec::new(),
            action: AttachmentAction::Add {
                path: staged_path.to_path_buf(),
                name,
                mime,
                description,
            },
        };
    }

    let first = covers[0];
    let deletes = covers
        .iter()
        .skip(1)
        .map(|attachment| attachment_selector(attachment))
        .collect();

    AttachmentEdit {
        deletes,
        action: AttachmentAction::Replace {
            selector: attachment_selector(first),
            path: staged_path.to_path_buf(),
            name,
            mime,
            description,
        },
    }
}

pub fn attachment_selector(attachment: &MkvAttachment) -> String {
    if attachment.uid > 0 {
        format!("={}", attachment.uid)
    } else if !attachment.name.is_empty() {
        format!("name:{}", attachment.name.replace(':', "\\c"))
    } else {
        attachment.id.to_string()
    }
}

pub fn describe_plan(plan: &ApplyPlan) -> String {
    let mut lines = vec![
        format!("Target: {}", plan.target.display()),
        format!(
            "Tags: {}",
            plan.tags_xml
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "split global/track files".to_owned())
        ),
        format!(
            "Audio track 1: default={}, language={}, name={}",
            plan.flags.set_default, plan.flags.language, plan.flags.name
        ),
    ];
    match &plan.attachment.action {
        AttachmentAction::None => lines.push("Cover: unchanged".to_owned()),
        AttachmentAction::Add { path, name, .. } => {
            lines.push(format!("Cover: add {} from {}", name, path.display()))
        }
        AttachmentAction::Replace {
            selector,
            name,
            path,
            ..
        } => lines.push(format!(
            "Cover: replace {} with {} from {}",
            selector,
            name,
            path.display()
        )),
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_core_args() {
        let plan = ApplyPlan {
            target: PathBuf::from("/music/01 - Song.mka"),
            tags_xml: Some(PathBuf::from("/cache/tags.xml")),
            album_tags_xml: None,
            track_tags_xml: None,
            track_uid: Some(42),
            flags: TrackFlags {
                set_default: true,
                language: "eng".to_owned(),
                name: "5.1 Surround".to_owned(),
            },
            audio_track_count: 2,
            attachment: AttachmentEdit::default(),
        };
        let args: Vec<String> = build_args(&plan)
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "/music/01 - Song.mka");
        assert!(args.contains(&"--tags".to_owned()));
        assert!(args.contains(&"all:/cache/tags.xml".to_owned()));
        assert!(args.contains(&"flag-default=1".to_owned()));
        assert!(args.contains(&"language=eng".to_owned()));
        assert!(args.contains(&"name=5.1 Surround".to_owned()));
        assert!(args.contains(&"track:a2".to_owned()));
        assert!(args.contains(&"flag-default=0".to_owned()));
    }

    #[test]
    fn builds_split_args_when_track_uid_is_missing() {
        let plan = ApplyPlan {
            target: PathBuf::from("/music/01 - Song.mka"),
            tags_xml: Some(PathBuf::from("/cache/tags.xml")),
            album_tags_xml: Some(PathBuf::from("/cache/album.xml")),
            track_tags_xml: Some(PathBuf::from("/cache/track.xml")),
            track_uid: None,
            flags: TrackFlags::default(),
            audio_track_count: 1,
            attachment: AttachmentEdit::default(),
        };
        let args: Vec<String> = build_args(&plan)
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"global:/cache/album.xml".to_owned()));
        assert!(args.contains(&"track:a1:/cache/track.xml".to_owned()));
        assert!(!args.contains(&"all:/cache/tags.xml".to_owned()));
    }

    #[test]
    fn detects_cover_by_name_even_with_binary_mime() {
        let attachment = MkvAttachment {
            id: 0,
            uid: 0,
            name: "cover.jpg".to_owned(),
            mime_type: "application/octet-stream".to_owned(),
            description: None,
            size: 10,
        };
        assert!(attachment.looks_like_cover());
        assert_eq!(attachment_selector(&attachment), "name:cover.jpg");
    }

    #[test]
    fn escapes_colons_in_attachment_names() {
        let attachment = MkvAttachment {
            id: 0,
            uid: 0,
            name: "cover:front.jpg".to_owned(),
            mime_type: String::new(),
            description: None,
            size: 10,
        };
        assert_eq!(attachment_selector(&attachment), "name:cover\\cfront.jpg");
    }

    #[test]
    fn plans_add_when_no_cover_exists() {
        let info = TargetInfo::default();
        let cover = CoverArt {
            bytes: vec![0xFF, 0xD8, 0xFF],
            mime: crate::model::CoverMime::Jpeg,
            source: crate::model::CoverSource::Embedded,
        };
        let edit = plan_attachment(&info, Some(&cover), Some(Path::new("/cache/cover.jpg")));
        assert!(matches!(edit.action, AttachmentAction::Add { .. }));
        assert!(edit.deletes.is_empty());
    }

    #[test]
    fn plans_replace_and_delete_duplicates() {
        let info = TargetInfo {
            attachments: vec![
                MkvAttachment {
                    id: 0,
                    uid: 11,
                    name: "cover.jpg".to_owned(),
                    mime_type: "image/jpeg".to_owned(),
                    description: Some("Front cover".to_owned()),
                    size: 10,
                },
                MkvAttachment {
                    id: 1,
                    uid: 12,
                    name: "front.png".to_owned(),
                    mime_type: "image/png".to_owned(),
                    description: None,
                    size: 10,
                },
            ],
            ..Default::default()
        };
        let cover = CoverArt {
            bytes: vec![0x89, b'P', b'N', b'G'],
            mime: crate::model::CoverMime::Png,
            source: crate::model::CoverSource::Embedded,
        };
        let edit = plan_attachment(&info, Some(&cover), Some(Path::new("/cache/cover.png")));
        assert_eq!(edit.deletes, vec!["=12".to_owned()]);
        match edit.action {
            AttachmentAction::Replace { selector, .. } => assert_eq!(selector, "=11"),
            _ => panic!("expected replace"),
        }
    }
}
