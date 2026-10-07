use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

pub mod matching;
pub use matching::{
    extract_series_name_from_filename, extract_series_name_from_release, fold_accents,
    is_integral_release, match_release_title, match_title_volumes, normalize_title,
    title_matches_series, MatchConfidence, MatchReason, ReleaseTitleMatch,
};

/// Extract a volume number from an external metadata title when it has an
/// explicit marker (`Tome`, `T.`, `Vol`, `Volume` or `#`).
///
/// Unlike filename parsing, this deliberately does not infer a volume from a
/// bare trailing number: provider titles often end in an edition number or a
/// year.
pub fn extract_metadata_volume(title: &str) -> Option<i32> {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(?i)(?:tome|t\.|vol(?:ume)?\.?|#)\s*(\d+)")
            .expect("valid metadata volume regex")
    });
    re.captures(title)
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse().ok())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookFormat {
    Cbz,
    Cbr,
    Pdf,
    Epub,
}

impl BookFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Pdf => "pdf",
            Self::Epub => "epub",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeType {
    Regular,
    Hs,
    Oneshot,
    Integral,
}

impl VolumeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Hs => "hs",
            Self::Oneshot => "oneshot",
            Self::Integral => "integral",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedMetadata {
    pub title: String,
    pub series: Option<String>,
    pub volume: Option<i32>,
    pub volume_type: VolumeType,
    pub page_count: Option<i32>,
}

pub fn detect_format(path: &Path) -> Option<BookFormat> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "cbz" => Some(BookFormat::Cbz),
        "cbr" => Some(BookFormat::Cbr),
        "pdf" => Some(BookFormat::Pdf),
        "epub" => Some(BookFormat::Epub),
        _ => None,
    }
}

/// Internal representation shared by the public volume projections.
///
/// It keeps the source title alongside the deduplicated numbers so title
/// cleanup can use the same parsing entry point without affecting consumers
/// that only need volumes.
struct ParsedVolumeMarkers<'a> {
    title: &'a str,
    volumes: Vec<i32>,
}

