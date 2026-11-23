//! Source probing coverage against a small real MP3 fixture with ID3 tags and
//! an embedded front cover.

use std::path::PathBuf;

use lofty::prelude::*;
use surround_tag::audio::probe_source;

#[test]
fn reads_id3_metadata_and_embedded_cover() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fixture.mp3");
    let probed = probe_source(&path).expect("fixture should be readable");

    assert_eq!(probed.title, "Fixture Track");
    assert_eq!(probed.artist, "Fixture Artist");
    assert_eq!(probed.album, "Fixture Album");
    assert_eq!(probed.album_artist, "Fixture Album Artist");
    assert_eq!(probed.genre, "Electronic");
    assert_eq!(probed.track_number, Some(3));
    assert_eq!(probed.year.as_deref(), Some("2026"));
    assert!(probed.duration_secs.unwrap_or_default() > 0.5);

    let cover = probed.cover.expect("embedded cover should be found");
    assert_eq!(cover.mime, surround_tag::model::CoverMime::Png);
    assert!(cover.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(cover.source.label().contains("Embedded"));
}

#[test]
fn discovers_folder_artwork() {
    let temp = tempfile::tempdir().unwrap();
    let icon = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("icon.png"),
    )
    .unwrap();
    std::fs::write(temp.path().join("folder.png"), &icon).unwrap();

    let cover = surround_tag::audio::find_sidecar_cover(temp.path()).expect("folder.png");
    assert_eq!(cover.mime, surround_tag::model::CoverMime::Png);
    assert!(matches!(
        cover.source,
        surround_tag::model::CoverSource::Sidecar(_)
    ));
}

fn write_tag(
    key: lofty::tag::ItemKey,
    value: &str,
    remove: &[lofty::tag::ItemKey],
) -> surround_tag::audio::ProbedSource {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("fixture.mp3");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("fixture.mp3"),
        &path,
    )
    .unwrap();

    let mut tagged = lofty::read_from_path(&path).unwrap();
    let tag = tagged.primary_tag_mut().expect("fixture has a tag");
    for key in remove {
        tag.remove_key(*key);
    }
    tag.insert_text(key, value.to_owned());
    tagged
        .save_to_path(&path, lofty::config::WriteOptions::default())
        .unwrap();

    probe_source(&path).unwrap()
}

#[test]
fn year_comes_from_recording_date_even_with_newer_mtime() {
    let mut probed = write_tag(
        lofty::tag::ItemKey::RecordingDate,
        "1998-04-02",
        &[
            lofty::tag::ItemKey::Year,
            lofty::tag::ItemKey::OriginalReleaseDate,
        ],
    );
    assert_eq!(probed.year.as_deref(), Some("1998"));

    // Change the file's modification time to a different year. The tag must win.
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("mtime.mp3");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("fixture.mp3"),
        &path,
    )
    .unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_modified(
        std::time::SystemTime::now() + std::time::Duration::from_secs(60 * 60 * 24 * 365 * 8),
    )
    .unwrap();
    drop(file);
    probed = probe_source(&path).unwrap();
    assert_eq!(probed.year.as_deref(), Some("2026"));
}

#[test]
fn year_falls_back_to_original_release_date() {
    let probed = write_tag(
        lofty::tag::ItemKey::OriginalReleaseDate,
        "1977-06-01",
        &[
            lofty::tag::ItemKey::Year,
            lofty::tag::ItemKey::RecordingDate,
        ],
    );
    assert_eq!(probed.year.as_deref(), Some("1977"));
}

#[test]
fn converts_webp_cover_to_png() {
    let bytes = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("cover.webp"),
    )
    .unwrap();
    let cover = surround_tag::audio::artwork::cover_from_bytes(
        bytes,
        surround_tag::model::CoverSource::Embedded,
    )
    .unwrap();
    assert_eq!(cover.mime, surround_tag::model::CoverMime::Png);
    assert!(cover.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}
