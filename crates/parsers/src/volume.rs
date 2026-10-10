use std::path::Path;
use std::sync::OnceLock;

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

/// Internal representation shared by the public volume projections.
///
/// It keeps the source title alongside the deduplicated numbers so title
/// cleanup can use the same parsing entry point without affecting consumers
/// that only need volumes.
pub struct ParsedVolumeMarkers<'a> {
    pub title: &'a str,
    pub volumes: Vec<i32>,
}

/// Parse individual and range volume markers from a title or filename.
///
/// Handles individual volumes (T01, Tome 01, Vol. 01, v01, #01) and also
/// **range packs** like `T01.T15`, `[T001.T104]`, `T01-T15`, `Tome 01 à Tome 15`
/// — the range is expanded so every volume in [start..=end] is returned.
pub fn parse_volume_markers(title: &str) -> ParsedVolumeMarkers<'_> {
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
pub fn is_oneshot_folder(name: &str) -> bool {
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
pub fn is_hs_subfolder(name: &str) -> bool {
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

pub fn extract_series(path: &Path, library_root: &Path) -> Option<String> {
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

/// Legacy helper: cleaned display title derived from a filename.
#[allow(dead_code)]
pub fn clean_title(filename: &str) -> String {
    parse_volume_markers(filename).cleaned_title()
}

impl ParsedVolumeMarkers<'_> {
    /// Remove the marker forms historically stripped from display titles.
    ///
    /// This intentionally keeps its conservative legacy cleanup policy: not
    /// every number recognized for file matching is suitable for removal from
    /// a display title.
    pub fn cleaned_title(&self) -> String {
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