/// Parse individual and range volume markers from a title or filename.
///
/// Handles individual volumes (T01, Tome 01, Vol. 01, v01, #01) and also
/// **range packs** like `T01.T15`, `[T001.T104]`, `T01-T15`, `Tome 01 à Tome 15`
/// — the range is expanded so every volume in [start..=end] is returned.
fn parse_volume_markers(title: &str) -> ParsedVolumeMarkers<'_> {
    let lower = title.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let mut volumes = Vec::new();

    // Pass 1 — range expansion: PREFIX NUMBER (SEP) PREFIX NUMBER
    // Separator: '.' | '-' | 'à'
    let mut i = 0;
    while i < chars.len() {
        if let Some((n1, after1)) = read_vol_prefix_number(&chars, i) {
            let mut j = after1;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            let after_sep = if j < chars.len() && (chars[j] == '.' || chars[j] == '-') {
                Some(j + 1)
            } else if j < chars.len() && chars[j] == '\u{00e0}' {
                // 'à' (U+00E0) — French "à" as in "Tome 01 à Tome 15"
                Some(j + 1)
            } else if j < chars.len()
                && chars[j] == 'a'
                && j > 0
                && chars[j - 1] == ' '
                && j + 1 < chars.len()
                && (chars[j + 1] == ' ' || chars[j + 1].is_ascii_digit())
            {
                // 'a' without accent — French "T01 a T34" (space before, space or digit after)
                Some(j + 1)
            } else {
                None
            };

            if let Some(sep_end) = after_sep {
                let mut k = sep_end;
                while k < chars.len() && chars[k] == ' ' {
                    k += 1;
                }
                // Try prefixed number first (T17-T23), then bare number (T17-23)
                let n2_result =
                    read_vol_prefix_number(&chars, k).or_else(|| read_bare_number(&chars, k));
                if let Some((n2, _)) = n2_result {
                    if n1 < n2 && n2 - n1 <= 500 {
                        for v in n1..=n2 {
                            if !volumes.contains(&v) {
                                volumes.push(v);
                            }
                        }
                        i = after1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    // Pass 2 — individual volumes not already captured by range expansion
    // Note: work entirely with char indices (not byte offsets) to avoid
    // mismatches when the title contains multi-byte UTF-8 characters.
    let prefixes: &[(&[char], bool)] = &[
        (&['v', 'o', 'l', 'u', 'm', 'e'], false),
        (&['t', 'o', 'm', 'e'], false),
        (&['v', 'o', 'l', '.'], false),
        (&['v', 'o', 'l', ' '], false),
        (&['t'], true),
        (&['v'], true),
        (&['#'], false),
    ];
    let len = chars.len();

    for &(prefix, needs_boundary) in prefixes {
        let plen = prefix.len();
        let mut ci = 0usize;
        while ci + plen <= len {
            if chars[ci..ci + plen] != *prefix {
                ci += 1;
                continue;
            }

            // For single-char prefixes (t, v): word boundary check.
            // Standard boundary = previous char is non-alphanumeric.
            // Exception: alphabetic-then-digit "glued" form like "FrontierT01" is
            // also valid, BUT only when the prefix is directly followed by digits
            // (no separator) AND at least 2 digits — limits false positives like
            // "Asterisk" being read as a volume.
            let prev_is_alpha = ci > 0 && chars[ci - 1].is_alphabetic();
            let prev_is_alphanumeric = ci > 0 && chars[ci - 1].is_alphanumeric();

            // Skip "v" inside brackets like [V2] — that's a version, not a volume
            if needs_boundary && ci > 0 && chars[ci - 1] == '[' {
                ci += plen;
                continue;
            }

            // Skip optional spaces, dots, underscores, or '#' after prefix
            let mut i = ci + plen;
            let after_prefix = i;
            while i < len
                && (chars[i] == ' ' || chars[i] == '.' || chars[i] == '_' || chars[i] == '#')
            {
                i += 1;
            }
            let had_separator = i > after_prefix;

            // Read digits
            let digit_start = i;
            while i < len && chars[i].is_ascii_digit() {
                i += 1;
            }
            let digit_count = i - digit_start;

            // Apply boundary rules:
            // - If previous is a digit → never accept (e.g., "MP3" should not match)
            // - If previous is alpha:
            //     - With separator (space/dot/_/#): reject (e.g., "FrontT 1" no, but
            //       "FrontT1" via no-sep+2digit allowed below)
            //     - Without separator AND digits >= 2: accept the glued form
            // - Otherwise (boundary OK): accept
            if needs_boundary {
                let prev_is_digit = ci > 0 && chars[ci - 1].is_ascii_digit();
                if prev_is_digit {
                    ci += plen;
                    continue;
                }
                if prev_is_alpha {
                    if had_separator || digit_count < 2 {
                        ci += plen;
                        continue;
                    }
                } else if prev_is_alphanumeric {
                    // shouldn't happen (alphanumeric without alpha = digit, already handled)
                    ci += plen;
                    continue;
                }
            }

            if i > digit_start {
                let num_str: String = chars[digit_start..i].iter().collect();
                if let Ok(num) = num_str.parse::<i32>() {
                    if !volumes.contains(&num) {
                        volumes.push(num);
                    }
                }
            }

            ci += plen;
        }
    }

    // Pass 3 — bare number patterns (only if passes 1 & 2 found nothing)
    // Handles:
    //   "Les Géants - 07 - Moon.cbz"  →  7
    //   "06. yatho.cbz"               →  6
    if volumes.is_empty() {
        // Pattern A: " - NN - ", " - NN.", or " -NN- " (number between dash separators)
        let dash_num_re = |chars: &[char]| -> Vec<i32> {
            let mut found = Vec::new();
            let mut i = 0;
            while i + 3 < chars.len() {
                // Look for " -" or " - " (dash preceded by space)
                if chars[i] == ' ' && chars[i + 1] == '-' {
                    let mut j = i + 2;
                    // Skip optional space after dash
                    while j < chars.len() && chars[j] == ' ' {
                        j += 1;
                    }
                    let digit_start = j;
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > digit_start {
                        // Ensure followed by "- ", " - ", ".", "@" (Telegram channel tag), or end-ish
                        let valid_end = j >= chars.len()
                            || (j + 2 < chars.len()
                                && chars[j] == ' '
                                && chars[j + 1] == '-'
                                && chars[j + 2] == ' ')
                            || (j + 1 < chars.len() && chars[j] == '-' && chars[j + 1] == ' ')
                            || chars[j] == '.'
                            || chars[j] == '@'
                            || (chars[j] == ' '
                                && (j + 1 >= chars.len() || !chars[j + 1].is_ascii_digit()));
                        if valid_end {
                            let num_str: String = chars[digit_start..j].iter().collect();
                            if let Ok(num) = num_str.parse::<i32>() {
                                if !found.contains(&num) {
                                    found.push(num);
                                }
                            }
                        }
                    }
                }
                i += 1;
            }
            found
        };
        volumes.extend(dash_num_re(&chars));

        // Pattern B: "NN. ", "NN_", or "NN - " at the very start of the string
        if volumes.is_empty() {
            let mut j = 0;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > 0 && j < chars.len() {
                let valid_sep = chars[j] == '.' || chars[j] == ' ' || chars[j] == '_';
                if valid_sep {
                    let num_str: String = chars[..j].iter().collect();
                    if let Ok(num) = num_str.parse::<i32>() {
                        volumes.push(num);
                    }
                }
            }
        }

        // Pattern C: trailing " NN" at the end of the string OR right before a
        // known file extension. Handles:
        //   "Shangri-La Frontier 18.cbz" → 18
        //   "Shangri-La Frontier 18"     → 18 (after file_stem() strips the ext)
        // Requires a preceding space (no glued digits) and a small number
        // (≤999) to limit false positives.
        if volumes.is_empty() {
            const EXTENSIONS: &[&str] = &[".cbz", ".cbr", ".pdf", ".epub", ".zip"];
            let lower_str: String = chars.iter().collect();

            // Find the position where the number must end: either before a
            // known extension, or at the end of the string.
            let end_pos: Option<usize> = EXTENSIONS
                .iter()
                .find_map(|ext| {
                    lower_str.rfind(ext).map(|byte_pos| {
                        // Convert byte position to char position
                        lower_str[..byte_pos].chars().count()
                    })
                })
                .or(Some(chars.len()));

            if let Some(end) = end_pos {
                // Walk back from end to find digits
                let mut start = end;
                while start > 0 && chars[start - 1].is_ascii_digit() {
                    start -= 1;
                }
                // Require digits AND a space (not alphanumeric) before them
                if end > start && start > 0 && chars[start - 1] == ' ' {
                    let num_str: String = chars[start..end].iter().collect();
                    if let Ok(num) = num_str.parse::<i32>() {
                        if num <= 999 {
                            volumes.push(num);
                        }
                    }
                }
            }
        }

        // Pattern D: _NN_ or _NN@ (underscore-delimited volume, Telegram channel filenames)
        // Example: "Black_Clover_29_Une_Nuit_Sans_Matin_...@BD_fr.cbz" → 29
        if volumes.is_empty() {
            let mut i = 0;
            while i < chars.len() {
                if chars[i] == '_' {
                    let digit_start = i + 1;
                    let mut j = digit_start;
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    let digit_count = j - digit_start;
                    if (1..=3).contains(&digit_count)
                        && j < chars.len()
                        && (chars[j] == '_' || chars[j] == '@')
                    {
                        let num_str: String = chars[digit_start..j].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            if num > 0 {
                                volumes.push(num);
                                break;
                            }
                        }
                    }
                }
                i += 1;
            }
        }

        // Pattern E: " NN (" / " NN [" / " NN@" — volume before author info or Telegram channel
        // Example: "Détective Conan 02 (Gosho AOYAMA)@BD_fr.cbz" → 2
        if volumes.is_empty() {
            let mut i = 0;
            while i + 1 < chars.len() {
                if chars[i] == ' ' {
                    let digit_start = i + 1;
                    let mut j = digit_start;
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    let digit_count = j - digit_start;
                    if (1..=3).contains(&digit_count) && j < chars.len() {
                        let after_nn = chars[j];
                        let valid = after_nn == '@'
                            || (after_nn == ' '
                                && j + 1 < chars.len()
                                && (chars[j + 1] == '(' || chars[j + 1] == '['));
                        if valid {
                            let num_str: String = chars[digit_start..j].iter().collect();
                            if let Ok(num) = num_str.parse::<i32>() {
                                if num > 0 && num <= 999 {
                                    volumes.push(num);
                                    break;
                                }
                            }
                        }
                    }
                }
                i += 1;
            }
        }
    }

    ParsedVolumeMarkers { title, volumes }
}

/// Extract all volume numbers from a title string.
pub fn extract_volumes(title: &str) -> Vec<i32> {
    parse_volume_markers(title).volumes
}

/// Read a bare number (no prefix) at `pos`. Returns `(number, position_after_last_digit)`.
pub fn read_bare_number(chars: &[char], pos: usize) -> Option<(i32, usize)> {
    let mut i = pos;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i == pos {
        return None;
    }
    let n: i32 = chars[pos..i].iter().collect::<String>().parse().ok()?;
    Some((n, i))
}

/// Try to read a vol-prefixed number starting at `pos` in the `chars` slice.
/// Returns `(number, position_after_last_digit)` or `None`.
/// Prefixes recognised (longest first to avoid "t" matching "tome"):
/// `tome`, `vol.`, `vol `, `t`, `v`, `#`.
pub fn read_vol_prefix_number(chars: &[char], pos: usize) -> Option<(i32, usize)> {
    if pos >= chars.len() {
        return None;
    }

    // Build a look-ahead string from `pos` (at most 6 chars is enough for the longest prefix "tome ")
    let suffix: String = chars[pos..].iter().collect();

    const PREFIXES: &[(&str, bool)] = &[
        ("volume", false),
        ("tome", false),
        ("vol.", false),
        ("vol ", false),
        ("t", true),
        ("v", true),
        ("#", false),
    ];

    let mut prefix_char_count = 0usize;
    for (p, needs_boundary) in PREFIXES {
        if suffix.starts_with(p) {
            if *needs_boundary && pos > 0 && chars[pos - 1].is_alphanumeric() {
                continue;
            }
            prefix_char_count = p.chars().count();
            break;
        }
    }

    if prefix_char_count == 0 {
        return None;
    }

    let mut i = pos + prefix_char_count;
    while i < chars.len() && (chars[i] == ' ' || chars[i] == '.' || chars[i] == '_') {
        i += 1;
    }

    let digit_start = i;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }

    if i == digit_start {
        return None;
    }

    let n: i32 = chars[digit_start..i]
        .iter()
        .collect::<String>()
        .parse()
        .ok()?;
    Some((n, i))
}

/// Extract the first volume number from a filename (convenience wrapper).
/// Returns `None` if no volume is found.
pub fn extract_volume(filename: &str) -> Option<i32> {
    extract_volumes(filename).into_iter().next()
}

/// Check if a directory name is a oneshot folder at library root level.
fn is_oneshot_folder(name: &str) -> bool {
    let lower = name.to_lowercase();
    // Strip leading underscores/dots (e.g. "_oneshots", "_oneshot")
    let stripped = lower.trim_start_matches(['_', '.']);
    const PATTERNS: &[&str] = &[
        "oneshots",
        "oneshot",
        "one-shots",
        "one-shot",
        "one shots",
        "one shot",
    ];
    PATTERNS.contains(&stripped)
}

/// Check if a directory name is an HS/special subfolder (not a series).
fn is_hs_subfolder(name: &str) -> bool {
    let lower = name.to_lowercase();
    const PATTERNS: &[&str] = &[
        "hors-série",
        "hors-serie",
        "hors série",
        "hors serie",
        "spécial",
        "special",
        "specials",
        "spéciaux",
        "bonus",
        "hs",
        "extras",
        "extra",
        "intégrales",
        "integrales",
        "intégrale",
        "integrale",
        "int",
    ];
    PATTERNS.iter().any(|p| lower == *p)
}

fn extract_series(path: &Path, library_root: &Path) -> Option<String> {
    path.parent().and_then(|parent| {
        let parent_str = parent.to_string_lossy().to_string();
        let root_str = library_root.to_string_lossy().to_string();

        let relative = if let Some(idx) = parent_str.find(&root_str) {
            let after_root = &parent_str[idx + root_str.len()..];
            Path::new(after_root)
        } else if let Ok(relative) = parent.strip_prefix(library_root) {
            relative
        } else {
            tracing::warn!(
                "[PARSER] Cannot determine series: parent '{}' doesn't start with root '{}'",
                parent.display(),
                library_root.display()
            );
            return None;
        };

        let relative_str = relative.to_string_lossy();
        let components: Vec<&str> = relative_str
            .split(['/', '\\'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if components.is_empty() {
            return None;
        }

        // Use the immediate parent directory as the series name.
        // If it matches an HS/special subfolder pattern, go up one level.
        let last = *components.last().unwrap();
        let series_name = if components.len() > 1 && is_hs_subfolder(last) {
            components[components.len() - 2]
        } else {
            last
        };

        if series_name.is_empty() {
            None
        } else {
            Some(series_name.to_string())
        }
    })
}

/// Detect intégrale patterns in a filename.
/// Returns `Some((int_number, cleaned_title))` if an INT pattern is found.
/// Must be called BEFORE extract_hs_info to avoid "INTHS" matching "HS".
pub fn extract_int_info(filename: &str) -> Option<(Option<i32>, String)> {
    let lower = filename.to_lowercase();

    // Patterns to check (longest first, INTHS before INT to avoid partial)
    const PATTERNS: &[&str] = &[
        "intégrale",
        "integrale",
        "intégral",
        "integral",
        "inths",
        "int",
    ];

    for pattern in PATTERNS {
        if let Some(pos) = lower.find(pattern) {
            // Check word boundary before pattern
            if pos > 0 {
                let prev = lower.as_bytes()[pos - 1];
                if prev.is_ascii_alphanumeric() {
                    continue;
                }
            }

            let after_pattern = pos + pattern.len();

            // Check word boundary after pattern (for short patterns like "int")
            if (*pattern == "int" || *pattern == "inths") && after_pattern < lower.len() {
                let next = lower.as_bytes()[after_pattern];
                if next.is_ascii_alphabetic() && next != b'h' {
                    continue; // "inter", "into", etc. — but allow "inths"
                }
                // For "int" specifically, also skip if followed by "h" (will be caught by "inths")
                if *pattern == "int"
                    && after_pattern < lower.len()
                    && lower.as_bytes()[after_pattern] == b'h'
                {
                    continue;
                }
            }

            // Extract optional number after pattern
            let rest = &lower[after_pattern..];
            let mut i = 0;
            let rest_bytes = rest.as_bytes();
            while i < rest_bytes.len() && matches!(rest_bytes[i], b' ' | b'.' | b'-' | b'_') {
                i += 1;
            }
            let digit_start = i;
            while i < rest_bytes.len() && rest_bytes[i].is_ascii_digit() {
                i += 1;
            }
            let int_number = if i > digit_start {
                rest[digit_start..i].parse::<i32>().ok()
            } else {
                None
            };

            // Build cleaned title
            let before = filename[..pos].trim_end_matches([' ', '-', '_', '.']);
            let after_all = &filename[pos + pattern.len()..];
            // Skip number part in original string too
            let cleaned_after = if i > 0 { &after_all[i..] } else { after_all };
            let cleaned_after = cleaned_after.trim_start_matches([' ', '-', '_', '.']);
            let cleaned = if cleaned_after.is_empty() {
                before.to_string()
            } else {
                format!("{} {}", before, cleaned_after).trim().to_string()
            };

            return Some((int_number, cleaned));
        }
    }
    None
}

/// Detect hors-série patterns in a filename.
/// Returns `Some((hs_number, cleaned_title))` if an HS pattern is found.
/// `hs_number` is the optional number after the HS keyword (HS1 → Some(1), HS → None).
pub fn extract_hs_info(filename: &str) -> Option<(Option<i32>, String)> {
    let lower = filename.to_lowercase();

    // Patterns to check (longest first to avoid partial matches)
    const PATTERNS: &[&str] = &[
        "hors-série",
        "hors-serie",
        "hors série",
        "hors serie",
        "spécial",
        "special",
        "bonus",
        "hs",
    ];

    for pattern in PATTERNS {
        if let Some(pos) = lower.find(pattern) {
            // Check word boundary before pattern
            if pos > 0 {
                let prev = lower.as_bytes()[pos - 1];
                if prev.is_ascii_alphanumeric() {
                    continue; // Not at word boundary (e.g., "cahsier" contains "hs")
                }
            }

            let after_pattern = pos + pattern.len();

            // Check word boundary after pattern (for short patterns like "hs")
            if *pattern == "hs" && after_pattern < lower.len() {
                let next = lower.as_bytes()[after_pattern];
                if next.is_ascii_alphabetic() {
                    continue; // Not at word boundary (e.g., "hsk" is not "hs")
                }
            }

            // Extract optional number after pattern
            let rest = &lower[after_pattern..];
            let mut i = 0;
            let rest_bytes = rest.as_bytes();
            // Skip separators (space, dot, dash, underscore)
            while i < rest_bytes.len() && matches!(rest_bytes[i], b' ' | b'.' | b'-' | b'_') {
                i += 1;
            }
            // Read digits
            let digit_start = i;
            while i < rest_bytes.len() && rest_bytes[i].is_ascii_digit() {
                i += 1;
            }
            let hs_number = if i > digit_start {
                rest[digit_start..i].parse::<i32>().ok()
            } else {
                None
            };

            // Build cleaned title (remove the pattern and surrounding separators)
            let before = filename[..pos].trim_end_matches([' ', '-', '.', '_']);
            let after_end = after_pattern + i;
            let after = if after_end < filename.len() {
                filename[after_end..].trim_start_matches([' ', '-', '.', '_'])
            } else {
                ""
            };
            let cleaned = if before.is_empty() {
                after.to_string()
            } else if after.is_empty() {
                before.to_string()
            } else {
                format!("{} - {}", before, after)
            };

            return Some((hs_number, cleaned));
        }
    }

    None
}

/// Fast metadata extraction from filename only — no archive I/O. Always succeeds.
pub fn parse_metadata_fast(
    path: &Path,
    _format: BookFormat,
    library_root: &Path,
) -> ParsedMetadata {
    let filename = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    // Detect oneshot folder: file is exactly one level below library root in a known oneshot dir.
    let in_oneshot_folder = path
        .parent()
        .and_then(|p| p.strip_prefix(library_root).ok())
        .map(|rel| {
            let mut comps = rel.components();
            match (comps.next(), comps.next()) {
                (Some(first), None) => is_oneshot_folder(&first.as_os_str().to_string_lossy()),
                _ => false,
            }
        })
        .unwrap_or(false);

    if in_oneshot_folder {
        return ParsedMetadata {
            title: filename.clone(),
            series: Some(filename),
            volume: None,
            volume_type: VolumeType::Oneshot,
            page_count: None,
        };
    }

    // Check for INT patterns first (before HS, since "INTHS" contains "HS")
    let (volume, volume_type) = if let Some((int_number, _)) = extract_int_info(&filename) {
        (int_number, VolumeType::Integral)
    } else if let Some((hs_number, _)) = extract_hs_info(&filename) {
        (hs_number, VolumeType::Hs)
    } else {
        (extract_volume(&filename), VolumeType::Regular)
    };

    let title = filename;
    let series = extract_series(path, library_root);

    ParsedMetadata {
        title,
        series,
        volume,
        volume_type,
        page_count: None,
    }
}

pub fn parse_metadata(
    path: &Path,
    format: BookFormat,
    library_root: &Path,
) -> Result<ParsedMetadata> {
    let mut meta = parse_metadata_fast(path, format, library_root);

    meta.page_count = match format {
        BookFormat::Cbz => parse_cbz_page_count(path).ok(),
        BookFormat::Cbr => parse_cbr_page_count(path).ok(),
        BookFormat::Pdf => parse_pdf_page_count(path).ok(),
        BookFormat::Epub => parse_epub_page_count(path).ok(),
    };

    Ok(meta)
}

/// Open an archive once and return (page_count, first_page_bytes).
/// `pdf_render_scale`: max dimension used for PDF rasterization; 0 means use default (400).
pub fn analyze_book(
    path: &Path,
    format: BookFormat,
    pdf_render_scale: u32,
) -> Result<(i32, Vec<u8>)> {
    match format {
        BookFormat::Cbz => analyze_cbz(path, true),
        BookFormat::Cbr => analyze_cbr(path, true),
        BookFormat::Pdf => analyze_pdf(path, pdf_render_scale),
        BookFormat::Epub => analyze_epub(path),
    }
}

fn analyze_cbz(path: &Path, allow_fallback: bool) -> Result<(i32, Vec<u8>)> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(zip_err) => {
            if allow_fallback {
                tracing::debug!(target: "extraction", "[EXTRACTION] ZipArchive::new failed for {}: {} — trying fallbacks", path.display(), zip_err);

                // Check magic bytes to avoid expensive RAR probe on ZIP files
                let is_zip_magic = std::fs::File::open(path)
                    .and_then(|mut f| {
                        let mut magic = [0u8; 4];
                        std::io::Read::read_exact(&mut f, &mut magic)?;
                        Ok(magic[0] == b'P' && magic[1] == b'K')
                    })
                    .unwrap_or(false);

                if !is_zip_magic {
                    // Try RAR fallback (file might be a RAR with .cbz extension)
                    if let Ok(result) = analyze_cbr(path, false) {
                        tracing::debug!(target: "extraction", "[EXTRACTION] RAR fallback succeeded for {}", path.display());
                        return Ok(result);
                    }
                }

                // Try streaming fallback: read local file headers without central directory
                // (handles ZIP files with NTFS extra fields that confuse the central dir parser)
                let t0 = std::time::Instant::now();
                if let Ok(result) = analyze_cbz_streaming(path) {
                    tracing::debug!(target: "extraction", "[EXTRACTION] Streaming fallback succeeded for {} — {} pages in {:.0}ms", path.display(), result.0, t0.elapsed().as_secs_f64() * 1000.0);
                    return Ok(result);
                }
            }
            return Err(anyhow::anyhow!(
                "invalid cbz archive for {}: {}",
                path.display(),
                zip_err
            ));
        }
    };

    let mut image_names: Vec<String> = archive
        .file_names()
        .filter(|name| is_image_name(&name.to_ascii_lowercase()))
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    image_names.sort_by(|a, b| natord::compare(a, b));

    if image_names.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in cbz: {}",
            path.display()
        ));
    }

    // Try images in order until one reads successfully (first pages can be corrupted too)
    let count = image_names.len() as i32;
    for first_image in &image_names {
        if let Ok(mut entry) = archive.by_name(first_image) {
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                return Ok((count, buf));
            }
        }
    }

    // zip v8 is stricter about local-header extra fields and CRC validation — fall back to
    // the raw streaming reader which bypasses those checks (method 0/8 only).
    if allow_fallback {
        if let Ok(result) = analyze_cbz_streaming(path) {
            tracing::debug!(target: "extraction", "[EXTRACTION] Streaming fallback succeeded for {} (by_name read failed)", path.display());
            return Ok(result);
        }
    }

    Err(anyhow::anyhow!(
        "all entries unreadable in cbz: {}",
        path.display()
    ))
}

