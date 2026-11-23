//! Filename normalization, fuzzy pairing, and deterministic one-to-one
//! assignment between stereo masters and 5.1 Matroska targets.

use std::collections::{HashMap, HashSet};

use strsim::jaro_winkler;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::model::{MatchReason, Pairing, ProbeStatus, SourceEntry, TargetEntry};

const MIN_CANDIDATE_SCORE: f32 = 0.50;
const AUTO_PAIR_SCORE: f32 = 0.66;
const AMBIGUITY_MARGIN: f32 = 0.06;
const VERY_HIGH_CONFIDENCE: f32 = 0.98;

#[derive(Clone, Debug)]
pub struct PairCandidate {
    pub source_id: u64,
    pub target_id: u64,
    pub score: f32,
    pub reasons: Vec<MatchReason>,
}

pub fn normalize_stem(input: &str) -> String {
    let stripped = strip_technical_groups(input);
    let decomposed: String = stripped
        .nfkd()
        .filter(|character| !is_combining_mark(*character))
        .collect();
    let mut text = decomposed.to_ascii_lowercase();
    text = text.replace('&', " and ");
    text = text.replace("5.1", " ");
    text = text.replace("5 1", " ");
    text = text.replace("e-ac-3", " ");
    text = text.replace("e-ac3", " ");
    text = text.replace("e ac3", " ");
    text = text.replace("eac3", " ");

    let mut tokens = Vec::new();
    for token in text.split(|character: char| !character.is_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        if matches!(
            token,
            "surround"
                | "eac3"
                | "ac3"
                | "dts"
                | "mka"
                | "mkv"
                | "flac"
                | "remux"
                | "kbps"
                | "640kbps"
                | "24bit"
                | "48khz"
                | "stereo"
        ) {
            continue;
        }
        tokens.push(token);
    }
    tokens.join(" ")
}

fn strip_technical_groups(input: &str) -> String {
    let mut output = String::new();
    let mut group = String::new();
    let mut depth = 0u32;

    for character in input.chars() {
        match character {
            '(' | '[' | '{' => {
                if depth == 0 {
                    group.clear();
                }
                depth += 1;
                group.push(character);
            }
            ')' | ']' | '}' if depth > 0 => {
                group.push(character);
                depth -= 1;
                if depth == 0 && !is_technical_group(&group) {
                    output.push_str(&group);
                }
            }
            _ if depth > 0 => group.push(character),
            _ => output.push(character),
        }
    }
    if depth > 0 {
        output.push_str(&group);
    }
    output
}

fn is_technical_group(group: &str) -> bool {
    const TECHNICAL: &[&str] = &[
        "5", "1", "3", "51", "5.1", "e", "ac", "ac3", "eac3", "e-ac-3", "e-ac3", "dts", "surround",
        "640", "640kbps", "kbps", "mka", "mkv", "flac", "remux", "24bit", "48khz", "stereo", "aac",
        "opus",
    ];

    let inner = group
        .trim_matches(|character: char| matches!(character, '(' | ')' | '[' | ']' | '{' | '}'));
    let mut saw_token = false;
    for token in inner.split(|character: char| {
        !character.is_alphanumeric() && character != '.' && character != '-'
    }) {
        if token.is_empty() {
            continue;
        }
        saw_token = true;
        if !TECHNICAL.contains(&token.to_ascii_lowercase().as_str()) {
            return false;
        }
    }
    saw_token
}

fn leading_number(text: &str) -> Option<(u32, usize)> {
    let mut digits = 0usize;
    let mut value = 0u32;
    for (index, character) in text.char_indices() {
        if let Some(digit) = character.to_digit(10) {
            digits += 1;
            if digits > 3 {
                return None;
            }
            value = value * 10 + digit;
        } else {
            break;
        }
        if index + character.len_utf8() == text.len() {
            break;
        }
    }
    if digits == 0 || value == 0 {
        None
    } else {
        Some((value, digits))
    }
}

