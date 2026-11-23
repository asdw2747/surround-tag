//! Matroska tag XML generation, parsing, and merge-aware planning.

use std::fmt::Write as _;

use quick_xml::escape::{escape, resolve_xml_entity, unescape};
use quick_xml::events::Event;
use quick_xml::Reader;

use super::model::{AlbumTags, SimpleTag, TagDocument, TagElement, TagPlan, TagPolicy, TrackTags};

pub fn plan_document(
    existing: Option<&TagDocument>,
    plan: &TagPlan,
    policy: TagPolicy,
) -> TagDocument {
    match policy {
        TagPolicy::Replace => fresh_document(plan),
        TagPolicy::Merge => {
            let mut document = existing.cloned().unwrap_or_default();
            merge_album(document.album_mut(), &plan.album);
            let track = document.track_mut(plan.track_uid);
            if plan.track_uid.is_some() {
                track.track_uid = plan.track_uid;
            }
            merge_track(track, &plan.track);
            document
        }
    }
}

pub fn fresh_document(plan: &TagPlan) -> TagDocument {
    let mut album = TagElement {
        target_type_value: 50,
        ..Default::default()
    };
    merge_album(&mut album, &plan.album);

    let mut track = TagElement {
        target_type_value: 30,
        track_uid: plan.track_uid,
        ..Default::default()
    };
    merge_track(&mut track, &plan.track);

    TagDocument {
        tags: vec![album, track],
    }
}

fn merge_album(element: &mut TagElement, album: &AlbumTags) {
    element.target_type_value = 50;
    element.set("TITLE", album.title.clone());
    element.set("ARTIST", album.artist.clone());
    element.set_optional("DATE_RELEASED", album.year.clone());
    element.set_optional("GENRE", album.genre.clone());
    element.set_optional(
        "TOTAL_PARTS",
        album.total_tracks.map(|total| total.to_string()),
    );
}

fn merge_track(element: &mut TagElement, track: &TrackTags) {
    element.target_type_value = 30;
    element.set("TITLE", track.title.clone());
    element.set("ARTIST", track.artist.clone());
    element.set_optional(
        "PART_NUMBER",
        track.track_number.map(|number| number.to_string()),
    );
    element.set_optional(
        "PART_OFFSET",
        track.disc_offset.map(|offset| offset.to_string()),
    );
    element.set_optional(
        "_DISC_NUMBER",
        track.disc_number.map(|number| number.to_string()),
    );
}

pub fn render(document: &TagDocument) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<!DOCTYPE Tags SYSTEM \"matroskatags.dtd\">\n");
    out.push_str("<Tags>\n");

    for tag in &document.tags {
        out.push_str("  <Tag>\n");
        out.push_str("    <Targets>\n");
        let _ = writeln!(
            out,
            "      <TargetTypeValue>{}</TargetTypeValue>",
            tag.target_type_value
        );
        if let Some(uid) = tag.track_uid {
            let _ = writeln!(out, "      <TagTrackUID>{uid}</TagTrackUID>");
        }
        out.push_str("    </Targets>\n");

        for simple in &tag.simple {
            let value_text = crate::tags::model::sanitize_xml_value(&simple.value);
            if value_text.is_empty() {
                continue;
            }
            let name = escape(&simple.name);
            let value = escape(&value_text);
            let _ = writeln!(
                out,
                "    <Simple><Name>{}</Name><String>{}</String></Simple>",
                name, value
            );
        }
        out.push_str("  </Tag>\n");
    }

    out.push_str("</Tags>\n");
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    TargetType,
    TrackUid,
    Name,
    String,
}