// ---------------------------------------------------------------------------
// Raw ZIP reader — bypasses extra field validation (CRC32 on Unicode path, NTFS, etc.)
// ---------------------------------------------------------------------------

/// Information about a ZIP local file entry (parsed from raw headers).
struct RawZipEntry {
    name: String,
    compression: u16,
    compressed_size: u64,
    uncompressed_size: u64,
    /// File offset of the compressed data (right after name + extra field).
    data_offset: u64,
}

/// Scan local file headers and return metadata for all entries.
/// Does NOT read file data — only collects names and offsets.
fn raw_zip_list_entries(path: &Path) -> Result<Vec<RawZipEntry>> {
    use std::io::{BufReader, Seek, SeekFrom};

    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open zip: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut entries = Vec::new();

    loop {
        let mut sig = [0u8; 4];
        if reader.read_exact(&mut sig).is_err() {
            break;
        }
        if u32::from_le_bytes(sig) != 0x04034b50 {
            break;
        }

        let mut hdr = [0u8; 26];
        reader
            .read_exact(&mut hdr)
            .context("truncated local file header")?;

        let compression = u16::from_le_bytes([hdr[4], hdr[5]]);
        let compressed_size = u32::from_le_bytes([hdr[14], hdr[15], hdr[16], hdr[17]]) as u64;
        let uncompressed_size = u32::from_le_bytes([hdr[18], hdr[19], hdr[20], hdr[21]]) as u64;
        let name_len = u16::from_le_bytes([hdr[22], hdr[23]]) as u64;
        let extra_len = u16::from_le_bytes([hdr[24], hdr[25]]) as u64;

        let mut name_buf = vec![0u8; name_len as usize];
        reader.read_exact(&mut name_buf)?;
        let name = String::from_utf8_lossy(&name_buf).to_string();

        // Skip extra field entirely
        if extra_len > 0 {
            reader.seek(SeekFrom::Current(extra_len as i64))?;
        }

        let data_offset = reader.stream_position()?;

        entries.push(RawZipEntry {
            name,
            compression,
            compressed_size,
            uncompressed_size,
            data_offset,
        });

        // Skip file data
        if compressed_size > 0 {
            reader.seek(SeekFrom::Current(compressed_size as i64))?;
        }
    }

    Ok(entries)
}

/// Read and decompress the data for a single entry.
fn raw_zip_read_entry(path: &Path, entry: &RawZipEntry) -> Result<Vec<u8>> {
    use std::io::{BufReader, Seek, SeekFrom};

    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(entry.data_offset))?;

    let mut compressed = vec![0u8; entry.compressed_size as usize];
    reader.read_exact(&mut compressed)?;

    match entry.compression {
        0 => Ok(compressed),
        8 => {
            let mut decoder = flate2::read::DeflateDecoder::new(&compressed[..]);
            let mut decompressed = Vec::with_capacity(entry.uncompressed_size as usize);
            decoder.read_to_end(&mut decompressed)?;
            Ok(decompressed)
        }
        other => Err(anyhow::anyhow!(
            "unsupported zip compression method: {}",
            other
        )),
    }
}

/// Fallback: list image names + extract all images (for analyze_book which needs first page + count).
fn analyze_cbz_streaming(path: &Path) -> Result<(i32, Vec<u8>)> {
    let entries = raw_zip_list_entries(path)?;
    let mut image_entries: Vec<&RawZipEntry> = entries
        .iter()
        .filter(|e| is_image_name(&e.name.to_ascii_lowercase()))
        .collect();

    if image_entries.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in streaming cbz: {}",
            path.display()
        ));
    }

    image_entries.sort_by(|a, b| natord::compare(&a.name, &b.name));
    let count = image_entries.len() as i32;
    let first_bytes = raw_zip_read_entry(path, image_entries[0])?;
    Ok((count, first_bytes))
}

/// Returns true if the error indicates the file is not a RAR archive (likely a ZIP with .cbr extension).
fn is_not_rar_error(err_str: &str) -> bool {
    err_str.contains("Not a RAR archive") || err_str.contains("bad archive")
}

/// Try to open a CBR file for listing. Returns the archive or an error string.
/// If the error indicates the file is not actually a RAR archive, the caller can fall back to CBZ.
fn open_cbr_listing(
    path: &Path,
) -> std::result::Result<unrar::OpenArchive<unrar::List, unrar::CursorBeforeHeader>, String> {
    unrar::Archive::new(path)
        .open_for_listing()
        .map_err(|e| format!("unrar listing failed for {}: {}", path.display(), e))
}

fn analyze_cbr(path: &Path, allow_fallback: bool) -> Result<(i32, Vec<u8>)> {
    // Pass 1: list all image names via unrar (in-process, no subprocess)
    let mut image_names: Vec<String> = {
        // Some .cbr files are actually ZIP archives with wrong extension — fallback to CBZ parser
        let archive = match open_cbr_listing(path) {
            Ok(a) => a,
            Err(e_str) => {
                if allow_fallback && is_not_rar_error(&e_str) {
                    return analyze_cbz(path, false).map_err(|zip_err| {
                        anyhow::anyhow!(
                            "not a RAR archive and ZIP fallback also failed for {}: RAR={}, ZIP={}",
                            path.display(),
                            e_str,
                            zip_err
                        )
                    });
                }
                return Err(anyhow::anyhow!("{}", e_str));
            }
        };
        let mut names = Vec::new();
        for entry in archive {
            let entry = entry.map_err(|e| anyhow::anyhow!("unrar entry error: {}", e))?;
            let name = entry.filename.to_string_lossy().to_string();
            if is_image_name(&name.to_ascii_lowercase()) {
                names.push(name);
            }
        }
        names
    };

    if image_names.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in cbr: {}",
            path.display()
        ));
    }

    image_names.sort_by(|a, b| natord::compare(a, b));
    let count = image_names.len() as i32;
    let first_name = image_names[0].clone();

    // Pass 2: extract first image to memory
    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| {
            anyhow::anyhow!(
                "unrar open for processing failed for {}: {}",
                path.display(),
                e
            )
        })?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        if entry_name == first_name {
            let (data, _) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read data: {}", e))?;
            return Ok((count, data));
        }
        archive = header
            .skip()
            .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
    }

    Err(anyhow::anyhow!(
        "could not find '{}' in {}",
        first_name,
        path.display()
    ))
}

fn analyze_pdf(path: &Path, pdf_render_scale: u32) -> Result<(i32, Vec<u8>)> {
    use pdfium_render::prelude::*;

    // Open PDF once — get page count and render first page in a single pass
    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library()
            .map_err(|e| anyhow::anyhow!("pdfium library not available: {:?}", e))?,
    );

    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| anyhow::anyhow!("pdfium load failed for {}: {:?}", path.display(), e))?;

    let count = document.pages().len();
    if count == 0 {
        return Err(anyhow::anyhow!("PDF has no pages: {}", path.display()));
    }

    let scale = if pdf_render_scale == 0 {
        400
    } else {
        pdf_render_scale
    } as i32;
    let config = PdfRenderConfig::new()
        .set_target_width(scale)
        .set_maximum_height(scale);

    let page = document
        .pages()
        .get(0)
        .map_err(|e| anyhow::anyhow!("cannot get first page of {}: {:?}", path.display(), e))?;

    let bitmap = page
        .render_with_config(&config)
        .map_err(|e| anyhow::anyhow!("pdfium render failed for {}: {:?}", path.display(), e))?;

    let image = bitmap
        .as_image()
        .map_err(|e| anyhow::anyhow!("pdfium image conversion failed: {:?}", e))?;
    let mut buf = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut buf, image::ImageFormat::Png)
        .context("failed to encode rendered PDF page as PNG")?;

    Ok((count, buf.into_inner()))
}

fn parse_cbz_page_count(path: &Path) -> Result<i32> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    match zip::ZipArchive::new(file) {
        Ok(mut archive) => {
            let mut count: i32 = 0;
            for i in 0..archive.len() {
                let entry = archive.by_index(i).context("cannot read cbz entry")?;
                let name = entry.name().to_ascii_lowercase();
                if is_image_name(&name) {
                    count += 1;
                }
            }
            Ok(count)
        }
        Err(_) => {
            // Fallback: streaming count (bypasses extra field validation)
            parse_cbz_page_count_streaming(path)
        }
    }
}

fn parse_cbz_page_count_streaming(path: &Path) -> Result<i32> {
    let entries = raw_zip_list_entries(path)?;
    let count = entries
        .iter()
        .filter(|e| is_image_name(&e.name.to_ascii_lowercase()))
        .count() as i32;
    Ok(count)
}

fn parse_cbr_page_count(path: &Path) -> Result<i32> {
    let archive = match open_cbr_listing(path) {
        Ok(a) => a,
        Err(e_str) => {
            if is_not_rar_error(&e_str) {
                return parse_cbz_page_count(path);
            }
            return Err(anyhow::anyhow!("{}", e_str));
        }
    };
    let count = archive
        .filter(|r| {
            r.as_ref()
                .map(|e| is_image_name(&e.filename.to_string_lossy().to_ascii_lowercase()))
                .unwrap_or(false)
        })
        .count() as i32;
    Ok(count)
}

fn parse_pdf_page_count(path: &Path) -> Result<i32> {
    let doc = lopdf::Document::load(path)
        .with_context(|| format!("cannot open pdf: {}", path.display()))?;
    Ok(doc.get_pages().len() as i32)
}

