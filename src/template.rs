//! Configurable filename templates with Windows-safe sanitization.

use std::path::{Path, PathBuf};

pub const MAX_COMPONENT_CHARS: usize = 150;
pub const MAX_COMPONENT_BYTES: usize = 200;

pub const PRESETS: &[(&str, &str)] = &[
    ("Track - Title", "{track:02} - {title}.mka"),
    ("Track - Title (5.1)", "{track:02} - {title} (5.1).mka"),
    (
        "Disc-Track - Title (5.1)",
        "{disc:02}-{track:02} - {title} (5.1).mka",
    ),
    (
        "Album Artist / Album / Track",
        "{album_artist} - {album}/{track:02} - {title}.mka",
    ),
];

#[derive(Clone, Debug, Default)]
pub struct TemplateContext {
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub year: String,
    pub ext: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Track,
    Disc,
    Title,
    Artist,
    Album,
    AlbumArtist,
    Year,
    Ext,
}

pub fn validate(template: &str) -> Result<(), String> {
    if template.trim().is_empty() {
        return Err("The naming template is empty.".to_owned());
    }
    for segment in template.split('/') {
        if segment.trim().is_empty() {
            return Err("The naming template has an empty path segment.".to_owned());
        }
        parse_segment(segment)?;
    }
    Ok(())
}

pub fn render(template: &str, context: &TemplateContext) -> Result<String, String> {
    validate(template)?;
    let mut segments = Vec::new();
    for segment in template.split('/') {
        let rendered = parse_segment(segment)?
            .into_iter()
            .map(|part| match part {
                Part::Literal(text) => text,
                Part::Token(token, width) => render_token(token, width, context),
            })
            .collect::<String>();
        segments.push(sanitize_component(&rendered));
    }

    let mut path = segments.join("/");
    if !path.to_ascii_lowercase().ends_with(".mka") {
        path.push_str(".mka");
    }
    Ok(path)
}

pub fn target_path(
    target_dir: &Path,
    template: &str,
    context: &TemplateContext,
) -> Result<PathBuf, String> {
    let relative = render(template, context)?;
    let path = target_dir.join(relative);
    let limit = if cfg!(windows) { 240 } else { 4096 };
    if path.to_string_lossy().len() > limit {
        return Err(format!(
            "The target path is too long (limit is {limit} bytes): {}",
            path.display()
        ));
    }
    Ok(path)
}

pub fn sanitize_component(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        if character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
        {
            output.push('_');
        } else {
            output.push(character);
        }
    }

    let mut trimmed = output.trim().trim_end_matches(['.', ' ']).to_owned();
    if trimmed.is_empty() {
        trimmed = "Untitled".to_owned();
    }

    let stem = trimmed.split('.').next().unwrap_or(&trimmed);
    if is_reserved(stem) {
        trimmed.insert(0, '_');
    }

    truncate_component(trimmed)
}

fn truncate_component(mut value: String) -> String {
    if value.chars().count() > MAX_COMPONENT_CHARS {
        value = value.chars().take(MAX_COMPONENT_CHARS).collect();
    }
    while value.len() > MAX_COMPONENT_BYTES {
        value.pop();
    }
    value.trim_end_matches(['.', ' ']).to_owned()
}

fn is_reserved(stem: &str) -> bool {
    let upper = stem.trim().to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
            | "CONIN$"
            | "CONOUT$"
    )
}

fn render_token(token: Token, width: Option<usize>, context: &TemplateContext) -> String {
    let number = match token {
        Token::Track => context.track,
        Token::Disc => context.disc,
        _ => None,
    };
    if let Some(number) = number {
        return match width {
            Some(width) => format!("{number:0width$}"),
            None => number.to_string(),
        };
    }

    match token {
        Token::Title => fallback(&context.title, "Untitled"),
        Token::Artist => context.artist.clone(),
        Token::Album => context.album.clone(),
        Token::AlbumArtist => {
            if context.album_artist.trim().is_empty() {
                context.artist.clone()
            } else {
                context.album_artist.clone()
            }
        }
        Token::Year => context.year.clone(),
        Token::Ext => {
            if context.ext.trim().is_empty() {
                "mka".to_owned()
            } else {
                context.ext.trim_start_matches('.').to_owned()
            }
        }
        Token::Track | Token::Disc => String::new(),
    }
}

fn fallback(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_owned()
    } else {
        value.to_owned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Literal(String),
    Token(Token, Option<usize>),
}