pub fn extract_track_number(stem: &str) -> Option<u32> {
    let text = stem.trim_start();
    let lower = text.to_ascii_lowercase();

    if let Some(stripped) = text.strip_prefix('[') {
        if let Some(end) = stripped.find(']') {
            let inner = stripped[..end].trim();
            if let Some((number, digits)) = leading_number(inner) {
                if digits == inner.len() {
                    return Some(number);
                }
            }
        }
    }
    if let Some(stripped) = text.strip_prefix('(') {
        if let Some(end) = stripped.find(')') {
            let inner = stripped[..end].trim();
            if let Some((number, digits)) = leading_number(inner) {
                if digits == inner.len() {
                    return Some(number);
                }
            }
        }
    }

    if lower.starts_with("disc") {
        let rest = text[4..].trim_start();
        if let Some((_, digits)) = leading_number(rest) {
            let after_disc = rest[digits..]
                .trim_start()
                .trim_start_matches(|character: char| !character.is_ascii_digit());
            if let Some((track, _)) = leading_number(after_disc) {
                return Some(track);
            }
        }
    }

    for prefix in ["track", "song"] {
        if lower.starts_with(prefix) {
            let rest = text[prefix.len()..]
                .trim_start()
                .trim_start_matches(|character: char| {
                    matches!(character, '-' | '.' | '#' | ':' | '_')
                })
                .trim_start();
            if let Some((number, digits)) = leading_number(rest) {
                let after = &rest[digits..];
                if after.is_empty()
                    || after.starts_with(' ')
                    || after.starts_with('-')
                    || after.starts_with('.')
                {
                    return Some(number);
                }
            }
        }
    }

    let (first, digits) = leading_number(text)?;
    let rest = &text[digits..];
    if let Some(stripped) = rest.strip_prefix('_') {
        if let Some((second, _)) = leading_number(stripped.trim_start()) {
            return Some(second);
        }
    }
    if let Some(stripped) = rest.strip_prefix('.') {
        if let Some((second, digits2)) = leading_number(stripped) {
            let after = &stripped[digits2..];
            if after.is_empty() || after.starts_with(' ') || after.starts_with('-') {
                return Some(second);
            }
        }
    }
    if rest.is_empty()
        || rest.starts_with(' ')
        || rest.starts_with('-')
        || rest.starts_with('.')
        || rest.starts_with(')')
        || rest.starts_with('_')
    {
        Some(first)
    } else {
        None
    }
}

fn similarity(left: &str, right: &str) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    if left == right {
        return 1.0;
    }
    jaro_winkler(left, right)
}

pub fn name_similarity(left: &str, right: &str) -> f64 {
    similarity(&normalize_stem(left), &normalize_stem(right))
}

pub fn score_pair(source: &SourceEntry, target: &TargetEntry) -> PairCandidate {
    let source_stem = source
        .path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let target_stem = target
        .path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();

    let normalized_source = normalize_stem(source_stem);
    let normalized_target = normalize_stem(target_stem);
    let exact_stem = !normalized_source.is_empty() && normalized_source == normalized_target;

    let normalized_source_title = normalize_stem(&source.title);
    let normalized_target_title = normalize_stem(&target.display_title());
    let exact_title =
        !normalized_source_title.is_empty() && normalized_source_title == normalized_target_title;

    let stem_similarity = similarity(&normalized_source, &normalized_target);
    let title_similarity = similarity(&normalized_source_title, &normalized_target_title);
    let name_score = stem_similarity.max(title_similarity * 0.98);

    let source_track = source
        .track_number
        .or_else(|| extract_track_number(source_stem));
    let target_track = target
        .track_number
        .or_else(|| extract_track_number(target_stem));

    let track_score = match (source_track, target_track) {
        (Some(left), Some(right)) if left == right => 1.0,
        (Some(_), Some(_)) => 0.0,
        _ => 0.5,
    };

    let (duration_score, duration_delta) = match (source.duration_secs, target.duration_secs) {
        (Some(left), Some(right)) => {
            let delta = (left - right).abs();
            let tolerance = (right.abs() * 0.005).max(2.0);
            ((1.0 - (delta / tolerance)).clamp(0.0, 1.0), Some(delta))
        }
        _ => (0.5, None),
    };

    let mut score = (0.55 * name_score + 0.25 * track_score + 0.20 * duration_score) as f32;
    let mut reasons = Vec::new();

    if exact_stem {
        reasons.push(MatchReason::ExactStem);
        score = score.max(0.98);
    }
    if exact_title {
        score = score.max(0.97);
    }
    if let (Some(left), Some(right)) = (source_track, target_track) {
        if left == right {
            reasons.push(MatchReason::TrackNumber);
        }
    }
    reasons.push(MatchReason::NameSimilarity(
        (name_score * 100.0).round() as u32
    ));
    if let Some(delta) = duration_delta {
        reasons.push(MatchReason::DurationDelta(delta));
    }

    if let (Some(left), Some(right)) = (source_track, target_track) {
        if left != right {
            score -= 0.15;
            // Track numbers are strong hints, but an exact title is stronger
            // for stitched albums where one target replaces two sources.
            if name_score < 0.80 && title_similarity < 0.80 {
                score = score.min(0.55);
            }
        }
    }
    if let Some(delta) = duration_delta {
        if delta > 15.0 {
            score = score.min(if name_score >= 0.90 { 0.80 } else { 0.50 });
        } else if name_score >= 0.95 && delta <= 2.0 {
            score = score.max(0.99);
        }
    }

    PairCandidate {
        source_id: source.id,
        target_id: target.id,
        score: score.clamp(0.0, 1.0),
        reasons,
    }
}