pub fn is_image_name(name: &str) -> bool {
    // Skip macOS metadata entries (__MACOSX/ prefix or AppleDouble ._* files)
    if name.starts_with("__macosx/") || name.contains("/._") || name.starts_with("._") {
        return false;
    }
    name.ends_with(".jpg")
        || name.ends_with(".jpeg")
        || name.ends_with(".png")
        || name.ends_with(".webp")
        || name.ends_with(".avif")
        || name.ends_with(".gif")
        || name.ends_with(".bmp")
        || name.ends_with(".tif")
        || name.ends_with(".tiff")
}

/// Returns the sorted list of image entry names in a CBZ or CBR archive.
/// Intended to be cached by the caller; pass the result to `extract_image_by_name`.
pub fn list_archive_images(path: &Path, format: BookFormat) -> Result<Vec<String>> {
    match format {
        BookFormat::Cbz => list_cbz_images(path),
        BookFormat::Cbr => list_cbr_images(path),
        BookFormat::Pdf => Err(anyhow::anyhow!(
            "list_archive_images not applicable for PDF"
        )),
        BookFormat::Epub => get_epub_image_index(path),
    }
}

fn list_cbz_images(path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(zip_err) => {
            // Try RAR fallback
            if let Ok(names) = list_cbr_images(path) {
                return Ok(names);
            }
            // Try streaming fallback
            return list_cbz_images_streaming(path)
                .map_err(|_| anyhow::anyhow!("invalid cbz for {}: {}", path.display(), zip_err));
        }
    };

    let mut names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let lower = entry.name().to_ascii_lowercase();
        if is_image_name(&lower) {
            names.push(entry.name().to_string());
        }
    }
    names.sort_by(|a, b| natord::compare(a, b));
    Ok(names)
}

fn list_cbz_images_streaming(path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz for streaming: {}", path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    let mut names: Vec<String> = Vec::new();

    loop {
        match zip::read::read_zipfile_from_stream(&mut reader) {
            Ok(Some(mut entry)) => {
                let name = entry.name().to_string();
                if is_image_name(&name.to_ascii_lowercase()) {
                    names.push(name);
                }
                std::io::copy(&mut entry, &mut std::io::sink())?;
            }
            Ok(None) => break,
            Err(_) => {
                if !names.is_empty() {
                    break;
                }
                return Err(anyhow::anyhow!(
                    "streaming ZIP listing failed for {}",
                    path.display()
                ));
            }
        }
    }
    names.sort_by(|a, b| natord::compare(a, b));
    Ok(names)
}

fn list_cbr_images(path: &Path) -> Result<Vec<String>> {
    let archive = match open_cbr_listing(path) {
        Ok(a) => a,
        Err(e_str) => {
            if is_not_rar_error(&e_str) {
                return list_cbz_images(path);
            }
            return Err(anyhow::anyhow!("{}", e_str));
        }
    };
    let mut names: Vec<String> = Vec::new();
    for entry in archive {
        let entry = entry.map_err(|e| anyhow::anyhow!("unrar entry error: {}", e))?;
        let name = entry.filename.to_string_lossy().to_string();
        if is_image_name(&name.to_ascii_lowercase()) {
            names.push(name);
        }
    }
    names.sort_by(|a, b| natord::compare(a, b));
    Ok(names)
}

/// Extract a specific image entry by name from a CBZ or CBR archive.
/// Use in combination with `list_archive_images` to avoid re-enumerating entries.
pub fn extract_image_by_name(path: &Path, format: BookFormat, image_name: &str) -> Result<Vec<u8>> {
    match format {
        BookFormat::Cbz => extract_cbz_by_name(path, image_name),
        BookFormat::Cbr => extract_cbr_by_name(path, image_name),
        BookFormat::Pdf => Err(anyhow::anyhow!("use extract_page for PDF")),
        BookFormat::Epub => extract_cbz_by_name(path, image_name),
    }
}

fn extract_cbz_by_name(path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return extract_cbz_by_name_streaming(path, image_name),
    };
    let mut entry = archive
        .by_name(image_name)
        .with_context(|| format!("entry '{}' not found in {}", image_name, path.display()))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

fn extract_cbz_by_name_streaming(path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz for streaming: {}", path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    loop {
        match zip::read::read_zipfile_from_stream(&mut reader) {
            Ok(Some(mut entry)) => {
                if entry.name() == image_name {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf)?;
                    return Ok(buf);
                }
                std::io::copy(&mut entry, &mut std::io::sink())?;
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    Err(anyhow::anyhow!(
        "entry '{}' not found in streaming cbz: {}",
        image_name,
        path.display()
    ))
}

fn extract_cbr_by_name(path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| {
            anyhow::anyhow!(
                "unrar open for processing failed for {}: {}",
                path.display(),
                e
            )
        })?;
    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        if entry_name == image_name {
            let (data, _) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read data: {}", e))?;
            return Ok(data);
        }
        archive = header
            .skip()
            .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
    }
    Err(anyhow::anyhow!(
        "entry '{}' not found in cbr: {}",
        image_name,
        path.display()
    ))
}

pub fn extract_first_page(path: &Path, format: BookFormat) -> Result<Vec<u8>> {
    extract_page(path, format, 1, 0)
}

/// Extract a specific page (1-based index) from a book archive.
/// `pdf_render_width`: max width for PDF rasterization; 0 means use default (1200).
pub fn extract_page(
    path: &Path,
    format: BookFormat,
    page_number: u32,
    pdf_render_width: u32,
) -> Result<Vec<u8>> {
    if page_number == 0 {
        return Err(anyhow::anyhow!("page index starts at 1"));
    }
    match format {
        BookFormat::Cbz => extract_cbz_page(path, page_number, true),
        BookFormat::Cbr => extract_cbr_page(path, page_number, true),
        BookFormat::Pdf => {
            let width = if pdf_render_width == 0 {
                1200
            } else {
                pdf_render_width
            };
            render_pdf_page_n(path, page_number, width)
        }
        BookFormat::Epub => extract_epub_page(path, page_number),
    }
}

/// Cache of sorted image names per archive path. Avoids re-listing and sorting on every page request.
/// Keyed by (path, mtime) so the cache invalidates automatically when the file is replaced.
type ArchiveIndexCache = OnceLock<Mutex<HashMap<PathBuf, (SystemTime, Vec<String>)>>>;

static CBZ_INDEX_CACHE: ArchiveIndexCache = OnceLock::new();

fn cbz_index_cache() -> &'static Mutex<HashMap<PathBuf, (SystemTime, Vec<String>)>> {
    CBZ_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Get sorted image names from cache, or list + sort + cache them.
fn get_cbz_image_index(path: &Path, archive: &mut zip::ZipArchive<std::fs::File>) -> Vec<String> {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    {
        let cache = cbz_index_cache().lock().unwrap();
        if let Some((cached_mtime, names)) = cache.get(path) {
            if mtime == Some(*cached_mtime) {
                return names.clone();
            }
        }
    }
    let mut image_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.name().to_ascii_lowercase();
        if is_image_name(&name) {
            image_names.push(entry.name().to_string());
        }
    }
    image_names.sort_by(|a, b| natord::compare(a, b));
    if let Some(mtime) = mtime {
        let mut cache = cbz_index_cache().lock().unwrap();
        cache.insert(path.to_path_buf(), (mtime, image_names.clone()));
    }
    image_names
}

fn extract_cbz_page(path: &Path, page_number: u32, allow_fallback: bool) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let index = page_number as usize - 1;

    match zip::ZipArchive::new(file) {
        Ok(mut archive) => {
            let image_names = get_cbz_image_index(path, &mut archive);

            let selected = image_names.get(index).with_context(|| {
                format!(
                    "page {} out of range (total: {})",
                    page_number,
                    image_names.len()
                )
            })?;

            let mut entry = archive
                .by_name(selected)
                .with_context(|| format!("cannot read page {}", selected))?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            Ok(buf)
        }
        Err(zip_err) => {
            if allow_fallback {
                // Try RAR fallback (file might be a RAR with .cbz extension)
                if let Ok(data) = extract_cbr_page(path, page_number, false) {
                    return Ok(data);
                }
                // Raw ZIP fallback (bypasses extra field validation)
                return extract_cbz_page_raw(path, page_number);
            }
            Err(anyhow::anyhow!(
                "invalid cbz archive for {}: {}",
                path.display(),
                zip_err
            ))
        }
    }
}

fn extract_cbz_page_raw(path: &Path, page_number: u32) -> Result<Vec<u8>> {
    let entries = raw_zip_list_entries(path)?;
    let mut image_entries: Vec<&RawZipEntry> = entries
        .iter()
        .filter(|e| is_image_name(&e.name.to_ascii_lowercase()))
        .collect();
    image_entries.sort_by(|a, b| natord::compare(&a.name, &b.name));

    let index = page_number as usize - 1;
    let entry = image_entries.get(index).with_context(|| {
        format!(
            "page {} out of range (total: {})",
            page_number,
            image_entries.len()
        )
    })?;

    raw_zip_read_entry(path, entry)
}

fn extract_cbr_page(path: &Path, page_number: u32, allow_fallback: bool) -> Result<Vec<u8>> {
    let index = page_number as usize - 1;

    let mut image_names: Vec<String> = {
        let archive = match open_cbr_listing(path) {
            Ok(a) => a,
            Err(e_str) => {
                if allow_fallback && is_not_rar_error(&e_str) {
                    return extract_cbz_page(path, page_number, false);
                }
                return Err(anyhow::anyhow!("{}", e_str));
            }
        };
        let mut names = Vec::new();
        for entry in archive {
            let entry = entry.map_err(|e| anyhow::anyhow!("unrar entry error: {}", e))?;
            let name = entry.filename.to_string_lossy().to_string();
            if is_image_name(&name.to_ascii_lowercase()) {
                names.push(name);
            }
        }
        names
    };

    image_names.sort_by(|a, b| natord::compare(a, b));
    let target = image_names
        .get(index)
        .with_context(|| {
            format!(
                "page {} out of range (total: {})",
                page_number,
                image_names.len()
            )
        })?
        .clone();

    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| anyhow::anyhow!("unrar open for processing failed: {}", e))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        if entry_name == target {
            let (data, _) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read data: {}", e))?;
            return Ok(data);
        }
        archive = header
            .skip()
            .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
    }

    Err(anyhow::anyhow!(
        "page '{}' not found in {}",
        target,
        path.display()
    ))
}

fn render_pdf_page_n(path: &Path, page_number: u32, width: u32) -> Result<Vec<u8>> {
    use pdfium_render::prelude::*;

    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library()
            .map_err(|e| anyhow::anyhow!("pdfium library not available: {:?}", e))?,
    );

    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| anyhow::anyhow!("pdfium load failed for {}: {:?}", path.display(), e))?;

    let page_index = (page_number - 1) as i32;
    let page = document
        .pages()
        .get(page_index)
        .map_err(|_| anyhow::anyhow!("page {} out of range in {}", page_number, path.display()))?;

    let config = PdfRenderConfig::new().set_target_width(width as i32);

    let bitmap = page
        .render_with_config(&config)
        .map_err(|e| anyhow::anyhow!("pdfium render failed for {}: {:?}", path.display(), e))?;

    let image = bitmap
        .as_image()
        .map_err(|e| anyhow::anyhow!("pdfium image conversion failed: {:?}", e))?;
    let mut buf = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut buf, image::ImageFormat::Png)
        .context("failed to encode rendered PDF page as PNG")?;

    Ok(buf.into_inner())
}

// ============================================================
// EPUB support — spine-aware image index with cache
// ============================================================

/// Cache of ordered image paths per EPUB file. Avoids re-parsing OPF/XHTML on every page request.
/// Keyed by (path, mtime) so the cache invalidates automatically when the file is replaced.
static EPUB_INDEX_CACHE: ArchiveIndexCache = OnceLock::new();

fn epub_index_cache() -> &'static Mutex<HashMap<PathBuf, (SystemTime, Vec<String>)>> {
    EPUB_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

// Pre-compiled regex patterns for EPUB XML parsing (compiled once on first use)
static RE_EPUB_ROOTFILE: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ITEM: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ITEMREF: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_IMG_SRC: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_SVG_HREF: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ATTR_ID: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ATTR_HREF: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ATTR_MEDIA: OnceLock<regex::Regex> = OnceLock::new();

struct EpubManifestItem {
    href: String,
    media_type: String,
}

