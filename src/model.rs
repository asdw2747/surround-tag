//! Shared domain model for source masters, 5.1 Matroska targets, pairings,
//! editable tag drafts, and batch results.

use std::path::PathBuf;

use crate::tags::TagDocument;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverMime {
    Jpeg,
    Png,
}

impl CoverMime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
        }
    }

    pub fn from_mime(value: &str) -> Option<Self> {
        let lower = value.trim().to_ascii_lowercase();
        if lower == "image/jpeg" || lower == "image/jpg" {
            Some(Self::Jpeg)
        } else if lower == "image/png" {
            Some(Self::Png)
        } else {
            None
        }
    }

    pub fn from_magic(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoverSource {
    Embedded,
    Sidecar(PathBuf),
    Override(PathBuf),
}

impl CoverSource {
    pub fn label(&self) -> String {
        match self {
            Self::Embedded => "Embedded front cover".to_owned(),
            Self::Sidecar(path) => path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| format!("Folder artwork: {name}"))
                .unwrap_or_else(|| "Folder artwork".to_owned()),
            Self::Override(path) => path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| format!("Override: {name}"))
                .unwrap_or_else(|| "Cover override".to_owned()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CoverArt {
    pub bytes: Vec<u8>,
    pub mime: CoverMime,
    pub source: CoverSource,
}

impl CoverArt {
    pub fn filename(&self) -> String {
        format!("cover.{}", self.mime.extension())
    }

    pub fn cache_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.bytes.hash(&mut hasher);
        self.mime.as_str().hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeStatus {
    Pending,
    Probing,
    Ready,
    Warning,
    Error,
}

impl ProbeStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Probing => "Probing",
            Self::Ready => "Ready",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SourceEntry {
    pub id: u64,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub year: Option<String>,
    pub genre: String,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub total_tracks: Option<u32>,
    pub duration_secs: Option<f64>,
    pub cover: Option<CoverArt>,
    pub status: ProbeStatus,
    pub error: Option<String>,
}

impl SourceEntry {
    pub fn new(id: u64, path: PathBuf) -> Self {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Unknown title")
            .to_owned();
        Self {
            id,
            path,
            title: stem,
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            year: None,
            genre: String::new(),
            track_number: None,
            disc_number: None,
            total_tracks: None,
            duration_secs: None,
            cover: None,
            status: ProbeStatus::Pending,
            error: None,
        }
    }

    pub fn display_title(&self) -> &str {
        if self.title.trim().is_empty() {
            self.path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Unknown title")
        } else {
            &self.title
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TargetInfo {
    pub duration_secs: Option<f64>,
    pub tracks: Vec<MkvTrack>,
    pub attachments: Vec<MkvAttachment>,
}

impl TargetInfo {
    pub fn audio_tracks(&self) -> Vec<&MkvTrack> {
        self.tracks.iter().filter(|track| track.audio).collect()
    }

    pub fn first_audio(&self) -> Option<&MkvTrack> {
        self.tracks.iter().find(|track| track.audio)
    }
}

#[derive(Clone, Debug, Default)]
pub struct MkvTrack {
    pub id: u64,
    pub uid: u64,
    pub number: u64,
    pub codec: String,
    pub audio: bool,
    pub channels: u32,
    pub language: String,
    pub name: Option<String>,
    pub default: bool,
    pub forced: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MkvAttachment {
    pub id: u64,
    pub uid: u64,
    pub name: String,
    pub mime_type: String,
    pub description: Option<String>,
    pub size: u64,
}

impl MkvAttachment {
    pub fn looks_like_cover(&self) -> bool {
        let name = self.name.to_ascii_lowercase();
        let description = self
            .description
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mime = self.mime_type.to_ascii_lowercase();
        let named_cover =
            name.starts_with("cover") || name.starts_with("front") || name.starts_with("folder");
        let image_like = mime.starts_with("image/")
            || name.ends_with(".jpg")
            || name.ends_with(".jpeg")
            || name.ends_with(".png")
            || name.ends_with(".webp");
        (named_cover && (image_like || name.contains("cover") || name.contains("front")))
            || description.contains("cover")
            || description.contains("front")
            || description.contains("artwork")
    }
}

#[derive(Clone, Debug)]
pub struct TargetEntry {
    pub id: u64,
    pub path: PathBuf,
    pub track_number: Option<u32>,
    pub duration_secs: Option<f64>,
    pub info: Option<TargetInfo>,
    pub existing_tags: Option<TagDocument>,
    pub status: ProbeStatus,
    pub error: Option<String>,
}

impl TargetEntry {
    pub fn new(id: u64, path: PathBuf) -> Self {
        Self {
            id,
            path,
            track_number: None,
            duration_secs: None,
            info: None,
            existing_tags: None,
            status: ProbeStatus::Pending,
            error: None,
        }
    }

    pub fn display_title(&self) -> String {
        self.existing_tags
            .as_ref()
            .and_then(|doc| {
                doc.track_value(
                    self.info
                        .as_ref()
                        .and_then(|info| info.first_audio())
                        .map(|track| track.uid),
                    "TITLE",
                )
            })
            .map(str::to_owned)
            .unwrap_or_else(|| {
                self.path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("Unknown target")
                    .to_owned()
            })
    }

    pub fn audio_summary(&self) -> String {
        let Some(info) = &self.info else {
            return "No track information".to_owned();
        };
        let Some(track) = info.first_audio() else {
            return "No audio track".to_owned();
        };
        let default = if track.default {
            "default"
        } else {
            "not default"
        };
        let name = track.name.as_deref().unwrap_or("unnamed");
        format!(
            "{} · {} ch · {} · {} · {}",
            track.codec, track.channels, track.language, default, name
        )
    }
}

#[derive(Clone, Debug)]
pub struct Pairing {
    pub source_id: u64,
    pub target_id: u64,
    pub confidence: f32,
    pub reasons: Vec<MatchReason>,
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MatchReason {
    ExactStem,
    TrackNumber,
    NameSimilarity(u32),
    DurationDelta(f64),
    Locked,
}

impl MatchReason {
    pub fn label(&self) -> String {
        match self {
            Self::ExactStem => "Exact stem".to_owned(),
            Self::TrackNumber => "Track #".to_owned(),
            Self::NameSimilarity(percent) => format!("Name {percent}%"),
            Self::DurationDelta(seconds) => format!("Duration Δ{seconds:.1}s"),
            Self::Locked => "Locked".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AlbumDraft {
    pub title: String,
    pub artist: String,
    pub year: String,
    pub genre: String,
    pub total_tracks: String,
}

impl AlbumDraft {
    pub fn from_source(source: &SourceEntry) -> Self {
        Self {
            title: source.album.clone(),
            artist: if source.album_artist.trim().is_empty() {
                source.artist.clone()
            } else {
                source.album_artist.clone()
            },
            year: source.year.clone().unwrap_or_default(),
            genre: source.genre.clone(),
            total_tracks: source
                .total_tracks
                .map(|total| total.to_string())
                .unwrap_or_default(),
        }
    }

    pub fn total_tracks_value(&self) -> Option<u32> {
        parse_u32(&self.total_tracks)
    }
}

#[derive(Clone, Debug, Default)]
pub struct TrackDraft {
    pub track_number: String,
    pub disc_number: String,
    pub title: String,
    pub artist: String,
}

impl TrackDraft {
    pub fn from_source(source: &SourceEntry) -> Self {
        Self {
            track_number: source
                .track_number
                .map(|number| number.to_string())
                .unwrap_or_default(),
            disc_number: source
                .disc_number
                .map(|number| number.to_string())
                .unwrap_or_default(),
            title: if source.title.trim().is_empty() {
                source
                    .path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or_default()
                    .to_owned()
            } else {
                source.title.clone()
            },
            artist: source.artist.clone(),
        }
    }

    pub fn track_number_value(&self) -> Option<u32> {
        parse_u32(&self.track_number)
    }

    pub fn disc_number_value(&self) -> Option<u32> {
        parse_u32(&self.disc_number)
    }
}

pub fn parse_u32(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u32>().ok()
}

#[derive(Clone, Debug)]
pub struct FolderArtWrite {
    pub directory: PathBuf,
    pub filename: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyState {
    Pending,
    Applying,
    Verifying,
    Done,
    Warning,
    Failed,
}

impl ApplyState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Applying => "Applying",
            Self::Verifying => "Verifying",
            Self::Done => "Done",
            Self::Warning => "Warning",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ApplyItemResult {
    pub target: PathBuf,
    pub renamed_to: Option<PathBuf>,
    pub state: ApplyState,
    pub message: String,
    pub warnings: Vec<String>,
    pub log: String,
    pub folder_art: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct ApplySummary {
    pub results: Vec<ApplyItemResult>,
    pub succeeded: usize,
    pub warnings: usize,
    pub failed: usize,
    pub renamed: usize,
    pub elapsed_secs: f64,
}

impl ApplySummary {
    pub fn total(&self) -> usize {
        self.results.len()
    }

    pub fn recompute(&mut self) {
        self.succeeded = self
            .results
            .iter()
            .filter(|result| result.state == ApplyState::Done)
            .count();
        self.warnings = self
            .results
            .iter()
            .filter(|result| result.state == ApplyState::Warning)
            .count();
        self.failed = self
            .results
            .iter()
            .filter(|result| result.state == ApplyState::Failed)
            .count();
        self.renamed = self
            .results
            .iter()
            .filter(|result| result.renamed_to.is_some())
            .count();
    }
}
