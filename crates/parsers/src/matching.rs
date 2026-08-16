//! Pure title matching helpers shared by download sources.

use crate::extract_volumes;

/// The volumes a release covers after its title was matched to a series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTitleMatch {
    pub matched_missing_volumes: Vec<i32>,
    /// All explicit volumes found in the title. Empty for integral releases.
    pub all_volumes: Vec<i32>,
    pub is_integral: bool,
    pub confidence: MatchConfidence,
    pub reasons: Vec<MatchReason>,
}

/// Confidence assigned to a title match before any source-specific policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchConfidence {
    High,
    Review,
}

impl MatchConfidence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Review => "review",
        }
    }
}

/// Evidence used when qualifying a release title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchReason {
    SeriesTitle,
    ExplicitVolume,
    IntegralEdition,
    ShortSeriesTitle,
    AmbiguousVolumeNumber,
}

impl MatchReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SeriesTitle => "series_title",
            Self::ExplicitVolume => "explicit_volume",
            Self::IntegralEdition => "integral_edition",
            Self::ShortSeriesTitle => "short_series_title",
            Self::AmbiguousVolumeNumber => "ambiguous_volume_number",
        }
    }
}

/// Fold common Latin diacritics while preserving case, punctuation and the
/// structure of the original text. Non-Latin characters are kept unchanged.
pub fn fold_accents(value: &str) -> String {
    let mut folded = String::with_capacity(value.len());

    for c in value.chars() {
        let replacement = match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' | 'À' | 'Á' | 'Â' | 'Ä' | 'Ã' | 'Å' => {
                "a"
            }
            'ç' | 'Ç' => "c",
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => "e",
            'î' | 'ï' | 'í' | 'ì' | 'Î' | 'Ï' | 'Í' | 'Ì' => "i",
            'ñ' | 'Ñ' => "n",
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' | 'Ô' | 'Ö' | 'Ó' | 'Ò' | 'Õ' => "o",
            'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' | 'Ú' => "u",
            'ÿ' | 'ý' | 'Ÿ' | 'Ý' => "y",
            'æ' | 'Æ' => "ae",
            'œ' | 'Œ' => "oe",
            _ => {
                folded.push(c);
                continue;
            }
        };
        if c.is_uppercase() {
            folded.extend(replacement.chars().flat_map(char::to_uppercase));
        } else {
            folded.push_str(replacement);
        }
    }

    folded
}

/// Normalize a title for comparison: lowercase, accents stripped and every
/// separator collapsed to a single space.
pub fn normalize_title(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_was_space = true;

    for c in fold_accents(value).chars() {
        if c.is_alphanumeric() {
            if c.is_ascii() {
                normalized.push(c.to_ascii_lowercase());
            } else {
                normalized.extend(c.to_lowercase());
            }
            previous_was_space = false;
            continue;
        }
        if !previous_was_space {
            normalized.push(' ');
            previous_was_space = true;
        }
    }

    normalized.trim().to_string()
}

/// True when the complete normalized series name appears as a sequence of
/// words in the candidate title. This rejects substring matches such as
/// `Hotel` in `Hotelier`.
pub fn title_matches_series(candidate_title: &str, series_name: &str) -> bool {
    let title = normalize_title(candidate_title);
    let series = normalize_title(series_name);
    if title.is_empty() || series.is_empty() {
        return false;
    }

    let title_words: Vec<&str> = title.split_whitespace().collect();
    let series_words: Vec<&str> = series.split_whitespace().collect();
    title_words
        .windows(series_words.len())
        .any(|window| window == series_words.as_slice())
}

/// Returns true if a title denotes a complete or integral edition.
pub fn is_integral_release(title: &str) -> bool {
    normalize_title(title)
        .split_whitespace()
        .any(|word| matches!(word, "integrale" | "integral" | "complet" | "complete"))
}

/// Match a title's volumes against missing volumes without checking the series.
/// Use this for manual search results; automatic detection should call
/// [`match_release_title`] instead.
pub fn match_title_volumes(title: &str, missing_volumes: &[i32]) -> (Vec<i32>, Vec<i32>) {
    let all_volumes = extract_volumes(title);
    let is_integral = is_integral_release(title);
    let matched_missing_volumes = if is_integral && !missing_volumes.is_empty() {
        missing_volumes.to_vec()
    } else {
        all_volumes
            .iter()
            .copied()
            .filter(|volume| missing_volumes.contains(volume))
            .collect()
    };

    let all_volumes = if is_integral { vec![] } else { all_volumes };
    (matched_missing_volumes, all_volumes)
}