/// Detect the common stitched-track shape: one target whose duration is the
/// sum of two consecutive sources and whose title includes both source titles.
/// The target is intentionally left for manual pairing rather than being
/// force-matched to one of the two adjacent tracks.
pub fn combined_track_hint(sources: &[SourceEntry], target: &TargetEntry) -> Option<(u64, u64)> {
    let target_duration = target.duration_secs?;
    let target_title = normalize_stem(&target.display_title());
    if target_title.is_empty() {
        return None;
    }

    let mut ordered: Vec<&SourceEntry> = sources
        .iter()
        .filter(|source| source.status == ProbeStatus::Ready)
        .collect();
    ordered.sort_by_key(|source| (source.track_number.unwrap_or(u32::MAX), source.path.clone()));

    for pair in ordered.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        let (Some(left_duration), Some(right_duration)) = (left.duration_secs, right.duration_secs)
        else {
            continue;
        };
        let sum = left_duration + right_duration;
        let tolerance = (target_duration.abs() * 0.01).max(2.0);
        if (sum - target_duration).abs() > tolerance {
            continue;
        }
        let left_title = normalize_stem(&left.title);
        let right_title = normalize_stem(&right.title);
        if left_title.is_empty() || right_title.is_empty() {
            continue;
        }
        let contains_both =
            target_title.contains(&left_title) && target_title.contains(&right_title);
        let similar_both = similarity(&target_title, &left_title) >= 0.72
            && similarity(&target_title, &right_title) >= 0.72;
        if contains_both || similar_both {
            return Some((left.id, right.id));
        }
    }
    None
}