pub fn parse(xml: &str) -> Result<TagDocument, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut document = TagDocument::default();
    let mut current: Option<TagElement> = None;
    let mut field: Option<Field> = None;
    let mut text = String::new();
    let mut pending_name: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => match event.name().as_ref() {
                b"Tag" => {
                    current = Some(TagElement::default());
                    pending_name = None;
                }
                b"TargetTypeValue" => {
                    field = Some(Field::TargetType);
                    text.clear();
                }
                b"TagTrackUID" | b"TrackUID" => {
                    field = Some(Field::TrackUid);
                    text.clear();
                }
                b"Name" => {
                    field = Some(Field::Name);
                    text.clear();
                }
                b"String" => {
                    field = Some(Field::String);
                    text.clear();
                }
                _ => {}
            },
            Ok(Event::Text(event)) => {
                if field.is_some() {
                    let decoded = event
                        .decode()
                        .map_err(|error| format!("Could not decode tag XML text: {error}"))?;
                    let decoded = unescape(&decoded)
                        .map_err(|error| format!("Could not unescape tag XML text: {error}"))?;
                    text.push_str(&decoded);
                }
            }
            Ok(Event::CData(event)) => {
                if field.is_some() {
                    let decoded = event
                        .decode()
                        .map_err(|error| format!("Could not decode tag XML CDATA: {error}"))?;
                    text.push_str(&decoded);
                }
            }
            Ok(Event::GeneralRef(event)) => {
                if field.is_some() {
                    if let Some(character) = event.resolve_char_ref().map_err(|error| {
                        format!("Could not decode tag XML character reference: {error}")
                    })? {
                        text.push(character);
                    } else {
                        let name = event
                            .decode()
                            .map_err(|error| format!("Could not decode tag XML entity: {error}"))?;
                        if let Some(value) = resolve_xml_entity(&name) {
                            text.push_str(value);
                        }
                    }
                }
            }
            Ok(Event::End(event)) => {
                match event.name().as_ref() {
                    b"TargetTypeValue" => {
                        if let Some(tag) = current.as_mut() {
                            tag.target_type_value = text.trim().parse().unwrap_or_default();
                        }
                        field = None;
                    }
                    b"TagTrackUID" | b"TrackUID" => {
                        if let Some(tag) = current.as_mut() {
                            tag.track_uid = text.trim().parse().ok();
                        }
                        field = None;
                    }
                    b"Name" => {
                        pending_name = Some(text.trim().to_owned());
                        field = None;
                    }
                    b"String" => {
                        if let (Some(tag), Some(name)) = (current.as_mut(), pending_name.take()) {
                            if !name.is_empty() {
                                tag.simple.push(SimpleTag {
                                    name,
                                    value: text.trim().to_owned(),
                                });
                            }
                        }
                        field = None;
                    }
                    b"Tag" => {
                        if let Some(tag) = current.take() {
                            document.tags.push(tag);
                        }
                        pending_name = None;
                    }
                    _ => {}
                }
                text.clear();
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("Could not parse Matroska tag XML: {error}")),
            _ => {}
        }
    }

    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plan() -> TagPlan {
        TagPlan {
            album: AlbumTags {
                title: "Album & Title".to_owned(),
                artist: "Album <Artist>".to_owned(),
                year: Some("2026".to_owned()),
                genre: Some("Electronic".to_owned()),
                total_tracks: Some(10),
            },
            track: TrackTags {
                title: "Track \"One\"".to_owned(),
                artist: "Track Artist".to_owned(),
                track_number: Some(1),
                disc_number: Some(2),
                disc_offset: Some(10),
            },
            track_uid: Some(42),
        }
    }

    #[test]
    fn renders_and_parses_round_trip() {
        let document = fresh_document(&sample_plan());
        let xml = render(&document);
        assert!(xml.contains("TargetTypeValue>50<"));
        assert!(xml.contains("TargetTypeValue>30<"));
        assert!(xml.contains("<TagTrackUID>42</TagTrackUID>"));
        assert!(xml.contains("Album &amp; Title"));
        assert!(xml.contains("Album &lt;Artist&gt;"));

        let parsed = parse(&xml).unwrap();
        assert_eq!(
            parsed.album().unwrap().value("TITLE"),
            Some("Album & Title")
        );
        assert_eq!(parsed.album().unwrap().value("TOTAL_PARTS"), Some("10"));
        assert_eq!(
            parsed.track(Some(42)).unwrap().value("TITLE"),
            Some("Track \"One\"")
        );
        assert_eq!(
            parsed.track(Some(42)).unwrap().value("PART_OFFSET"),
            Some("10")
        );
    }

    #[test]
    fn escapes_specials_and_removes_invalid_xml_characters() {
        let mut plan = sample_plan();
        plan.track.title = "A & B < C > D \"E\" 'F' [feat. G]\u{0001}".to_owned();
        plan.album.title = "Album\u{000B}Name".to_owned();
        let document = fresh_document(&plan);
        let xml = render(&document);
        assert!(xml.contains("A &amp; B &lt; C &gt; D &quot;E&quot; &apos;F&apos; [feat. G]"));
        assert!(xml.contains("Album Name"));
        assert!(!xml.contains('\u{0001}'));
        assert!(!xml.contains('\u{000B}'));

        let parsed = parse(&xml).unwrap();
        assert_eq!(
            parsed.track(Some(42)).unwrap().value("TITLE"),
            Some("A & B < C > D \"E\" 'F' [feat. G]")
        );
    }

    #[test]
    fn merge_preserves_unmanaged_tags() {
        let mut existing = fresh_document(&sample_plan());
        existing.album_mut().simple.push(SimpleTag {
            name: "BPS".to_owned(),
            value: "640000".to_owned(),
        });
        let mut plan = sample_plan();
        plan.track.title = "New Title".to_owned();

        let merged = plan_document(Some(&existing), &plan, TagPolicy::Merge);
        assert_eq!(merged.album().unwrap().value("BPS"), Some("640000"));
        assert_eq!(
            merged.track(Some(42)).unwrap().value("TITLE"),
            Some("New Title")
        );
    }

    #[test]
    fn replace_drops_unmanaged_tags() {
        let mut existing = fresh_document(&sample_plan());
        existing.album_mut().simple.push(SimpleTag {
            name: "BPS".to_owned(),
            value: "640000".to_owned(),
        });
        let replaced = plan_document(Some(&existing), &sample_plan(), TagPolicy::Replace);
        assert_eq!(replaced.album().unwrap().value("BPS"), None);
    }
}