/// Match a release title to both a series and its missing volumes.
/// A common volume number alone is not enough for automatic detection.
pub fn match_release_title(
    candidate_title: &str,
    series_name: &str,
    missing_volumes: &[i32],
) -> Option<ReleaseTitleMatch> {
    if !title_matches_series(candidate_title, series_name) {
        return None;
    }

    let (matched_missing_volumes, all_volumes) =
        match_title_volumes(candidate_title, missing_volumes);
    if matched_missing_volumes.is_empty() {
        return None;
    }

    Some(ReleaseTitleMatch {
        matched_missing_volumes,
        all_volumes,
        is_integral: is_integral_release(candidate_title),
        confidence: match_confidence(candidate_title, series_name),
        reasons: match_reasons(candidate_title, series_name),
    })
}

fn match_confidence(candidate_title: &str, series_name: &str) -> MatchConfidence {
    if normalize_title(series_name).split_whitespace().count() == 1
        && normalize_title(series_name).chars().count() <= 4
        || !has_explicit_volume_marker(candidate_title) && !is_integral_release(candidate_title)
    {
        MatchConfidence::Review
    } else {
        MatchConfidence::High
    }
}

fn match_reasons(candidate_title: &str, series_name: &str) -> Vec<MatchReason> {
    let mut reasons = vec![MatchReason::SeriesTitle];
    if is_integral_release(candidate_title) {
        reasons.push(MatchReason::IntegralEdition);
    } else if has_explicit_volume_marker(candidate_title) {
        reasons.push(MatchReason::ExplicitVolume);
    } else {
        reasons.push(MatchReason::AmbiguousVolumeNumber);
    }
    if normalize_title(series_name).split_whitespace().count() == 1
        && normalize_title(series_name).chars().count() <= 4
    {
        reasons.push(MatchReason::ShortSeriesTitle);
    }
    reasons
}

fn has_explicit_volume_marker(title: &str) -> bool {
    let normalized = normalize_title(title);
    normalized.split_whitespace().any(|word| {
        matches!(
            word,
            "tome" | "vol" | "volume" | "chapitre" | "chapter" | "ch"
        ) || (word.starts_with('t') && word[1..].chars().all(|c| c.is_ascii_digit()))
            || (word.starts_with("vol") && word[3..].chars().all(|c| c.is_ascii_digit()))
    })
}

/// Extract the likely series name from a Prowlarr-style release title.
///
/// This deliberately preserves the original casing for display and database
/// lookups; use [`normalize_title`] when comparing the resulting value.
pub fn extract_series_name_from_release(title: &str) -> String {
    // Dot-separated NRC-style: "Series.Name.T31.Author.Year.FR.[CBZ]-NRC".
    let dot_count = title.chars().filter(|&c| c == '.').count();
    let space_count = title.chars().filter(|&c| c == ' ').count();
    if dot_count > space_count {
        let parts: Vec<&str> = title.split('.').collect();
        let mut end_idx = parts.len();
        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                continue;
            }
            let upper = part.to_uppercase();
            let is_combined_volume = (upper.starts_with('T')
                && upper.len() >= 2
                && upper[1..].chars().all(|c| c.is_ascii_digit()))
                || ["VOL", "TOME", "VOLUME"].iter().any(|prefix| {
                    upper.starts_with(prefix)
                        && upper.len() > prefix.len()
                        && upper[prefix.len()..].chars().all(|c| c.is_ascii_digit())
                });
            let is_standalone_volume =
                matches!(upper.as_str(), "TOME" | "TOMES" | "VOL" | "VOLS" | "VOLUME");
            let is_year = upper.len() == 4
                && (upper.starts_with("19") || upper.starts_with("20"))
                && upper.chars().all(|c| c.is_ascii_digit());
            let is_tag = matches!(
                upper.as_str(),
                "FR" | "EN" | "JP" | "VF" | "VO" | "FRENCH" | "CBZ" | "CBR" | "PDF" | "EPUB"
            ) || upper.starts_with('[');
            if is_combined_volume || is_standalone_volume || is_year || is_tag {
                end_idx = i;
                break;
            }
        }
        return parts[..end_idx].join(" ");
    }

    let lower = title.to_lowercase();
    let separators = [
        " - bd ",
        " - tome ",
        " - t0",
        " - t1",
        " - t2",
        " - t3",
        " - t4",
        " - t5",
        " - t6",
        " - t7",
        " - t8",
        " - t9",
        " -bd ",
        " tome ",
        " vol.",
        " vol ",
        " [",
        " (",
        " intégrale",
        " integrale",
        " complet",
    ];
    let mut best_pos = title.len();
    for separator in separators {
        if let Some(pos) = lower.find(separator) {
            if pos > 0 && pos < best_pos {
                best_pos = pos;
            }
        }
    }
    title[..best_pos].trim().to_string()
}

