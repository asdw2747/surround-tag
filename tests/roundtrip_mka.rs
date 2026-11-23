//! End-to-end Matroska round-trip test. Skipped unless a real mkvpropedit is
//! available; set `SURROUNDTAG_REQUIRE_MKV=1` to make a missing tool fatal.

use std::io::Cursor;
use std::path::PathBuf;

use image::{ImageFormat, Rgba, RgbaImage};
use surround_tag::config::AppConfig;
use surround_tag::mkv::{self, apply::ApplyJob, plan::TrackFlags};
use surround_tag::model::{ApplyState, CoverArt, CoverMime, CoverSource, FolderArtWrite};
use surround_tag::tags::{AlbumTags, TagPlan, TagPolicy, TrackTags};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test-5.1.mka")
}

#[test]
fn applies_tags_cover_flags_and_rename() {
    let tools = mkv::ToolSet::detect(&AppConfig::default());
    if !tools.can_apply() || tools.mkvmerge.is_none() || tools.mkvextract.is_none() {
        let message = "mkvpropedit is not available; skipping round-trip test";
        if std::env::var("SURROUNDTAG_REQUIRE_MKV").as_deref() == Ok("1") {
            panic!("{message}");
        }
        eprintln!("{message}");
        return;
    }

    let test_root = surround_tag::config::cache_dir().join("tests");
    std::fs::create_dir_all(&test_root).unwrap();
    let temp = tempfile::Builder::new()
        .prefix("roundtrip-")
        .tempdir_in(&test_root)
        .unwrap();
    let original = temp.path().join("01 - Test Track.mka");
    std::fs::copy(fixture(), &original).unwrap();

    let info = mkv::identify::identify(&original, &tools).unwrap();
    let uid = info.first_audio().unwrap().uid;
    let existing = mkv::identify::extract_tags(&original, &tools)
        .ok()
        .flatten();

    let mut png = Cursor::new(Vec::new());
    RgbaImage::from_pixel(16, 16, Rgba([0x38, 0xBD, 0xF8, 0xFF]))
        .write_to(&mut png, ImageFormat::Png)
        .unwrap();

    let cover = CoverArt {
        bytes: png.into_inner(),
        mime: CoverMime::Png,
        source: CoverSource::Embedded,
    };

    let plan = TagPlan {
        album: AlbumTags {
            title: "Test Album & Friends 'Deluxe'".to_owned(),
            artist: "Album Artist".to_owned(),
            year: Some("2026".to_owned()),
            genre: Some("Electronic".to_owned()),
            total_tracks: Some(10),
        },
        track: TrackTags {
            title: "Test Track [feat. O'Brien & Co.] <Live>".to_owned(),
            artist: "Track Artist".to_owned(),
            track_number: Some(1),
            disc_number: Some(1),
            disc_offset: None,
        },
        track_uid: Some(uid),
    };

    let renamed = temp.path().join("01 - Test Track (5.1).mka");
    let job = ApplyJob {
        target: original.clone(),
        target_info: info,
        existing_tags: existing,
        tag_plan: plan.clone(),
        cover: Some(cover.clone()),
        flags: TrackFlags {
            set_default: true,
            language: "eng".to_owned(),
            name: "5.1 Surround (E-AC-3 640 kbps)".to_owned(),
        },
        tag_policy: TagPolicy::Merge,
        rename_to: Some(renamed.clone()),
        overwrite_rename: false,
        keep_backup: false,
        folder_art: Some(FolderArtWrite {
            directory: temp.path().to_path_buf(),
            filename: "folder.png".to_owned(),
            bytes: cover.bytes.clone(),
        }),
    };

    let outcome = mkv::apply_job(&job, &tools);
    if outcome.result.state == ApplyState::Failed {
        panic!("apply failed:\n{}", outcome.result.log);
    }
    assert_eq!(
        outcome.result.state,
        ApplyState::Done,
        "{}",
        outcome.result.log
    );
    assert!(renamed.exists());
    assert!(!original.exists());
    assert_eq!(
        std::fs::read(temp.path().join("folder.png")).unwrap(),
        cover.bytes
    );

    let info = mkv::identify::identify(&renamed, &tools).unwrap();
    let audio = info.first_audio().unwrap();
    assert!(audio.default);
    assert_eq!(audio.language, "eng");
    assert_eq!(
        audio.name.as_deref(),
        Some("5.1 Surround (E-AC-3 640 kbps)")
    );
    assert!(info
        .attachments
        .iter()
        .any(|attachment| attachment.looks_like_cover()
            && attachment.size == cover.bytes.len() as u64));

    let document = mkv::identify::extract_tags(&renamed, &tools)
        .unwrap()
        .expect("tags were written");
    assert_eq!(
        document.album().and_then(|tag| tag.value("TITLE")),
        Some("Test Album & Friends 'Deluxe'")
    );
    assert_eq!(
        document.track(Some(uid)).and_then(|tag| tag.value("TITLE")),
        Some("Test Track [feat. O'Brien & Co.] <Live>")
    );
    assert_eq!(
        document
            .track(Some(uid))
            .and_then(|tag| tag.value("PART_NUMBER")),
        Some("1")
    );

    // A second apply must replace the cover rather than accumulate duplicates.
    let info = mkv::identify::identify(&renamed, &tools).unwrap();
    let job = ApplyJob {
        target: renamed.clone(),
        target_info: info,
        existing_tags: Some(document),
        tag_plan: plan,
        cover: Some(cover.clone()),
        flags: TrackFlags {
            set_default: true,
            language: "eng".to_owned(),
            name: "5.1 Surround (E-AC-3 640 kbps)".to_owned(),
        },
        tag_policy: TagPolicy::Merge,
        rename_to: None,
        overwrite_rename: false,
        keep_backup: false,
        folder_art: None,
    };
    let outcome = mkv::apply_job(&job, &tools);
    assert_ne!(
        outcome.result.state,
        ApplyState::Failed,
        "{}",
        outcome.result.log
    );
    let info = mkv::identify::identify(&renamed, &tools).unwrap();
    let covers = info
        .attachments
        .iter()
        .filter(|attachment| attachment.looks_like_cover())
        .count();
    assert_eq!(covers, 1, "cover attachments should not duplicate");
}