/// Build the ordered list of image paths for an EPUB file.
/// Walks the OPF spine to determine reading order, parses XHTML/SVG pages
/// for image references, and falls back to CBZ-style listing if no
/// images are found through the spine.
fn build_epub_image_index(path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open epub: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("invalid epub zip: {}", path.display()))?;

    // 1. Find OPF path from META-INF/container.xml
    let opf_path = {
        let mut entry = archive
            .by_name("META-INF/container.xml")
            .context("missing META-INF/container.xml — not a valid EPUB")?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let xml = String::from_utf8_lossy(&buf);
        let re = RE_EPUB_ROOTFILE.get_or_init(|| {
            regex::Regex::new(r#"<(?:\w+:)?rootfile[^>]+full-path="([^"]+)""#).unwrap()
        });
        re.captures(&xml)
            .and_then(|c| c.get(1))
            .map(|m| decode_xml_entities(m.as_str()))
            .context("no rootfile found in container.xml")?
    };

    let opf_dir = std::path::Path::new(&opf_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    // 2. Parse OPF manifest + spine
    let (manifest, spine_idrefs) = {
        let mut entry = archive
            .by_name(&opf_path)
            .with_context(|| format!("missing OPF file: {}", opf_path))?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let xml = String::from_utf8_lossy(&buf);
        parse_epub_opf(&xml, &opf_dir)?
    };

    // 3. Walk spine entries to build ordered image list
    let re_img = RE_EPUB_IMG_SRC
        .get_or_init(|| regex::Regex::new(r#"(?i)<img\s[^>]*src=["']([^"']+)["']"#).unwrap());
    let re_svg = RE_EPUB_SVG_HREF.get_or_init(|| {
        regex::Regex::new(r#"(?i)<image\s[^>]*(?:xlink:)?href=["']([^"']+)["']"#).unwrap()
    });

    let mut images: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for idref in &spine_idrefs {
        let item = match manifest.get(idref.as_str()) {
            Some(item) => item,
            None => continue,
        };

        // Direct raster image in spine (rare but possible)
        if item.media_type.starts_with("image/") && !item.media_type.contains("svg") {
            if seen.insert(item.href.clone()) {
                images.push(item.href.clone());
            }
            continue;
        }

        // Read XHTML/SVG content — entry is dropped at end of match arm, releasing archive borrow
        let content = match archive.by_name(&item.href) {
            Ok(mut entry) => {
                let mut buf = Vec::new();
                match entry.read_to_end(&mut buf) {
                    Ok(_) => String::from_utf8_lossy(&buf).to_string(),
                    Err(_) => continue,
                }
            }
            Err(_) => continue,
        };

        let content_dir = std::path::Path::new(&item.href)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // Extract <img src="..."> and <image [xlink:]href="...">
        for re in [re_img, re_svg] {
            for cap in re.captures_iter(&content) {
                if let Some(src) = cap.get(1) {
                    let src_str = src.as_str();
                    if src_str.starts_with("data:") {
                        continue;
                    }
                    let decoded = decode_xml_entities(&percent_decode_epub(src_str));
                    let resolved = resolve_epub_path(&content_dir, &decoded);
                    if seen.insert(resolved.clone()) {
                        images.push(resolved);
                    }
                }
            }
        }
    }

    // 4. Fallback: no images from spine → list all images in ZIP (CBZ-style)
    if images.is_empty() {
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                let name = entry.name().to_string();
                if is_image_name(&name.to_ascii_lowercase()) && seen.insert(name.clone()) {
                    images.push(name);
                }
            }
        }
        images.sort_by(|a, b| natord::compare(a, b));
    }

    if images.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in epub: {}",
            path.display()
        ));
    }

    Ok(images)
}