/// Extract a series name from a Telegram book filename.
///
/// Telegram filenames often add a channel handle and use chapter markers that
/// are not present in Prowlarr release titles.
pub fn extract_series_name_from_filename(filename: &str) -> String {
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(filename);
    let stem = stem
        .rfind('@')
        .filter(|&pos| !stem[pos + 1..].is_empty() && !stem[pos + 1..].contains(' '))
        .map_or(stem, |pos| stem[..pos].trim_end());
    let normalized = stem.replace('_', " ");
    let lower = normalized.to_lowercase();

    let named_markers = [
        " - intégrale",
        " - integrale",
        " - hors-série",
        " - hors série",
        " - hors-serie",
        " - tome ",
        " - volume ",
        " - vol. ",
        " - vol ",
        " - chapter ",
        " - chapitre ",
        " - chap. ",
        " - chap ",
        " - ch. ",
        " - t. ",
        " - t ",
        " intégrale",
        " integrale",
        " hors-série",
        " hors série",
        " tome ",
        " volume ",
        " vol. ",
        " vol ",
        " chapitre ",
    ];
    let mut cut = named_markers
        .iter()
        .filter_map(|marker| lower.find(marker).filter(|&pos| pos > 0))
        .min()
        .unwrap_or(normalized.len());

    let bytes = lower.as_bytes();
    for i in 1..bytes.len() {
        let starts_number = bytes[i].is_ascii_digit() && i >= 3 && &bytes[i - 3..i] == b" - ";
        let marker = if bytes[i] == b'#' {
            bytes.get(i + 1).is_some_and(|c| c.is_ascii_digit())
                || (bytes.get(i + 1) == Some(&b'c') && bytes.get(i + 2) == Some(&b'h'))
        } else if matches!(bytes[i], b't' | b'v') {
            bytes.get(i + 1).is_some_and(|c| c.is_ascii_digit())
                || (matches!(bytes.get(i + 1), Some(b'.' | b' '))
                    && bytes.get(i + 2).is_some_and(|c| c.is_ascii_digit()))
        } else if bytes[i] == b'c' {
            bytes.get(i + 1) == Some(&b'h') && bytes.get(i + 2).is_some_and(|c| c.is_ascii_digit())
        } else {
            false
        };
        if starts_number {
            cut = cut.min(i - 3);
        } else if marker && i > 0 && bytes[i - 1] == b' ' {
            cut = cut.min(i - 1);
        } else if marker && i >= 2 && &bytes[i - 2..i] == b" -" {
            cut = cut.min(i - 2);
        }
    }

    if cut == normalized.len() {
        if let Some(pos) = lower.rfind(' ') {
            let suffix = &lower[pos + 1..];
            if pos >= 3
                && !suffix.is_empty()
                && suffix.len() <= 3
                && suffix.bytes().all(|c| c.is_ascii_digit())
            {
                cut = pos;
            }
        }
    }
    normalized[..cut]
        .trim_end_matches([' ', '-', '_', '.'])
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_accents_and_separators() {
        assert_eq!(normalize_title("L’Île-de minuit"), "l ile de minuit");
    }

    #[test]
    fn folds_accents_without_changing_title_structure() {
        assert_eq!(
            fold_accents("L'Île et L’Œuvre — sous_le  vent"),
            "L'Ile et L’OEuvre — sous_le  vent"
        );
        assert_eq!(fold_accents("cœur æther"), "coeur aether");
        assert_eq!(fold_accents("東京"), "東京");
    }

    #[test]
    fn matches_complete_series_words_only() {
        assert!(title_matches_series(
            "Les.Legendaires.T05",
            "Les Légendaires"
        ));
        assert!(title_matches_series("L.Incal.T01", "L'Incal"));
        assert!(!title_matches_series("Hotelier T01", "Hotel"));
        assert!(!title_matches_series("Naruto T05", "One Piece"));
    }

    #[test]
    fn rejects_same_volume_from_another_series() {
        assert_eq!(match_release_title("Naruto T05", "One Piece", &[5]), None);
    }

    #[test]
    fn matches_explicit_volume_after_series() {
        let matched = match_release_title("One Piece T05", "One Piece", &[3, 5, 7]).unwrap();
        assert_eq!(matched.matched_missing_volumes, vec![5]);
        assert_eq!(matched.all_volumes, vec![5]);
        assert!(!matched.is_integral);
    }

    #[test]
    fn matches_only_missing_volumes_in_a_range() {
        let matched =
            match_release_title("Dragon Ball T01-T10", "Dragon Ball", &[5, 8, 15]).unwrap();
        assert_eq!(matched.matched_missing_volumes, vec![5, 8]);
        assert_eq!(matched.all_volumes, (1..=10).collect::<Vec<_>>());
    }

    #[test]
    fn rejects_a_generic_title_from_another_series() {
        assert_eq!(match_release_title("Saga T05", "One Piece", &[5]), None);
    }

    #[test]
    fn marks_short_series_and_bare_numbers_for_review() {
        let matched = match_release_title("Saga 05", "Saga", &[5]).unwrap();
        assert_eq!(matched.confidence, MatchConfidence::Review);
        assert!(matched.reasons.contains(&MatchReason::ShortSeriesTitle));
        assert!(matched
            .reasons
            .contains(&MatchReason::AmbiguousVolumeNumber));
    }

    #[test]
    fn marks_short_series_with_explicit_volume_for_review() {
        let matched = match_release_title("Saga T05", "Saga", &[5]).unwrap();
        assert_eq!(matched.confidence, MatchConfidence::Review);
        assert!(matched.reasons.contains(&MatchReason::ShortSeriesTitle));
        assert!(matched.reasons.contains(&MatchReason::ExplicitVolume));
    }

    #[test]
    fn marks_explicit_and_integral_matches_as_high_confidence() {
        let explicit = match_release_title("One Piece T05", "One Piece", &[5]).unwrap();
        assert_eq!(explicit.confidence, MatchConfidence::High);
        assert_eq!(
            explicit.reasons,
            vec![MatchReason::SeriesTitle, MatchReason::ExplicitVolume]
        );

        let integral = match_release_title("One Piece Intégrale", "One Piece", &[1]).unwrap();
        assert_eq!(integral.confidence, MatchConfidence::High);
        assert_eq!(
            integral.reasons,
            vec![MatchReason::SeriesTitle, MatchReason::IntegralEdition]
        );
    }

    #[test]
    fn integral_requires_a_series_match() {
        let matched = match_release_title("One Piece Intégrale", "One Piece", &[1, 2, 3]).unwrap();
        assert_eq!(matched.matched_missing_volumes, vec![1, 2, 3]);
        assert!(matched.all_volumes.is_empty());
        assert!(matched.is_integral);
        assert_eq!(
            match_release_title("Naruto Intégrale", "One Piece", &[1, 2, 3]),
            None
        );
    }

    #[test]
    fn extracts_series_name_from_dot_separated_release() {
        assert_eq!(
            extract_series_name_from_release("Orcs.&.Gobelins.T31.Tren'gar.2025.FR.[CBZ]-NRC"),
            "Orcs & Gobelins"
        );
        assert_eq!(
            extract_series_name_from_release("One.Piece.Tome.[1 à 100].FR.[CBZ]-GRP"),
            "One Piece"
        );
    }

    #[test]
    fn extracts_series_name_from_space_separated_release() {
        assert_eq!(extract_series_name_from_release("Akira tome 1"), "Akira");
        assert_eq!(
            extract_series_name_from_release("Dragon Ball [CBZ]"),
            "Dragon Ball"
        );
    }

    #[test]
    fn extracts_series_name_from_telegram_filename() {
        assert_eq!(
            extract_series_name_from_filename("Boruto - Two Blue Vortex - #Ch03@BD_fr.cbz"),
            "Boruto - Two Blue Vortex"
        );
        assert_eq!(
            extract_series_name_from_filename("Berserk - 32@BD_fr.cbz"),
            "Berserk"
        );
    }
}
