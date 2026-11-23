use std::path::Path;

use lofty::picture::PictureType;
use lofty::prelude::*;

use crate::model::{CoverArt, CoverSource};

use super::artwork;

#[derive(Clone, Debug, Default)]
pub struct ProbedSource {
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
}

pub fn probe_source(path: &Path) -> Result<ProbedSource, String> {
    let tagged = lofty::read_from_path(path)
        .map_err(|error| format!("Could not read audio metadata: {error}"))?;

    let duration_secs = Some(tagged.properties().duration().as_secs_f64());
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let mut probed = ProbedSource {
        duration_secs,
        ..Default::default()
    };

    if let Some(tag) = tag {
        probed.title = tag
            .title()
            .map(|value| value.into_owned())
            .unwrap_or_default();
        probed.artist = tag
            .artist()
            .map(|value| value.into_owned())
            .unwrap_or_default();
        probed.album = tag
            .album()
            .map(|value| value.into_owned())
            .unwrap_or_default();
        probed.album_artist = tag
            .get_string(ItemKey::AlbumArtist)
            .map(str::to_owned)
            .unwrap_or_default();
        probed.year = extract_year(tag);
        probed.genre = tag
            .genre()
            .map(|value| value.into_owned())
            .unwrap_or_default();
        probed.track_number = tag.track();
        probed.disc_number = tag.disk();
        probed.total_tracks = tag.track_total();

        let picture = tag
            .get_picture_type(PictureType::CoverFront)
            .or_else(|| tag.pictures().first());

        if let Some(picture) = picture {
            probed.cover =
                artwork::cover_from_bytes(picture.data().to_vec(), CoverSource::Embedded).ok();
        }
    }

    if probed.cover.is_none() {
        probed.cover = path.parent().and_then(artwork::find_sidecar_cover);
    }

    Ok(probed)
}

fn extract_year(tag: &lofty::tag::Tag) -> Option<String> {
    if let Some(date) = tag.date() {
        if (1000..=2999).contains(&date.year) {
            return Some(format!("{:04}", date.year));
        }
    }

    for key in [
        ItemKey::Year,
        ItemKey::RecordingDate,
        ItemKey::OriginalReleaseDate,
    ] {
        if let Some(value) = tag.get_string(key) {
            if let Some(year) = parse_year(value) {
                return Some(year);
            }
        }
    }

    None
}

fn parse_year(value: &str) -> Option<String> {
    let mut digits = String::new();
    for character in value.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_digit() {
            digits.push(character);
            continue;
        }
        if !digits.is_empty() {
            let candidate = if digits.len() == 4 {
                digits.parse::<u16>().ok()
            } else if digits.len() > 4 {
                digits[..4].parse::<u16>().ok()
            } else {
                None
            };
            if let Some(year) = candidate {
                if (1000..=2999).contains(&year) {
                    return Some(format!("{year:04}"));
                }
            }
            digits.clear();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_year;

    #[test]
    fn parses_common_date_shapes() {
        assert_eq!(parse_year("2026"), Some("2026".to_owned()));
        assert_eq!(parse_year("2026-05-01"), Some("2026".to_owned()));
        assert_eq!(parse_year("2026/05/01"), Some("2026".to_owned()));
        assert_eq!(parse_year("01.05.2026"), Some("2026".to_owned()));
        assert_eq!(parse_year("released 2026"), Some("2026".to_owned()));
        assert_eq!(parse_year("0000"), None);
        assert_eq!(parse_year("not a date"), None);
    }
}
