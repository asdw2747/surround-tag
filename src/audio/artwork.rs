use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::ImageFormat;

use crate::model::{CoverArt, CoverMime, CoverSource};

const MAX_COVER_BYTES: u64 = 20 * 1024 * 1024;

const PREFERRED_NAMES: &[&str] = &[
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "folder.jpg",
    "folder.jpeg",
    "folder.png",
    "front.jpg",
    "front.jpeg",
    "front.png",
];

pub fn find_sidecar_cover(dir: &Path) -> Option<CoverArt> {
    let entries: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();

    for preferred in PREFERRED_NAMES {
        if let Some(path) = entries.iter().find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(preferred))
        }) {
            if let Ok(mut cover) = load_cover_file(path) {
                cover.source = CoverSource::Sidecar(path.clone());
                return Some(cover);
            }
        }
    }

    for path in entries {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        let image_like = lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".png")
            || lower.ends_with(".webp");
        if !image_like {
            continue;
        }
        if lower.contains("cover") || lower.contains("front") || lower.contains("folder") {
            if let Ok(mut cover) = load_cover_file(&path) {
                cover.source = CoverSource::Sidecar(path.clone());
                return Some(cover);
            }
        }
    }

    None
}

pub fn load_cover_file(path: &Path) -> Result<CoverArt, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    if metadata.len() > MAX_COVER_BYTES {
        return Err(format!(
            "Cover art {} is larger than 20 MB.",
            path.display()
        ));
    }

    let bytes =
        fs::read(path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    cover_from_bytes(bytes, CoverSource::Override(path.to_path_buf()))
        .map_err(|error| format!("Could not load cover art {}: {error}", path.display()))
}

/// Validate and normalize cover bytes. JPEG/PNG bytes are preserved exactly;
/// other decodable formats (notably WebP) are converted to PNG so Matroska
/// players and Kodi can display them.
pub fn cover_from_bytes(bytes: Vec<u8>, source: CoverSource) -> Result<CoverArt, String> {
    if bytes.is_empty() {
        return Err("Cover art is empty.".to_owned());
    }
    if bytes.len() as u64 > MAX_COVER_BYTES {
        return Err("Cover art is larger than 20 MB.".to_owned());
    }
    if let Some(mime) = CoverMime::from_magic(&bytes) {
        return Ok(CoverArt {
            bytes,
            mime,
            source,
        });
    }

    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("Could not decode cover art: {error}"))?;
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|error| format!("Could not convert cover art to PNG: {error}"))?;

    Ok(CoverArt {
        bytes: png.into_inner(),
        mime: CoverMime::Png,
        source,
    })
}
