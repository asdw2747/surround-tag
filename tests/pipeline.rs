//! Public-API integration coverage for matching, templates, and tag XML.

use std::path::PathBuf;

use surround_tag::matching::{auto_pair, extract_track_number, normalize_stem, score_pair};
use surround_tag::model::{ProbeStatus, SourceEntry, TargetEntry};
use surround_tag::tags::{self, AlbumTags, TagPlan, TagPolicy, TrackTags};
use surround_tag::template::{render, sanitize_component, TemplateContext};

fn source(id: u64, name: &str, track: u32, duration: f64) -> SourceEntry {
    let mut source = SourceEntry::new(id, PathBuf::from(name));
    source.track_number = Some(track);
    source.duration_secs = Some(duration);
    source.title = format!("Track {track}");
    source.album = "Album".to_owned();
    source.artist = "Artist".to_owned();
    source.album_artist = "Album Artist".to_owned();
    source.year = Some("2026".to_owned());
    source.status = ProbeStatus::Ready;
    source
}

fn target(id: u64, name: &str, track: u32, duration: f64) -> TargetEntry {
    let mut target = TargetEntry::new(id, PathBuf::from(name));
    target.track_number = Some(track);
    target.duration_secs = Some(duration);
    target.status = ProbeStatus::Ready;
    target
}

#[test]
fn public_matching_api_pairs_a_batch() {
    let sources = vec![
        source(1, "01 - Opening.flac", 1, 120.0),
        source(2, "02 - Closing.flac", 2, 240.0),
    ];
    let targets = vec![
        target(10, "01 - Opening (5.1).mka", 1, 120.2),
        target(11, "02 - Closing (5.1).mka", 2, 240.1),
    ];

    assert_eq!(normalize_stem("01 - Opening (5.1 E-AC-3)"), "01 opening");
    assert_eq!(extract_track_number("02 - Closing"), Some(2));
    assert!(score_pair(&sources[0], &targets[0]).score > 0.85);

    let pairs = auto_pair(&sources, &targets, &[]);
    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs[0].target_id, 10);
    assert_eq!(pairs[1].target_id, 11);
}

#[test]
fn public_template_api_previews_renames() {
    let context = TemplateContext {
        track: Some(2),
        disc: Some(1),
        title: "Closing: Reprise?".to_owned(),
        artist: "Artist".to_owned(),
        album: "Album".to_owned(),
        album_artist: "Album Artist".to_owned(),
        year: "2026".to_owned(),
        ext: "mka".to_owned(),
    };
    assert_eq!(
        render("{track:02} - {title} (5.1).mka", &context).unwrap(),
        "02 - Closing_ Reprise_ (5.1).mka"
    );
    assert_eq!(sanitize_component("CON.mka"), "_CON.mka");
}

#[test]
fn public_tag_api_renders_and_merges() {
    let plan = TagPlan {
        album: AlbumTags {
            title: "Album".to_owned(),
            artist: "Album Artist".to_owned(),
            year: Some("2026".to_owned()),
            genre: Some("Electronic".to_owned()),
            total_tracks: Some(2),
        },
        track: TrackTags {
            title: "Opening".to_owned(),
            artist: "Artist".to_owned(),
            track_number: Some(1),
            disc_number: None,
            disc_offset: None,
        },
        track_uid: Some(7),
    };
    let fresh = tags::fresh_document(&plan);
    let xml = tags::render(&fresh);
    assert!(xml.contains("<TargetTypeValue>50</TargetTypeValue>"));
    assert!(xml.contains("<TagTrackUID>7</TagTrackUID>"));

    let parsed = tags::parse(&xml).unwrap();
    let merged = tags::plan_document(Some(&parsed), &plan, TagPolicy::Merge);
    assert_eq!(
        merged.album().and_then(|tag| tag.value("TITLE")),
        Some("Album")
    );
}