fn parse_epub_opf(
    xml: &str,
    opf_dir: &str,
) -> Result<(HashMap<String, EpubManifestItem>, Vec<String>)> {
    let re_item = RE_EPUB_ITEM
        .get_or_init(|| regex::Regex::new(r#"(?s)<(?:\w+:)?item\s([^>]+?)/?>"#).unwrap());
    let re_itemref = RE_EPUB_ITEMREF
        .get_or_init(|| regex::Regex::new(r#"<(?:\w+:)?itemref\s[^>]*idref="([^"]+)""#).unwrap());
    let re_id =
        RE_EPUB_ATTR_ID.get_or_init(|| regex::Regex::new(r#"(?:^|\s)id="([^"]+)""#).unwrap());
    let re_href =
        RE_EPUB_ATTR_HREF.get_or_init(|| regex::Regex::new(r#"(?:^|\s)href="([^"]+)""#).unwrap());
    let re_media =
        RE_EPUB_ATTR_MEDIA.get_or_init(|| regex::Regex::new(r#"media-type="([^"]+)""#).unwrap());

    let mut manifest: HashMap<String, EpubManifestItem> = HashMap::new();
    for cap in re_item.captures_iter(xml) {
        if let Some(attrs) = cap.get(1) {
            let a = attrs.as_str();
            let id = re_id.captures(a).and_then(|c| c.get(1));
            let href = re_href.captures(a).and_then(|c| c.get(1));
            let media = re_media.captures(a).and_then(|c| c.get(1));

            if let (Some(id), Some(href), Some(media)) = (id, href, media) {
                let decoded_href = decode_xml_entities(&percent_decode_epub(href.as_str()));
                let resolved = resolve_epub_path(opf_dir, &decoded_href);
                manifest.insert(
                    id.as_str().to_string(),
                    EpubManifestItem {
                        href: resolved,
                        media_type: media.as_str().to_string(),
                    },
                );
            }
        }
    }

    let spine_idrefs: Vec<String> = re_itemref
        .captures_iter(xml)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();

    Ok((manifest, spine_idrefs))
}

/// Get the cached image index for an EPUB, building it on first access.
fn get_epub_image_index(path: &Path) -> Result<Vec<String>> {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    {
        let cache = epub_index_cache().lock().unwrap();
        if let Some((cached_mtime, names)) = cache.get(path) {
            if mtime == Some(*cached_mtime) {
                return Ok(names.clone());
            }
        }
    }
    let images = build_epub_image_index(path)?;
    if let Some(mtime) = mtime {
        let mut cache = epub_index_cache().lock().unwrap();
        cache.insert(path.to_path_buf(), (mtime, images.clone()));
    }
    Ok(images)
}

fn parse_epub_page_count(path: &Path) -> Result<i32> {
    let images = build_epub_image_index(path)?;
    Ok(images.len() as i32)
}

fn analyze_epub(path: &Path) -> Result<(i32, Vec<u8>)> {
    let images = get_epub_image_index(path)?;
    let count = images.len() as i32;

    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open epub: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;

    for img_path in &images {
        if let Ok(mut entry) = archive.by_name(img_path) {
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                return Ok((count, buf));
            }
        }
    }

    Err(anyhow::anyhow!(
        "no readable images in epub: {}",
        path.display()
    ))
}

fn extract_epub_page(path: &Path, page_number: u32) -> Result<Vec<u8>> {
    let images = get_epub_image_index(path)?;
    let index = page_number as usize - 1;
    let img_path = images.get(index).with_context(|| {
        format!(
            "page {} out of range (total: {})",
            page_number,
            images.len()
        )
    })?;

    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open epub: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive
        .by_name(img_path)
        .with_context(|| format!("image '{}' not found in epub", img_path))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

// --- EPUB path/encoding helpers ---

fn resolve_epub_path(base_dir: &str, href: &str) -> String {
    if let Some(stripped) = href.strip_prefix('/') {
        return normalize_epub_path(stripped);
    }
    if base_dir.is_empty() {
        return normalize_epub_path(href);
    }
    normalize_epub_path(&format!("{}/{}", base_dir, href))
}

fn normalize_epub_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

fn percent_decode_epub(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (epub_hex_val(bytes[i + 1]), epub_hex_val(bytes[i + 2])) {
                result.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

fn epub_hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn decode_xml_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Convert a CBR file to CBZ in-place (same directory, same stem).
///
/// The conversion is safe: a `.cbz.tmp` file is written first, verified, then
/// atomically renamed to `.cbz`. The original CBR is **not** deleted by this
/// function — the caller is responsible for removing it after a successful DB update.
///
/// Returns the path of the newly created `.cbz` file.
pub fn convert_cbr_to_cbz(cbr_path: &Path) -> Result<PathBuf> {
    let parent = cbr_path
        .parent()
        .with_context(|| format!("no parent directory for {}", cbr_path.display()))?;
    let stem = cbr_path
        .file_stem()
        .with_context(|| format!("no file stem for {}", cbr_path.display()))?;

    let cbz_path = parent.join(format!("{}.cbz", stem.to_string_lossy()));
    let tmp_path = parent.join(format!("{}.cbz.tmp", stem.to_string_lossy()));

    if cbz_path.exists() {
        return Err(anyhow::anyhow!(
            "CBZ file already exists: {}",
            cbz_path.display()
        ));
    }

    // Extract all images from CBR into memory using unrar crate (no subprocess)
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();
    let mut archive = unrar::Archive::new(cbr_path)
        .open_for_processing()
        .map_err(|e| anyhow::anyhow!("unrar open failed for {}: {}", cbr_path.display(), e))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        let file_name = Path::new(&entry_name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| entry_name.clone());

        if is_image_name(&entry_name.to_ascii_lowercase()) {
            let (data, next) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read: {}", e))?;
            images.push((file_name, data));
            archive = next;
        } else {
            archive = header
                .skip()
                .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
        }
    }

    if images.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in CBR: {}",
            cbr_path.display()
        ));
    }

    images.sort_by(|(a, _), (b, _)| natord::compare(a, b));
    let image_count = images.len();

    // Pack images into the .cbz.tmp file
    let pack_result = (|| -> Result<()> {
        let cbz_file = std::fs::File::create(&tmp_path)
            .with_context(|| format!("cannot create {}", tmp_path.display()))?;
        let mut zip = zip::ZipWriter::new(cbz_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for (file_name, data) in &images {
            zip.start_file(file_name, options)
                .with_context(|| format!("cannot add file {} to zip", file_name))?;
            zip.write_all(data)
                .with_context(|| format!("cannot write {} to zip", file_name))?;
        }
        zip.finish().context("cannot finalize zip")?;
        Ok(())
    })();

    if let Err(err) = pack_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    // Verify the CBZ contains the expected number of images
    let verify_result = (|| -> Result<()> {
        let file = std::fs::File::open(&tmp_path)
            .with_context(|| format!("cannot open {}", tmp_path.display()))?;
        let archive = zip::ZipArchive::new(file).context("invalid zip archive")?;
        let packed_count = (0..archive.len())
            .filter(|&i| {
                archive
                    .name_for_index(i)
                    .map(|n| is_image_name(&n.to_ascii_lowercase()))
                    .unwrap_or(false)
            })
            .count();
        if packed_count != image_count {
            return Err(anyhow::anyhow!(
                "CBZ verification failed: expected {} images, found {}",
                image_count,
                packed_count
            ));
        }
        Ok(())
    })();

    if let Err(err) = verify_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    std::fs::rename(&tmp_path, &cbz_path).with_context(|| {
        format!(
            "cannot rename {} to {}",
            tmp_path.display(),
            cbz_path.display()
        )
    })?;

    Ok(cbz_path)
}

#[allow(dead_code)]
fn clean_title(filename: &str) -> String {
    parse_volume_markers(filename).cleaned_title()
}

impl ParsedVolumeMarkers<'_> {
    /// Remove the marker forms historically stripped from display titles.
    ///
    /// This intentionally keeps its conservative legacy cleanup policy: not
    /// every number recognized for file matching is suitable for removal from
    /// a display title.
    fn cleaned_title(&self) -> String {
        let cleaned = regex::Regex::new(r"(?i)\s*T\d+\s*")
            .ok()
            .map(|re| re.replace_all(self.title, " ").to_string())
            .unwrap_or_else(|| self.title.to_string());

        let cleaned = regex::Regex::new(r"(?i)\s*Vol\.?\s*\d+\s*")
            .ok()
            .map(|re| re.replace_all(&cleaned, " ").to_string())
            .unwrap_or(cleaned);

        let cleaned = regex::Regex::new(r"(?i)\s*Volume\s*\d+\s*")
            .ok()
            .map(|re| re.replace_all(&cleaned, " ").to_string())
            .unwrap_or(cleaned);

        let cleaned = regex::Regex::new(r"#\d+")
            .ok()
            .map(|re| re.replace_all(&cleaned, " ").to_string())
            .unwrap_or(cleaned);

        let cleaned = regex::Regex::new(r"-\s*\d+\s*$")
            .ok()
            .map(|re| re.replace_all(&cleaned, " ").to_string())
            .unwrap_or(cleaned);

        cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_metadata_volume_requires_an_explicit_marker() {
        assert_eq!(extract_metadata_volume("One Piece - Tome 12"), Some(12));
        assert_eq!(extract_metadata_volume("Naruto Tome 12"), Some(12));
        assert_eq!(extract_metadata_volume("Astro Boy T.03"), Some(3));
        assert_eq!(extract_metadata_volume("T.007"), Some(7));
        assert_eq!(extract_metadata_volume("Saga Vol. 004"), Some(4));
        assert_eq!(extract_metadata_volume("Vol 5"), Some(5));
        assert_eq!(extract_metadata_volume("Series Volume 5"), Some(5));
        assert_eq!(extract_metadata_volume("Issue #42"), Some(42));
        assert_eq!(extract_metadata_volume("#5 something"), Some(5));

        assert_eq!(extract_metadata_volume("One Piece (12)"), None);
        assert_eq!(extract_metadata_volume("One Piece 12"), None);
        assert_eq!(extract_metadata_volume("Edition 2024"), None);
        assert_eq!(extract_metadata_volume("One Piece - Intégrale"), None);
    }

    #[test]
    fn volume_projections_share_a_deduplicated_parse() {
        let parsed = parse_volume_markers("Series T01-T03 T02");

        assert_eq!(parsed.volumes, vec![1, 2, 3]);
        assert_eq!(extract_volume(parsed.title), Some(1));
        assert_eq!(extract_volumes(parsed.title), parsed.volumes);
    }

    #[test]
    fn cleaning_a_title_does_not_create_a_volume_marker() {
        let title = "Series Vol. 02";
        let parsed = parse_volume_markers(title);
        let cleaned = parsed.cleaned_title();

        assert!(extract_volumes(&cleaned)
            .into_iter()
            .all(|volume| parsed.volumes.contains(&volume)));
    }

    #[test]
    fn is_not_rar_detects_not_rar_archive() {
        assert!(is_not_rar_error(
            "unrar listing failed for /path/file.cbr: Not a RAR archive"
        ));
    }

    #[test]
    fn is_not_rar_detects_bad_archive() {
        assert!(is_not_rar_error(
            "unrar listing failed for /path/file.cbr: bad archive"
        ));
    }

    #[test]
    fn is_not_rar_ignores_other_errors() {
        assert!(!is_not_rar_error(
            "unrar listing failed for /path/file.cbr: file not found"
        ));
        assert!(!is_not_rar_error("some other error"));
        assert!(!is_not_rar_error(""));
    }

    #[test]
    fn open_cbr_listing_nonexistent_file() {
        let result = open_cbr_listing(Path::new("/nonexistent/file.cbr"));
        assert!(result.is_err());
    }

    #[test]
    fn detect_format_cbz() {
        assert_eq!(detect_format(Path::new("test.cbz")), Some(BookFormat::Cbz));
        assert_eq!(detect_format(Path::new("test.CBZ")), Some(BookFormat::Cbz));
    }

    #[test]
    fn detect_format_cbr() {
        assert_eq!(detect_format(Path::new("test.cbr")), Some(BookFormat::Cbr));
    }

    #[test]
    fn detect_format_pdf() {
        assert_eq!(detect_format(Path::new("test.pdf")), Some(BookFormat::Pdf));
    }

    #[test]
    fn detect_format_epub() {
        assert_eq!(
            detect_format(Path::new("test.epub")),
            Some(BookFormat::Epub)
        );
    }

    #[test]
    fn detect_format_unknown() {
        assert_eq!(detect_format(Path::new("test.txt")), None);
        assert_eq!(detect_format(Path::new("no_extension")), None);
    }

    #[test]
    fn is_image_name_accepts_valid() {
        assert!(is_image_name("page01.jpg"));
        assert!(is_image_name("cover.png"));
        assert!(is_image_name("image.webp"));
    }

    #[test]
    fn is_image_name_rejects_non_images() {
        assert!(!is_image_name("readme.txt"));
        assert!(!is_image_name("metadata.xml"));
        assert!(!is_image_name(""));
    }

    #[test]
    fn is_image_name_rejects_macos_metadata() {
        assert!(!is_image_name("__macosx/page01.jpg"));
        assert!(!is_image_name("folder/._cover.png"));
        assert!(!is_image_name("._hidden.jpg"));
    }

    #[test]
    fn is_image_name_accepts_all_formats() {
        assert!(is_image_name("page.jpeg"));
        assert!(is_image_name("page.avif"));
        assert!(is_image_name("page.gif"));
        assert!(is_image_name("page.bmp"));
        assert!(is_image_name("page.tif"));
        assert!(is_image_name("page.tiff"));
    }

    // ─── extract_volume ──────────────────────────────────────────────────

    #[test]
    fn extract_volume_t_prefix() {
        assert_eq!(extract_volume("One Piece T01"), Some(1));
        assert_eq!(extract_volume("Naruto T12"), Some(12));
        assert_eq!(extract_volume("Series T100"), Some(100));
    }

    #[test]
    fn extract_volume_tome_prefix() {
        assert_eq!(extract_volume("Naruto Tome 3"), Some(3));
        assert_eq!(extract_volume("Asterix Tome 12"), Some(12));
    }

    #[test]
    fn extract_volume_tome_standalone() {
        // Just "Tome XX" without series name
        assert_eq!(extract_volume("Tome 05"), Some(5));
    }

    #[test]
    fn extract_volume_tome_with_series_and_dash() {
        // "Kaiju no8 - Tome 9"
        assert_eq!(extract_volume("Kaiju no8 - Tome 9"), Some(9));
    }

    #[test]
    fn extract_volume_tome_with_subtitle() {
        // "Tome 19 - Pas de Nol pour le père Grommel"
        assert_eq!(
            extract_volume("Tome 19 - Pas de Nol pour le père Grommel"),
            Some(19)
        );
    }

    #[test]
    fn extract_volume_vol_prefix() {
        assert_eq!(extract_volume("Vol.12"), Some(12));
        assert_eq!(extract_volume("Vol 5"), Some(5));
        assert_eq!(extract_volume("Volume 3"), Some(3));
        assert_eq!(extract_volume("Volume3"), Some(3));
    }

    #[test]
    fn extract_volume_hash_prefix() {
        assert_eq!(extract_volume("Issue #42"), Some(42));
        assert_eq!(extract_volume("#007"), Some(7));
    }

    #[test]
    fn extract_volume_trailing_dash_number() {
        assert_eq!(extract_volume("Series - 05"), Some(5));
    }

    #[test]
    fn extract_volume_zero_padded() {
        assert_eq!(extract_volume("T007"), Some(7));
        assert_eq!(extract_volume("T001"), Some(1));
    }

    #[test]
    fn extract_volume_no_match() {
        assert_eq!(extract_volume("Just a title"), None);
        assert_eq!(extract_volume("No numbers here"), None);
        assert_eq!(extract_volume(""), None);
    }

    // ─── extract_volumes ─────────────────────────────────────────────────

    fn sorted(mut v: Vec<i32>) -> Vec<i32> {
        v.sort_unstable();
        v
    }

    #[test]
    fn extract_volumes_individual() {
        assert_eq!(sorted(extract_volumes("One Piece T05")), vec![5]);
        assert_eq!(sorted(extract_volumes("Naruto Tome 12")), vec![12]);
        assert_eq!(sorted(extract_volumes("Vol.03")), vec![3]);
        assert_eq!(sorted(extract_volumes("v07")), vec![7]);
    }

    #[test]
    fn extract_volumes_trailing_bare_number() {
        // Series name + space + number + extension
        assert_eq!(
            sorted(extract_volumes("Shangri-La Frontier 18.cbz")),
            vec![18]
        );
        assert_eq!(
            sorted(extract_volumes("Shangri-la Frontier 01.cbz")),
            vec![1]
        );
        assert_eq!(sorted(extract_volumes("My Series 7.pdf")), vec![7]);
        assert_eq!(sorted(extract_volumes("Some manga 123.epub")), vec![123]);
        // Should NOT trigger when a prefix-based pattern already matched
        assert_eq!(sorted(extract_volumes("One Piece T05 18.cbz")), vec![5]);
        // No space before digits → reject (avoids matching ISBN-like junk)
        assert_eq!(sorted(extract_volumes("Series123.cbz")), Vec::<i32>::new());
        // Filename without extension — covers the rename bug where file_stem()
        // strips the extension before calling extract_volume.
        assert_eq!(sorted(extract_volumes("Shangri-La Frontier 18")), vec![18]);
        assert_eq!(sorted(extract_volumes("Some Series 7")), vec![7]);
    }

    #[test]
    fn extract_volume_works_after_file_stem() {
        // Reproduces the rename flow: filename with and without extension
        // must yield the same volume number.
        assert_eq!(extract_volume("Shangri-La Frontier 18.cbz"), Some(18));
        assert_eq!(extract_volume("Shangri-La Frontier 18"), Some(18));
        assert_eq!(extract_volume("Shangri-la Frontier 01"), Some(1));
        assert_eq!(extract_volume("Shangri-La Frontier 24"), Some(24));
    }

    #[test]
    fn extract_volume_underscore_delimited() {
        // Pattern D: _NN_ or _NN@ (Telegram channel filenames)
        assert_eq!(
            extract_volume("Black_Clover_29_Une_Nuit_Sans_Matin_Yûki_Tabata_2021@BD_fr.cbz"),
            Some(29)
        );
        assert_eq!(
            extract_volume("Black_Clover_30_Bonne_Nouvelle_Yûki_Tabata_2022@BD_fr.cbz"),
            Some(30)
        );
        assert_eq!(extract_volume("One_Piece_1_Romance_Dawn@ch.cbz"), Some(1));
        assert_eq!(
            extract_volume("My_Hero_Academia_42_La_Cavalerie_Est_Là_Kōhei_Horikoshi_2025@BD.cbz"),
            Some(42)
        );
        // 4-digit numbers (years) must not match as volume
        assert_eq!(extract_volume("Series_2021@channel.cbz"), None);
    }

    #[test]
    fn extract_volume_before_author_paren() {
        // Pattern E: " NN (" or " NN@" (author-in-parentheses format)
        assert_eq!(
            extract_volume("Détective Conan 02 (Gosho AOYAMA)@BD_fr.cbz"),
            Some(2)
        );
        assert_eq!(
            extract_volume("Hunter x Hunter 36 (Yoshihiro Togashi)@ch.cbz"),
            Some(36)
        );
        assert_eq!(
            extract_volume("Blacksad 1 (Juan Díaz Canales)@ch.cbz"),
            Some(1)
        );
        // Should not false-positive on series numbers that are not volumes
        assert_eq!(extract_volume("Les 7 Secrets (Author)@ch.cbz"), None);
    }

    #[test]
    fn extract_volumes_glued_t_prefix() {
        // T directly attached to the series name (no separator), 2+ digits
        assert_eq!(
            sorted(extract_volumes("Shangri-la FrontierT01.cbz")),
            vec![1]
        );
        assert_eq!(
            sorted(extract_volumes("Shangri-la Frontiert05.cbz")),
            vec![5]
        );
        assert_eq!(sorted(extract_volumes("NarutoT42")), vec![42]);
        // 1-digit glued form NOT matched (too risky for false positives)
        assert_eq!(sorted(extract_volumes("WordT5")), Vec::<i32>::new());
        // Pure word with no digits not affected
        assert_eq!(sorted(extract_volumes("Asterisk")), Vec::<i32>::new());
        // Digit-then-T-then-digit not matched (e.g., MP3T01 — looks suspicious)
        assert_eq!(sorted(extract_volumes("MP3T01")), Vec::<i32>::new());
    }

    #[test]
    fn extract_volumes_range_dot_separator() {
        let v = sorted(extract_volumes("One Piece T01.T15"));
        assert_eq!(v, (1..=15).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_range_dot_with_brackets() {
        let v = sorted(extract_volumes("Naruto [T001.T104]"));
        assert_eq!(v.len(), 104);
        assert_eq!(v[0], 1);
        assert_eq!(v[103], 104);
    }

    #[test]
    fn extract_volumes_range_dash_separator() {
        let v = sorted(extract_volumes("Dragon Ball T01-T10"));
        assert_eq!(v, (1..=10).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_range_french_a_grave() {
        let v = sorted(extract_volumes("Astérix Tome 01 à Tome 05"));
        assert_eq!(v, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn extract_volumes_range_long_prefix() {
        let v = sorted(extract_volumes("Naruto Tome01.Tome15"));
        assert_eq!(v, (1..=15).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_range_dash_bare_end() {
        let v = sorted(extract_volumes(
            "Compressé.Demon.Slayer.en.couleurs.T17-23.CBZ.Team.Chromatique",
        ));
        assert_eq!(v, (17..=23).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_no_false_positive_version_string() {
        let v = extract_volumes("tool v2.0 release");
        assert!(!v.contains(&0) || v.len() == 1);
    }

    #[test]
    fn extract_volumes_tome_hash_with_accented_chars() {
        let v = sorted(extract_volumes(
            "[Compressé] One Piece [Team Chromatique] - Tome #097 - [V2].cbz",
        ));
        assert!(v.contains(&97), "expected 97 in {:?}", v);
        assert!(
            !v.contains(&2),
            "[V2] should not be extracted as volume 2: {:?}",
            v
        );
    }

    #[test]
    fn extract_volumes_version_in_brackets_ignored() {
        let v = extract_volumes("Naruto T05 [V2].cbz");
        assert_eq!(v, vec![5]);
    }

    #[test]
    fn extract_volumes_tome_hash_single_digit() {
        let v = sorted(extract_volumes(
            "[Compressé] One Piece [Team Chromatique] - Tome #003 (Perfect Edition).cbz",
        ));
        assert!(v.contains(&3), "expected 3 in {:?}", v);
    }

    #[test]
    fn extract_volumes_bare_number_between_dashes() {
        let v = extract_volumes("Les Géants - 07 - Moon.cbz");
        assert_eq!(v, vec![7]);
    }

    #[test]
    fn extract_volumes_bare_number_dash_then_dot() {
        let v = extract_volumes("Les Géants - 07.cbz");
        assert_eq!(v, vec![7]);
    }

    #[test]
    fn extract_volumes_bare_number_at_start_dot() {
        let v = extract_volumes("06. yatho.cbz");
        assert_eq!(v, vec![6]);
    }

    #[test]
    fn extract_volumes_bare_number_at_start_dash() {
        let v = extract_volumes("07 - Moon.cbz");
        assert_eq!(v, vec![7]);
    }

    #[test]
    fn extract_volumes_bare_number_no_false_positive_with_prefix() {
        let v = extract_volumes("Naruto T05 - some 99 extra.cbz");
        assert_eq!(v, vec![5], "should only find T05, not bare 99");
    }

    #[test]
    fn extract_volumes_dash_number_dash_no_spaces() {
        assert_eq!(
            sorted(extract_volumes("Largo Winch -21- L'étoile du matin.cbr")),
            vec![21]
        );
        assert_eq!(sorted(extract_volumes("Largo Winch -05- H.cbr")), vec![5]);
        assert_eq!(
            sorted(extract_volumes("Largo winch -22- Les Voiles écarlates.cbz")),
            vec![22]
        );
    }

    #[test]
    fn extract_volumes_empty_string() {
        assert_eq!(extract_volumes(""), Vec::<i32>::new());
    }

    #[test]
    fn extract_volumes_no_volumes_plain_text() {
        assert_eq!(
            extract_volumes("Some random title without volumes"),
            Vec::<i32>::new()
        );
    }

    #[test]
    fn extract_volumes_hash_prefix() {
        assert_eq!(sorted(extract_volumes("Issue #42")), vec![42]);
    }

    #[test]
    fn extract_volumes_multiple_individual() {
        let v = sorted(extract_volumes("Pack Naruto T01 T05 T10"));
        assert_eq!(v, vec![1, 5, 10]);
    }

    #[test]
    fn extract_volumes_vol_space_prefix() {
        assert_eq!(sorted(extract_volumes("Vol 7 - Special")), vec![7]);
    }

    #[test]
    fn extract_volumes_vol_dot_prefix() {
        assert_eq!(sorted(extract_volumes("Vol.12 collector")), vec![12]);
    }

    #[test]
    fn extract_volumes_leading_zeros() {
        assert_eq!(sorted(extract_volumes("T0001")), vec![1]);
        assert_eq!(sorted(extract_volumes("Tome 007")), vec![7]);
    }

    #[test]
    fn extract_volumes_range_single_volume_not_range() {
        let v = extract_volumes("One Piece T05 [FR]");
        assert_eq!(v, vec![5]);
    }

    #[test]
    fn extract_volumes_range_large_gap_rejected() {
        let v = extract_volumes("Archive T001.T999");
        assert!(v.len() <= 2, "should not expand huge range, got {:?}", v);
    }

    #[test]
    fn extract_volumes_range_equal_numbers_not_expanded() {
        let v = sorted(extract_volumes("Pack T05.T05"));
        assert_eq!(v, vec![5]);
    }

    #[test]
    fn extract_volumes_range_reversed_not_expanded() {
        let v = sorted(extract_volumes("Pack T10.T05"));
        assert_eq!(v, vec![5, 10]);
    }

    #[test]
    fn extract_volumes_unicode_accented_series_name() {
        let v = sorted(extract_volumes("Série Éphémère T03 - Résumé.cbz"));
        assert_eq!(v, vec![3]);
    }

    #[test]
    fn extract_volumes_tome_with_dot_separator() {
        let v = sorted(extract_volumes("Series Tome.05.cbz"));
        assert_eq!(v, vec![5]);
    }

    #[test]
    fn extract_volumes_tome_with_underscore_separator() {
        assert_eq!(extract_volumes("Cyborgs_Tome_01_Ronin_fr.cbz"), vec![1]);
        assert_eq!(extract_volumes("Series_Tome_12.pdf"), vec![12]);
    }

    #[test]
    fn extract_volumes_t_with_underscore_separator() {
        assert_eq!(extract_volumes("Series_T_05.cbz"), vec![5]);
        assert_eq!(extract_volumes("Series_T05.cbz"), vec![5]);
    }

    #[test]
    fn extract_volumes_tome_space_standalone() {
        assert_eq!(extract_volumes("Tome 05.cbz"), vec![5]);
    }

    #[test]
    fn extract_volumes_tome_in_series_title() {
        assert_eq!(extract_volumes("Kaiju no8 - Tome 9.cbz"), vec![9]);
    }

    #[test]
    fn extract_volumes_tome_with_subtitle() {
        assert_eq!(
            extract_volumes("Tome 19 - Pas de Nol pour le père Grommel.pdf"),
            vec![19]
        );
    }

    #[test]
    fn extract_volumes_v_prefix_not_in_brackets() {
        assert_eq!(sorted(extract_volumes("Series v03 [1080p]")), vec![3]);
    }

    #[test]
    fn extract_volumes_bare_number_dash_end_of_string() {
        let v = extract_volumes("Series - 12");
        assert_eq!(v, vec![12]);
    }

    #[test]
    fn extract_volumes_bare_number_at_start_space() {
        let v = extract_volumes("03 title.cbz");
        assert_eq!(v, vec![3]);
    }

    #[test]
    fn extract_volumes_bare_number_at_start_underscore() {
        let v = extract_volumes("34_Increvables.pdf");
        assert_eq!(v, vec![34]);
    }

    #[test]
    fn extract_volumes_bare_number_underscore_various() {
        assert_eq!(extract_volumes("1_Tom_tom.pdf"), vec![1]);
        assert_eq!(extract_volumes("07_Dr_le_de_cirque.pdf"), vec![7]);
    }

    #[test]
    fn extract_volumes_range_with_spaces_around_dash() {
        let v = sorted(extract_volumes("Pack T01 - T10 [FR]"));
        assert_eq!(v, (1..=10).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_range_with_a_no_accent() {
        let v = sorted(extract_volumes("Tom Tom et Nana - T01 a T34 [PDF] Fr"));
        assert_eq!(v, (1..=34).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_range_with_a_grave_and_spaces() {
        let v = sorted(extract_volumes("Collection Tome 1 à Tome 5"));
        assert_eq!(v, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn extract_volumes_does_not_duplicate() {
        let v = extract_volumes("T05 - also Tome 05");
        assert_eq!(v.iter().filter(|&&x| x == 5).count(), 1);
    }

    // ─── read_bare_number ───────────────────────────────────────────────

    #[test]
    fn read_bare_number_at_start() {
        let chars: Vec<char> = "42abc".chars().collect();
        assert_eq!(read_bare_number(&chars, 0), Some((42, 2)));
    }

    #[test]
    fn read_bare_number_no_digits() {
        let chars: Vec<char> = "abc".chars().collect();
        assert_eq!(read_bare_number(&chars, 0), None);
    }

    #[test]
    fn read_bare_number_at_offset() {
        let chars: Vec<char> = "abc123def".chars().collect();
        assert_eq!(read_bare_number(&chars, 3), Some((123, 6)));
    }

    // ─── read_vol_prefix_number ─────────────────────────────────────────

    #[test]
    fn read_vol_prefix_number_tome() {
        let chars: Vec<char> = "tome 05 extra".chars().collect();
        assert_eq!(read_vol_prefix_number(&chars, 0), Some((5, 7)));
    }

    #[test]
    fn read_vol_prefix_number_t_prefix() {
        let chars: Vec<char> = "t12".chars().collect();
        assert_eq!(read_vol_prefix_number(&chars, 0), Some((12, 3)));
    }

    #[test]
    fn read_vol_prefix_number_boundary_check() {
        let chars: Vec<char> = "at12".chars().collect();
        assert_eq!(read_vol_prefix_number(&chars, 1), None);
    }

    #[test]
    fn read_vol_prefix_number_no_digits_after_prefix() {
        let chars: Vec<char> = "tome abc".chars().collect();
        assert_eq!(read_vol_prefix_number(&chars, 0), None);
    }

    #[test]
    fn read_vol_prefix_number_hash() {
        let chars: Vec<char> = "#007 extra".chars().collect();
        assert_eq!(read_vol_prefix_number(&chars, 0), Some((7, 4)));
    }

    // ─── extract_series ──────────────────────────────────────────────────

    #[test]
    fn extract_series_simple() {
        let path = Path::new("/libraries/manga/One Piece/T01.cbz");
        let root = Path::new("/libraries/manga");
        assert_eq!(extract_series(path, root), Some("One Piece".to_string()));
    }

    #[test]
    fn extract_series_nested_non_hs_subfolder() {
        let path = Path::new("/libraries/bd/Asterix/subfolder/file.cbz");
        let root = Path::new("/libraries/bd");
        // Non-HS subfolder: use immediate parent as series
        assert_eq!(extract_series(path, root), Some("subfolder".to_string()));
    }

    #[test]
    fn extract_series_file_at_root() {
        let path = Path::new("/libraries/manga/standalone.cbz");
        let root = Path::new("/libraries/manga");
        // File directly in root → no series directory
        assert_eq!(extract_series(path, root), None);
    }

    #[test]
    fn extract_series_unrelated_path() {
        let path = Path::new("/other/path/file.cbz");
        let root = Path::new("/libraries/manga");
        // Path doesn't start with root
        assert_eq!(extract_series(path, root), None);
    }

    // ─── parse_metadata_fast ─────────────────────────────────────────────

    #[test]
    fn parse_metadata_fast_extracts_all() {
        let path = Path::new("/libraries/manga/One Piece/One Piece T05.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.title, "One Piece T05");
        assert_eq!(meta.series, Some("One Piece".to_string()));
        assert_eq!(meta.volume, Some(5));
        assert_eq!(meta.page_count, None);
    }

    #[test]
    fn parse_metadata_fast_no_volume() {
        let path = Path::new("/libraries/bd/Asterix/Asterix le Gaulois.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.title, "Asterix le Gaulois");
        assert_eq!(meta.series, Some("Asterix".to_string()));
        assert_eq!(meta.volume, None);
    }

    #[test]
    fn parse_metadata_fast_no_extension() {
        let path = Path::new("/libraries/manga/Series/Untitled");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.title, "Untitled");
        assert_eq!(meta.series, Some("Series".to_string()));
    }

    // ─── detect_format ───────────────────────────────────────────────────

    #[test]
    fn detect_format_case_insensitive() {
        assert_eq!(detect_format(Path::new("test.PDF")), Some(BookFormat::Pdf));
        assert_eq!(
            detect_format(Path::new("test.Epub")),
            Some(BookFormat::Epub)
        );
        assert_eq!(detect_format(Path::new("test.CbR")), Some(BookFormat::Cbr));
    }

    // ─── BookFormat::as_str ──────────────────────────────────────────────

    #[test]
    fn book_format_as_str() {
        assert_eq!(BookFormat::Cbz.as_str(), "cbz");
        assert_eq!(BookFormat::Cbr.as_str(), "cbr");
        assert_eq!(BookFormat::Pdf.as_str(), "pdf");
        assert_eq!(BookFormat::Epub.as_str(), "epub");
    }

    // ─── clean_title ─────────────────────────────────────────────────────

    #[test]
    fn clean_title_removes_volume_patterns() {
        assert_eq!(clean_title("One Piece T05"), "One Piece");
        assert_eq!(clean_title("Naruto Vol.12"), "Naruto");
        assert_eq!(clean_title("Series Volume 3"), "Series");
        assert_eq!(clean_title("Issue #42"), "Issue");
        assert_eq!(clean_title("Series - 05"), "Series");
    }

    #[test]
    fn clean_title_no_volume() {
        assert_eq!(clean_title("Just a title"), "Just a title");
    }

    // ─── extract_hs_info ────────────────────────────────────────────────

    #[test]
    fn hs_basic() {
        let (num, _) = extract_hs_info("Asterix HS1").unwrap();
        assert_eq!(num, Some(1));
    }

    #[test]
    fn hs_dot_separator() {
        let (num, _) = extract_hs_info("Asterix HS.02").unwrap();
        assert_eq!(num, Some(2));
    }

    #[test]
    fn hs_space_separator() {
        let (num, _) = extract_hs_info("Asterix HS 03").unwrap();
        assert_eq!(num, Some(3));
    }

    #[test]
    fn hs_no_number() {
        let (num, cleaned) = extract_hs_info("Asterix HS").unwrap();
        assert_eq!(num, None);
        assert_eq!(cleaned, "Asterix");
    }

    #[test]
    fn hs_at_end_with_prefix() {
        let result = extract_hs_info("Boruto - Two Blue Vortex HS");
        assert!(
            result.is_some(),
            "should detect HS at end of 'Boruto - Two Blue Vortex HS'"
        );
        let (num, cleaned) = result.unwrap();
        assert_eq!(num, None);
        assert_eq!(cleaned, "Boruto - Two Blue Vortex");
    }

    #[test]
    fn hors_serie_with_accent() {
        let (num, _) = extract_hs_info("Naruto Hors-Série 3").unwrap();
        assert_eq!(num, Some(3));
    }

    #[test]
    fn hors_serie_without_accent() {
        let (num, _) = extract_hs_info("Naruto Hors Serie").unwrap();
        assert_eq!(num, None);
    }

    #[test]
    fn special_with_accent() {
        let (num, _) = extract_hs_info("One Piece Spécial 2").unwrap();
        assert_eq!(num, Some(2));
    }

    #[test]
    fn special_without_accent() {
        let (num, _) = extract_hs_info("One Piece Special").unwrap();
        assert_eq!(num, None);
    }

    #[test]
    fn bonus_pattern() {
        let (num, cleaned) = extract_hs_info("Donjon Bonus - Clefs en Mains").unwrap();
        assert_eq!(num, None);
        assert!(!cleaned.is_empty());
    }

    #[test]
    fn hs_not_in_word() {
        // "hsk" should not match "hs"
        assert!(extract_hs_info("The HSK Guide").is_none());
    }

    #[test]
    fn regular_volume_not_hs() {
        assert!(extract_hs_info("Asterix T05").is_none());
        assert!(extract_hs_info("Naruto Tome 12").is_none());
    }

    #[test]
    fn hs_volume_type_in_parse() {
        let path = Path::new("/libraries/test/Asterix/Asterix HS2.cbz");
        let root = Path::new("/libraries/test");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.volume_type, VolumeType::Hs);
        assert_eq!(meta.volume, Some(2));
    }

    #[test]
    fn regular_volume_type_in_parse() {
        let path = Path::new("/libraries/test/Asterix/Asterix T05.cbz");
        let root = Path::new("/libraries/test");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.volume_type, VolumeType::Regular);
        assert_eq!(meta.volume, Some(5));
    }

    #[test]
    fn hs_subfolder_keeps_parent_series() {
        let path = Path::new("/libraries/manga/Boruto/Hors-Série/Boruto HS.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Boruto".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Hs);
    }

    #[test]
    fn specials_subfolder_keeps_parent_series() {
        let path = Path::new("/libraries/bd/Asterix/Specials/Asterix Special 2.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Asterix".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Hs);
        assert_eq!(meta.volume, Some(2));
    }

    #[test]
    fn nested_series_uses_immediate_parent() {
        let path = Path::new("/libraries/manga/Shonen/Dragon Ball/Dragon Ball T01.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Dragon Ball".to_string()));
    }

    // ─── oneshot folder ───────────────────────────────────────────────────────

    #[test]
    fn oneshot_folder_uses_filename_as_series() {
        let path = Path::new("/libraries/bd/Oneshots/Blacksad.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Blacksad".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Oneshot);
        assert_eq!(meta.volume, None);
    }

    #[test]
    fn oneshot_folder_case_insensitive() {
        let path = Path::new("/libraries/bd/ONESHOTS/My One Shot.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("My One Shot".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Oneshot);
    }

    #[test]
    fn oneshot_folder_hyphenated() {
        let path = Path::new("/libraries/bd/One-Shots/Persepolis.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Persepolis".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Oneshot);
    }

    #[test]
    fn oneshot_folder_underscore_prefix() {
        let path = Path::new("/libraries/bd/_Oneshots/Maus.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Maus".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Oneshot);
    }

    #[test]
    fn non_oneshot_folder_not_affected() {
        // "Oneshots" nested 2 levels deep — NOT treated as oneshot folder
        let path = Path::new("/libraries/bd/MySeries/Oneshots/Extra.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        // Not oneshot type — the 2-level check correctly prevents matching
        assert_ne!(meta.volume_type, VolumeType::Oneshot);
        // Series is the immediate parent dir (normal behaviour)
        assert_eq!(meta.series, Some("Oneshots".to_string()));
    }

    #[test]
    fn nested_series_with_hs_subfolder() {
        let path = Path::new("/libraries/manga/Shonen/Dragon Ball/Hors-Série/Dragon Ball HS1.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Dragon Ball".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Hs);
    }

    #[test]
    fn flat_series_still_works() {
        let path = Path::new("/libraries/manga/One Piece/One Piece T05.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("One Piece".to_string()));
    }

    #[test]
    fn file_at_library_root_is_unclassified() {
        let path = Path::new("/libraries/manga/standalone.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, None);
    }

    #[test]
    fn bonus_subfolder_keeps_parent_series() {
        let path = Path::new("/libraries/bd/Tintin/Bonus/Tintin Bonus.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Tintin".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Hs);
    }

    #[test]
    fn extras_subfolder_keeps_parent_series() {
        let path = Path::new("/libraries/manga/Naruto/Extras/Naruto Special.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Naruto".to_string()));
    }

    // ─── extract_int_info ─────────────────────────────────────────────

    #[test]
    fn int_basic() {
        let (num, cleaned) = extract_int_info("Dragon Ball INT1").unwrap();
        assert_eq!(num, Some(1));
        assert_eq!(cleaned, "Dragon Ball");
    }

    #[test]
    fn int_no_number() {
        let (num, cleaned) = extract_int_info("Dragon Ball INT").unwrap();
        assert_eq!(num, None);
        assert_eq!(cleaned, "Dragon Ball");
    }

    #[test]
    fn int_dot_separator() {
        let (num, _) = extract_int_info("Asterix INT.02").unwrap();
        assert_eq!(num, Some(2));
    }

    #[test]
    fn int_space_separator() {
        let (num, _) = extract_int_info("Asterix INT 3").unwrap();
        assert_eq!(num, Some(3));
    }

    #[test]
    fn inths_pattern() {
        let (num, cleaned) = extract_int_info("Dragon Ball INTHS").unwrap();
        assert_eq!(num, None);
        assert_eq!(cleaned, "Dragon Ball");
    }

    #[test]
    fn inths_with_number() {
        let (num, cleaned) = extract_int_info("Dragon Ball INTHS1").unwrap();
        assert_eq!(num, Some(1));
        assert_eq!(cleaned, "Dragon Ball");
    }

    #[test]
    fn integrale_full_word() {
        let (num, cleaned) = extract_int_info("Naruto Intégrale 5").unwrap();
        assert_eq!(num, Some(5));
        assert_eq!(cleaned, "Naruto");
    }

    #[test]
    fn integrale_without_accent() {
        let (num, _) = extract_int_info("Naruto Integrale 3").unwrap();
        assert_eq!(num, Some(3));
    }

    #[test]
    fn int_not_in_word() {
        // "international" should NOT match
        assert!(extract_int_info("International Guide").is_none());
    }

    #[test]
    fn int_not_interest() {
        assert!(extract_int_info("Interesting Story").is_none());
    }

    #[test]
    fn int_volume_type_in_parse() {
        let path = Path::new("/libraries/test/Asterix/Asterix INT2.cbz");
        let root = Path::new("/libraries/test");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.volume_type, VolumeType::Integral);
        assert_eq!(meta.volume, Some(2));
    }

    #[test]
    fn inths_volume_type_in_parse() {
        let path = Path::new("/libraries/test/Asterix/Asterix INTHS1.cbz");
        let root = Path::new("/libraries/test");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.volume_type, VolumeType::Integral);
        assert_eq!(meta.volume, Some(1));
    }

    #[test]
    fn integrales_subfolder_keeps_parent_series() {
        let path = Path::new("/libraries/manga/Naruto/Intégrales/Naruto INT3.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Naruto".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Integral);
        assert_eq!(meta.volume, Some(3));
    }

    #[test]
    fn is_hs_subfolder_patterns() {
        assert!(is_hs_subfolder("Hors-Série"));
        assert!(is_hs_subfolder("hors-serie"));
        assert!(is_hs_subfolder("HS"));
        assert!(is_hs_subfolder("Specials"));
        assert!(is_hs_subfolder("Bonus"));
        assert!(is_hs_subfolder("Extras"));
        assert!(is_hs_subfolder("extra"));
        assert!(!is_hs_subfolder("Dragon Ball"));
        assert!(!is_hs_subfolder("Tome 1"));
        assert!(!is_hs_subfolder("Shonen"));
    }

    // ─── CBZ_INDEX_CACHE invalidation on mtime change ─────────────────────
    //
    // Regression test for a bug where the global cache was keyed only on the
    // file path. After replacing a CBZ in place (different content, same path),
    // the parser kept returning the OLD image list — leading to "page out of
    // range" errors and stale page renders.

    fn write_single_image_cbz(path: &Path, image_name: &str) -> std::io::Result<()> {
        use std::io::Write;
        let file = std::fs::File::create(path)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file(image_name, opts)
            .map_err(std::io::Error::other)?;
        // Minimal valid JPEG: just the SOI/EOI markers — enough to satisfy is_image_name.
        zip.write_all(&[0xFF, 0xD8, 0xFF, 0xD9])?;
        zip.finish().map_err(std::io::Error::other)?;
        Ok(())
    }

    #[test]
    fn cbz_index_cache_invalidates_on_mtime_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbz");

        // First version: a single image "001.jpg". Read once to populate cache.
        write_single_image_cbz(&path, "001.jpg").unwrap();
        let mtime_before = std::fs::metadata(&path).unwrap().modified().unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names = get_cbz_image_index(&path, &mut archive);
        assert_eq!(names, vec!["001.jpg".to_string()]);

        // Replace the file in place with a new image. Force a future mtime so
        // the test is robust regardless of filesystem mtime resolution.
        write_single_image_cbz(&path, "002.jpg").unwrap();
        let new_mtime = mtime_before + std::time::Duration::from_secs(10);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(new_mtime)
            .unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names = get_cbz_image_index(&path, &mut archive);
        assert_eq!(
            names,
            vec!["002.jpg".to_string()],
            "cache must invalidate when file mtime changes"
        );
    }

    #[test]
    fn cbz_index_cache_returns_cached_on_unchanged_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbz");
        write_single_image_cbz(&path, "001.jpg").unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let first = get_cbz_image_index(&path, &mut archive);

        // Second call — mtime unchanged, should return cached value.
        let file = std::fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let second = get_cbz_image_index(&path, &mut archive);

        assert_eq!(first, second);
    }
}