pub fn auto_pair(
    sources: &[SourceEntry],
    targets: &[TargetEntry],
    existing: &[Pairing],
) -> Vec<Pairing> {
    let mut pairings = existing
        .iter()
        .filter(|pair| pair.locked)
        .cloned()
        .collect::<Vec<_>>();

    let mut used_sources: HashSet<u64> = pairings.iter().map(|pair| pair.source_id).collect();
    let mut used_targets: HashSet<u64> = pairings.iter().map(|pair| pair.target_id).collect();

    // Leave stitched targets for an explicit decision; auto-pairing one half
    // of a combined track is worse than leaving it unmatched.
    let combined_targets: HashSet<u64> = targets
        .iter()
        .filter(|target| target.status == ProbeStatus::Ready)
        .filter(|target| combined_track_hint(sources, target).is_some())
        .map(|target| target.id)
        .collect();

    let mut candidates = Vec::new();
    for source in sources {
        if source.status != ProbeStatus::Ready {
            continue;
        }
        for target in targets {
            if target.status != ProbeStatus::Ready || combined_targets.contains(&target.id) {
                continue;
            }
            let candidate = score_pair(source, target);
            if candidate.score >= MIN_CANDIDATE_SCORE {
                candidates.push(candidate);
            }
        }
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let left_track = left.reasons.contains(&MatchReason::TrackNumber);
                let right_track = right.reasons.contains(&MatchReason::TrackNumber);
                right_track.cmp(&left_track)
            })
            .then_with(|| {
                duration_delta(left)
                    .partial_cmp(&duration_delta(right))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut best_by_source: HashMap<u64, (f32, f32)> = HashMap::new();
    let mut best_by_target: HashMap<u64, (f32, f32)> = HashMap::new();
    for candidate in &candidates {
        record_best(&mut best_by_source, candidate.source_id, candidate.score);
        record_best(&mut best_by_target, candidate.target_id, candidate.score);
    }

    for candidate in candidates {
        if candidate.score < AUTO_PAIR_SCORE {
            continue;
        }
        if used_sources.contains(&candidate.source_id)
            || used_targets.contains(&candidate.target_id)
        {
            continue;
        }
        if !is_unique_best(&best_by_source, candidate.source_id, candidate.score)
            || !is_unique_best(&best_by_target, candidate.target_id, candidate.score)
        {
            continue;
        }
        used_sources.insert(candidate.source_id);
        used_targets.insert(candidate.target_id);
        pairings.push(Pairing {
            source_id: candidate.source_id,
            target_id: candidate.target_id,
            confidence: candidate.score,
            reasons: candidate.reasons,
            locked: false,
        });
    }

    pairings.sort_by_key(|pair| pair.source_id);
    pairings
}

fn duration_delta(candidate: &PairCandidate) -> f64 {
    candidate
        .reasons
        .iter()
        .find_map(|reason| match reason {
            MatchReason::DurationDelta(delta) => Some(*delta),
            _ => None,
        })
        .unwrap_or(f64::MAX)
}

fn record_best(map: &mut HashMap<u64, (f32, f32)>, id: u64, score: f32) {
    let entry = map.entry(id).or_insert((f32::MIN, f32::MIN));
    if score > entry.0 {
        entry.1 = entry.0;
        entry.0 = score;
    } else if score > entry.1 {
        entry.1 = score;
    }
}

fn is_unique_best(map: &HashMap<u64, (f32, f32)>, id: u64, score: f32) -> bool {
    let Some(&(best, second)) = map.get(&id) else {
        return false;
    };
    if (score - best).abs() > f32::EPSILON {
        return false;
    }
    if (best - second).abs() <= 0.001 {
        return false;
    }
    score >= VERY_HIGH_CONFIDENCE || best - second >= AMBIGUITY_MARGIN
}

pub fn confidence_label(score: f32) -> &'static str {
    if score >= 0.85 {
        "High"
    } else if score >= 0.65 {
        "Medium"
    } else {
        "Low"
    }
}

