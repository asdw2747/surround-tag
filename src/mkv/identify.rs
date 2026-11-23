//! `mkvmerge -J` introspection and `mkvextract tags` reading.

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use crate::model::{MkvAttachment, MkvTrack, TargetInfo};
use crate::tags::{self, TagDocument};

use super::runner;
use super::tools::ToolSet;

#[derive(Clone, Debug)]
pub struct TargetProbe {
    pub info: TargetInfo,
    pub tags: Option<TagDocument>,
    pub track_number: Option<u32>,
}

pub fn probe_target(path: &Path, tools: &ToolSet) -> Result<TargetProbe, String> {
    let info = identify(path, tools)?;
    let tags = extract_tags(path, tools).ok().flatten();
    let track_number = tags
        .as_ref()
        .and_then(|document| {
            let uid = info.first_audio().map(|track| track.uid);
            document
                .track_value(uid, "PART_NUMBER")
                .and_then(|value| value.parse::<u32>().ok())
        })
        .or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(crate::matching::extract_track_number)
        });

    Ok(TargetProbe {
        info,
        tags,
        track_number,
    })
}

pub fn identify(path: &Path, tools: &ToolSet) -> Result<TargetInfo, String> {
    let Some(tool) = &tools.mkvmerge else {
        return Err("mkvmerge is not available; target introspection is limited.".to_owned());
    };

    let args = vec![OsString::from("-J"), path.as_os_str().to_os_string()];
    let output = runner::run(tool, &args, Duration::from_secs(90))?;
    if !output.success() {
        return Err(format!(
            "mkvmerge could not inspect {}: {}",
            path.display(),
            output.combined()
        ));
    }

    parse_identify(&output.stdout, path)
}

fn parse_identify(json: &str, path: &Path) -> Result<TargetInfo, String> {
    let raw: RawIdentify = serde_json::from_str(json).map_err(|error| {
        format!(
            "Could not parse mkvmerge JSON for {}: {error}",
            path.display()
        )
    })?;

    let duration_secs = raw
        .container
        .properties
        .duration
        .filter(|duration| *duration > 0)
        .map(|duration| duration as f64 / 1_000_000_000.0);

    let tracks = raw
        .tracks
        .into_iter()
        .map(|track| {
            let audio = track.track_type.eq_ignore_ascii_case("audio")
                || track.codec.to_ascii_uppercase().starts_with("A_");
            MkvTrack {
                id: track.id,
                uid: track.properties.uid.unwrap_or_default(),
                number: track
                    .properties
                    .number
                    .unwrap_or_else(|| track.id.saturating_add(1)),
                codec: track.codec,
                audio,
                channels: track.properties.audio_channels.unwrap_or_default(),
                language: track
                    .properties
                    .language
                    .unwrap_or_else(|| "und".to_owned()),
                name: track.properties.track_name,
                default: track.properties.default_track,
                forced: track.properties.forced_track,
            }
        })
        .collect();

    let attachments = raw
        .attachments
        .into_iter()
        .map(|attachment| MkvAttachment {
            id: attachment.id,
            uid: attachment.uid.unwrap_or_default(),
            name: attachment.name,
            mime_type: if attachment.content_type.is_empty() {
                attachment.mime_type
            } else {
                attachment.content_type
            },
            description: attachment.description,
            size: attachment.size,
        })
        .collect();

    Ok(TargetInfo {
        duration_secs,
        tracks,
        attachments,
    })
}

pub fn extract_tags(path: &Path, tools: &ToolSet) -> Result<Option<TagDocument>, String> {
    let Some(tool) = &tools.mkvextract else {
        return Ok(None);
    };

    let temp_dir = crate::config::cache_dir().join("tmp");
    fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("Could not create {}: {error}", temp_dir.display()))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let output_path = temp_dir.join(format!("tags-{}-{stamp}.xml", std::process::id()));

    let args = vec![
        path.as_os_str().to_os_string(),
        OsString::from("tags"),
        output_path.as_os_str().to_os_string(),
    ];
    let output = runner::run(tool, &args, Duration::from_secs(90))?;
    let text = fs::read_to_string(&output_path).unwrap_or_default();
    let _ = fs::remove_file(&output_path);
    let text = text.trim_start_matches('\u{feff}');

    if text.trim().is_empty() {
        return Ok(None);
    }
    if !output.success() {
        return Err(format!(
            "mkvextract could not read tags from {}: {}",
            path.display(),
            output.combined()
        ));
    }

    tags::parse(text).map(Some).map_err(|error| {
        format!(
            "Could not parse existing tags from {}: {error}",
            path.display()
        )
    })
}

#[derive(Debug, Deserialize)]
struct RawIdentify {
    #[serde(default)]
    attachments: Vec<RawAttachment>,
    #[serde(default)]
    container: RawContainer,
    #[serde(default)]
    tracks: Vec<RawTrack>,
}

#[derive(Debug, Default, Deserialize)]
struct RawContainer {
    #[serde(default)]
    properties: RawContainerProperties,
}

#[derive(Debug, Default, Deserialize)]
struct RawContainerProperties {
    #[serde(default)]
    duration: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawTrack {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    codec: String,
    #[serde(default, rename = "type")]
    track_type: String,
    #[serde(default)]
    properties: RawTrackProperties,
}

#[derive(Debug, Default, Deserialize)]
struct RawTrackProperties {
    #[serde(default)]
    uid: Option<u64>,
    #[serde(default)]
    number: Option<u64>,
    #[serde(default)]
    audio_channels: Option<u32>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default, alias = "name")]
    track_name: Option<String>,
    #[serde(default, rename = "default_track")]
    default_track: bool,
    #[serde(default, rename = "forced_track")]
    forced_track: bool,
}

#[derive(Debug, Deserialize)]
struct RawAttachment {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    uid: Option<u64>,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "content_type")]
    content_type: String,
    #[serde(default, rename = "mime_type")]
    mime_type: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    size: u64,
}
