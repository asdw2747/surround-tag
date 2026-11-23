//! Matroska tag document model and the album/track tag plan.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagPolicy {
    Merge,
    Replace,
}

#[derive(Clone, Debug, Default)]
pub struct TagDocument {
    pub tags: Vec<TagElement>,
}

#[derive(Clone, Debug, Default)]
pub struct TagElement {
    pub target_type_value: u32,
    pub track_uid: Option<u64>,
    pub simple: Vec<SimpleTag>,
}

#[derive(Clone, Debug, Default)]
pub struct SimpleTag {
    pub name: String,
    pub value: String,
}

impl TagDocument {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn album(&self) -> Option<&TagElement> {
        self.tags
            .iter()
            .find(|tag| tag.target_type_value == 50 && tag.track_uid.is_none())
            .or_else(|| self.tags.iter().find(|tag| tag.target_type_value == 50))
            .or_else(|| {
                self.tags
                    .iter()
                    .find(|tag| tag.target_type_value == 0 && tag.track_uid.is_none())
            })
    }

    pub fn album_mut(&mut self) -> &mut TagElement {
        if let Some(index) = self
            .tags
            .iter()
            .position(|tag| tag.target_type_value == 50 && tag.track_uid.is_none())
            .or_else(|| {
                self.tags
                    .iter()
                    .position(|tag| tag.target_type_value == 0 && tag.track_uid.is_none())
            })
        {
            self.tags[index].target_type_value = 50;
            return &mut self.tags[index];
        }
        self.tags.push(TagElement {
            target_type_value: 50,
            ..Default::default()
        });
        self.tags.last_mut().expect("tag was just pushed")
    }

    pub fn track(&self, uid: Option<u64>) -> Option<&TagElement> {
        if let Some(uid) = uid {
            if let Some(tag) = self.tags.iter().find(|tag| tag.track_uid == Some(uid)) {
                return Some(tag);
            }
        }
        let mut tracks = self.tags.iter().filter(|tag| {
            tag.target_type_value == 30 || (tag.target_type_value == 0 && tag.track_uid.is_some())
        });
        let first = tracks.next()?;
        if tracks.next().is_none() {
            Some(first)
        } else {
            None
        }
    }

    pub fn track_mut(&mut self, uid: Option<u64>) -> &mut TagElement {
        if let Some(uid) = uid {
            if let Some(index) = self.tags.iter().position(|tag| tag.track_uid == Some(uid)) {
                self.tags[index].target_type_value = 30;
                return &mut self.tags[index];
            }
        } else if let Some(index) = self.tags.iter().position(|tag| {
            tag.target_type_value == 30 && tag.track_uid.is_none()
                || tag.target_type_value == 0 && tag.track_uid.is_none()
        }) {
            self.tags[index].target_type_value = 30;
            return &mut self.tags[index];
        }

        self.tags.push(TagElement {
            target_type_value: 30,
            track_uid: uid,
            ..Default::default()
        });
        self.tags.last_mut().expect("tag was just pushed")
    }

    pub fn track_value(&self, uid: Option<u64>, name: &str) -> Option<&str> {
        self.track(uid)
            .and_then(|tag| tag.value(name))
            .filter(|value| !value.trim().is_empty())
    }
}

impl TagElement {
    pub fn value(&self, name: &str) -> Option<&str> {
        self.simple
            .iter()
            .find(|tag| tag.name == name)
            .map(|tag| tag.value.as_str())
    }

    pub fn set(&mut self, name: &str, value: impl Into<String>) {
        let value = sanitize_xml_value(&value.into());
        self.remove(name);
        if !value.is_empty() {
            self.simple.push(SimpleTag {
                name: name.to_owned(),
                value,
            });
        }
    }

    pub fn set_optional(&mut self, name: &str, value: Option<String>) {
        match value {
            Some(value) if !value.trim().is_empty() => self.set(name, value),
            _ => self.remove(name),
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.simple.retain(|tag| tag.name != name);
    }
}

#[derive(Clone, Debug, Default)]
pub struct TagPlan {
    pub album: AlbumTags,
    pub track: TrackTags,
    pub track_uid: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct AlbumTags {
    pub title: String,
    pub artist: String,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub total_tracks: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct TrackTags {
    pub title: String,
    pub artist: String,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_offset: Option<u32>,
}

/// Remove characters that XML 1.0 cannot represent and collapse line breaks
/// so a malformed tag value can never make mkvpropedit reject the document.
pub fn sanitize_xml_value(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\u{9}' | '\u{A}' | '\u{D}' => output.push(' '),
            '\u{0}'..='\u{1F}' => output.push(' '),
            '\u{FFFE}' | '\u{FFFF}' => {}
            _ => output.push(character),
        }
    }
    output.trim().to_owned()
}

pub const ALBUM_TAGS: &[&str] = &["TITLE", "ARTIST", "DATE_RELEASED", "GENRE", "TOTAL_PARTS"];
pub const TRACK_TAGS: &[&str] = &[
    "TITLE",
    "ARTIST",
    "PART_NUMBER",
    "PART_OFFSET",
    "_DISC_NUMBER",
];