pub fn format_duration(seconds: Option<f64>) -> String {
    let Some(seconds) = seconds else {
        return "--:--".to_owned();
    };
    let total = seconds.max(0.0).round() as u64;
    let minutes = total / 60;
    let seconds = total % 60;
    if minutes >= 60 {
        format!("{}:{:02}:{:02}", minutes / 60, minutes % 60, seconds)
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProbeStatus, SourceEntry, TargetEntry};
    use std::path::PathBuf;

    fn source(id: u64, name: &str, track: Option<u32>, duration: f64) -> SourceEntry {
        let mut source = SourceEntry::new(id, PathBuf::from(name));
        source.track_number = track;
        source.duration_secs = Some(duration);
        source.title = name.to_owned();
        source.status = ProbeStatus::Ready;
        source
    }

    fn target(id: u64, name: &str, track: Option<u32>, duration: f64) -> TargetEntry {
        let mut target = TargetEntry::new(id, PathBuf::from(name));
        target.track_number = track;
        target.duration_secs = Some(duration);
        target.status = ProbeStatus::Ready;
        target
    }

    #[test]
    fn normalizes_technical_suffixes() {
        assert_eq!(normalize_stem("01 - Miku (5.1 E-AC-3 640 kbps)"), "01 miku");
        assert_eq!(normalize_stem("Café & Bass [Surround]"), "cafe and bass");
        assert_eq!(normalize_stem("Song (Live 5.1)"), "song live");
    }

    #[test]
    fn extracts_track_numbers() {
        assert_eq!(extract_track_number("01 - Title"), Some(1));
        assert_eq!(extract_track_number("1_08_Hate The Other Side"), Some(8));
        assert_eq!(extract_track_number("Disc 2 - 04 Song"), Some(4));
        assert_eq!(extract_track_number("[07] Title"), Some(7));
        assert_eq!(extract_track_number("Track 09 - Title"), Some(9));
        assert_eq!(extract_track_number("1.02 Title"), Some(2));
        assert_eq!(extract_track_number("No number here"), None);
    }

    #[test]
    fn pairs_exact_names_with_duration() {
        let sources = vec![source(1, "01 - Song.flac", Some(1), 182.2)];
        let targets = vec![target(10, "01 - Song (5.1).mka", Some(1), 182.2)];
        let pairs = auto_pair(&sources, &targets, &[]);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].source_id, 1);
        assert_eq!(pairs[0].target_id, 10);
        assert!(pairs[0].confidence >= 0.85);
    }

    #[test]
    fn rejects_different_track_numbers() {
        let sources = [source(1, "01 - Song.flac", Some(1), 100.0)];
        let targets = [target(10, "02 - Other (5.1).mka", Some(2), 100.0)];
        let candidate = score_pair(&sources[0], &targets[0]);
        assert!(candidate.score <= 0.55);
    }

    #[test]
    fn assignment_is_one_to_one() {
        let sources = vec![
            source(1, "01 - Same.flac", Some(1), 100.0),
            source(2, "02 - Same.flac", Some(2), 100.0),
        ];
        let targets = vec![
            target(10, "01 - Same (5.1).mka", Some(1), 100.0),
            target(11, "02 - Same (5.1).mka", Some(2), 100.0),
        ];
        let pairs = auto_pair(&sources, &targets, &[]);
        assert_eq!(pairs.len(), 2);
        assert_ne!(pairs[0].target_id, pairs[1].target_id);
    }

    #[test]
    fn exact_name_with_duration_gap_is_a_medium_suggestion() {
        let sources = [source(1, "SurroundPacker 5.1 Mix.flac", None, 165.8)];
        let targets = [target(
            10,
            "SurroundPacker 5.1 Mix - E-AC3.mka",
            None,
            182.2,
        )];
        let candidate = score_pair(&sources[0], &targets[0]);
        assert!((0.65..0.85).contains(&candidate.score));
        assert_eq!(auto_pair(&sources, &targets, &[]).len(), 1);
    }

    #[test]
    fn stitched_tracks_do_not_mispair_adjacent_sources() {
        let mut alpha = source(1, "05 - Alpha.flac", Some(5), 100.0);
        alpha.title = "Alpha".to_owned();
        let mut beta = source(2, "06 - Beta.flac", Some(6), 50.0);
        beta.title = "Beta".to_owned();
        let mut gamma = source(3, "07 - Gamma.flac", Some(7), 120.0);
        gamma.title = "Gamma".to_owned();
        let sources = vec![alpha, beta, gamma];
        let targets = vec![
            target(10, "05 - Alpha + Beta (5.1).mka", Some(5), 150.0),
            target(11, "07 - Gamma (5.1).mka", Some(7), 120.0),
        ];
        assert_eq!(combined_track_hint(&sources, &targets[0]), Some((1, 2)));
        let pairs = auto_pair(&sources, &targets, &[]);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].source_id, 3);
        assert_eq!(pairs[0].target_id, 11);
    }

    #[test]
    fn ambiguous_adjacent_titles_are_not_auto_paired() {
        let mut left = source(1, "Theme.flac", None, 100.0);
        left.title = "Main Theme".to_owned();
        let mut right = source(2, "Theme (Copy).flac", None, 100.0);
        right.title = "Main Theme".to_owned();
        let target = target(10, "Main Theme (5.1).mka", None, 100.0);
        let pairs = auto_pair(&[left, right], &[target], &[]);
        // Both sources are plausible; the margin rule should prevent a guess.
        assert!(pairs.is_empty());
    }

    #[test]
    fn locked_pairs_are_preserved() {
        let sources = [source(1, "01 - Song.flac", Some(1), 100.0)];
        let targets = [target(10, "99 - Other.mka", Some(99), 100.0)];
        let locked = vec![Pairing {
            source_id: 1,
            target_id: 10,
            confidence: 0.2,
            reasons: vec![MatchReason::Locked],
            locked: true,
        }];
        let pairs = auto_pair(&sources, &targets, &locked);
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].locked);
    }
}