fn parse_segment(segment: &str) -> Result<Vec<Part>, String> {
    let chars: Vec<char> = segment.chars().collect();
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut index = 0;

    while index < chars.len() {
        match chars[index] {
            '{' if chars.get(index + 1) == Some(&'{') => {
                literal.push('{');
                index += 2;
            }
            '}' if chars.get(index + 1) == Some(&'}') => {
                literal.push('}');
                index += 2;
            }
            '{' => {
                let rest: String = chars[index + 1..].iter().collect();
                let Some(end) = rest.find('}') else {
                    return Err("The naming template has an unclosed '{'.".to_owned());
                };
                if !literal.is_empty() {
                    parts.push(Part::Literal(std::mem::take(&mut literal)));
                }
                let body: String = rest[..end]
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .collect();
                parts.push(parse_body(&body)?);
                index += 1 + end + 1;
            }
            '}' => return Err("The naming template has an unmatched '}'.".to_owned()),
            character => {
                literal.push(character);
                index += 1;
            }
        }
    }

    if !literal.is_empty() {
        parts.push(Part::Literal(literal));
    }
    Ok(parts)
}

fn parse_body(body: &str) -> Result<Part, String> {
    let (name, width) = match body.split_once(':') {
        Some((name, width)) => {
            if width.is_empty() || !width.chars().all(|character| character.is_ascii_digit()) {
                return Err(format!("Invalid width in token '{{{body}}}'."));
            }
            let width = width
                .parse::<usize>()
                .map_err(|_| format!("Invalid width in token '{{{body}}}'."))?;
            if width > 9 {
                return Err(format!("Width in token '{{{body}}}' is too large."));
            }
            (name, Some(width))
        }
        None => (body, None),
    };

    let token = match name {
        "track" => Token::Track,
        "disc" => Token::Disc,
        "title" => Token::Title,
        "artist" => Token::Artist,
        "album" => Token::Album,
        "album_artist" => Token::AlbumArtist,
        "year" => Token::Year,
        "ext" => Token::Ext,
        _ => return Err(format!("Unknown token '{{{body}}}'.")),
    };
    if width.is_some() && !matches!(token, Token::Track | Token::Disc) {
        return Err(format!(
            "Only track and disc tokens accept a width: '{{{body}}}'."
        ));
    }
    Ok(Part::Token(token, width))
}

pub fn unique_suffix(path: &Path, number: u32) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("track");
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("mka");
    parent.join(format!("{stem} ({number}).{extension}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> TemplateContext {
        TemplateContext {
            track: Some(3),
            disc: Some(1),
            title: "Song: Part 2?".to_owned(),
            artist: "Artist".to_owned(),
            album: "Album".to_owned(),
            album_artist: "Album Artist".to_owned(),
            year: "2026".to_owned(),
            ext: "mka".to_owned(),
        }
    }

    #[test]
    fn renders_padded_tokens() {
        assert_eq!(
            render("{track:02} - {title} (5.1).mka", &context()).unwrap(),
            "03 - Song_ Part 2_ (5.1).mka"
        );
    }

    #[test]
    fn adds_mka_extension() {
        assert_eq!(
            render("{track:02} - {title}", &context()).unwrap(),
            "03 - Song_ Part 2_.mka"
        );
    }

    #[test]
    fn supports_directory_segments() {
        assert_eq!(
            render(
                "{album_artist}/{album}/{track:02} - {title}.mka",
                &context()
            )
            .unwrap(),
            "Album Artist/Album/03 - Song_ Part 2_.mka"
        );
    }

    #[test]
    fn rejects_unknown_tokens() {
        assert!(render("{track:02} - {nope}.mka", &context()).is_err());
    }

    #[test]
    fn rejects_empty_segments() {
        assert!(validate("{album}//{title}.mka").is_err());
    }

    #[test]
    fn sanitizes_reserved_names() {
        assert_eq!(sanitize_component("CON.mka"), "_CON.mka");
        assert_eq!(sanitize_component("CON.foo.bar"), "_CON.foo.bar");
        assert_eq!(sanitize_component("LPT9.tar.gz"), "_LPT9.tar.gz");
        assert_eq!(sanitize_component("CONIN$.mka"), "_CONIN$.mka");
        assert_eq!(sanitize_component("trailing..."), "trailing");
    }

    #[test]
    fn replaces_every_windows_illegal_character() {
        assert_eq!(
            sanitize_component("a:b/c\\d?e*f\"g<h>i|j.mka"),
            "a_b_c_d_e_f_g_h_i_j.mka"
        );
    }

    #[test]
    fn unique_suffix_inserts_before_extension() {
        assert_eq!(
            unique_suffix(Path::new("/music/03 - Song.mka"), 2),
            PathBuf::from("/music/03 - Song (2).mka")
        );
    }
}
