//! Fourtoutici — francophone community file library.
//!
//! Unlike the other two providers listed in issue #64, fourtoutici exposes a
//! native JSON API (no account, no Cloudflare), so no DOM scraping is needed:
//!
//! * search  → `GET {base}/backend/api/files.php?q=<query>`
//! * download → `GET {base}/backend/api/download.php?file_id=<id>`
//!
//! The search payload is `{ "success": bool, "files": [{ "file_id", "original_name",
//! "file_name", "size", ... }] }`. `original_name` is the human-readable upload name
//! (e.g. `"BD Astérix le Gaulois 01 Astérix le Gaulois.pdf"`) and is what volume
//! extraction runs on.

use serde::Deserialize;

use super::{DdlProvider, DdlProviderFile, DdlProviderMeta, BROWSER_USER_AGENT, DISCOVERY_TIMEOUT};

const BASE_URL: &str = "https://fourtoutici.cc";
const USER_AGENT: &str = BROWSER_USER_AGENT;

pub struct FourtouticiProvider;

#[derive(Debug, Deserialize)]
struct FilesResponse {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    files: Vec<RemoteFile>,
}

#[derive(Debug, Deserialize)]
struct RemoteFile {
    file_id: String,
    /// Human-readable upload name; preferred for volume extraction.
    #[serde(default)]
    original_name: Option<String>,
    /// Random internal name; only used when `original_name` is missing.
    #[serde(default)]
    file_name: Option<String>,
    #[serde(default)]
    size: Option<i64>,
}

/// fourtoutici uploads are named `<kind> <series> <tome> <title> – <authors>.<ext>`,
/// e.g. `BD Astérix le Gaulois 41 Astérix en Lusitanie – René Goscinny.pdf`.
///
/// The bare tome (`41`) is not one of the volume patterns `parsers` recognises
/// (`T41`, `Vol. 41`, `#41`, …), so we rewrite the name to `<series> T<tome>`
/// before it reaches the shared matcher. Range packs (`One Piece 01 à T114`) keep
/// their start/end so the whole span is expanded instead of only the first tome.
/// Only the value used for matching is rewritten; the UI keeps showing the
/// original filename.
///
/// Returns `None` when no unambiguous standalone tome (1-3 digits) is found, in
/// which case the plain filename is matched instead.
pub(super) fn normalize_for_match(original: &str) -> Option<String> {
    let stem = original.rsplit_once('.').map_or(original, |(s, _)| s);
    // Drop the trailing " – <authors>" / " — <authors>" segment.
    let head = stem.split(['–', '—']).next().unwrap_or(stem);

    let tokens: Vec<&str> = head.split_whitespace().collect();
    let index = tokens.iter().position(|token| is_bare_tome_token(token))?;

    let mut series: Vec<&str> = tokens[..index].to_vec();
    if series.first().is_some_and(|token| is_kind_prefix(token)) {
        series.remove(0);
    }
    while let Some(last) = series.last() {
        let cleaned = last.trim_matches(['-', ':', '–']);
        let lower = cleaned.to_ascii_lowercase();
        if cleaned.is_empty() || matches!(lower.as_str(), "t" | "tome" | "vol" | "volume" | "n") {
            series.pop();
        } else {
            break;
        }
    }

    if series.is_empty() {
        return None;
    }
    let start = tome_digits(tokens[index])?;

    // A range continuation right after the first tome (`01 à T114`, `00 a 30`)
    // must survive as a real range, otherwise only the first tome is matched.
    if let Some(end) = range_end(&tokens, index) {
        return Some(format!("{} T{} à T{}", series.join(" "), start, end));
    }

    Some(format!("{} T{}", series.join(" "), start))
}

/// A standalone bare tome token (`01`, `5`, `114`). A token carrying letters
/// such as `no8` or `T114` is **not** the standalone tome of the upload name —
/// treating it as one would truncate the series (e.g. `Kaiju No.8`).
fn is_bare_tome_token(token: &str) -> bool {
    let cleaned = token.trim_matches(|c: char| !c.is_alphanumeric());
    !cleaned.is_empty() && cleaned.len() <= 3 && cleaned.chars().all(|c| c.is_ascii_digit())
}

/// Digit run of a tome token, preserving zero-padding (`01` → `01`,
/// `T114` → `114`). `None` when the token holds no usable 1-3 digit number.
fn tome_digits(token: &str) -> Option<&str> {
    let cleaned = token.trim_matches(|c: char| !c.is_ascii_digit());
    (!cleaned.is_empty() && cleaned.len() <= 3 && cleaned.chars().all(|c| c.is_ascii_digit()))
        .then_some(cleaned)
}

/// Detect a `start <connector> end` tome range right after `index`
/// (`01 à T114`, `00 a 30`, `01 - 05`). Returns the end tome digits.
fn range_end<'a>(tokens: &[&'a str], index: usize) -> Option<&'a str> {
    let connector = tokens.get(index + 1)?;
    if !matches!(
        connector.to_lowercase().as_str(),
        "à" | "a" | "-" | "–" | "to" | "au"
    ) {
        return None;
    }
    tome_digits(tokens.get(index + 2)?)
}

/// Leading upload-kind marker (`BD`, `EBOOK`, …) dropped from the series name.
fn is_kind_prefix(token: &str) -> bool {
    matches!(
        token.to_ascii_uppercase().as_str(),
        "BD" | "EBOOK" | "MANGA" | "COMIC" | "COMICS" | "BOOK" | "LIVRE" | "ROMAN"
    )
}

/// Parse a `files.php` body into provider entries.
///
/// Kept separate from the HTTP call so it can be unit-tested without a network.
fn parse_files(body: &str) -> anyhow::Result<Vec<DdlProviderFile>> {
    let parsed: FilesResponse = serde_json::from_str(body)
        .map_err(|e| anyhow::anyhow!("invalid fourtoutici payload: {e}"))?;
    if !parsed.success {
        anyhow::bail!("fourtoutici rejected the query");
    }

    Ok(parsed
        .files
        .into_iter()
        .filter_map(|file| {
            let filename = file.original_name.or(file.file_name)?;
            if filename.trim().is_empty() {
                return None;
            }
            let match_name = normalize_for_match(&filename);
            Some(DdlProviderFile {
                external_id: file.file_id,
                filename,
                match_name,
                size: file.size,
            })
        })
        .collect())
}

#[async_trait::async_trait]
impl DdlProvider for FourtouticiProvider {
    fn meta(&self) -> DdlProviderMeta {
        DdlProviderMeta {
            id: "fourtoutici",
            label: "Fourtoutici",
            kind: "json",
            description: "Bibliothèque communautaire francophone (API JSON, sans compte).",
            requires_auth: false,
            default_base_url: BASE_URL,
        }
    }

    async fn search(
        &self,
        base_url: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<DdlProviderFile>> {
        let client = stripstream_core::http::build_http_client_with_custom_agent(
            DISCOVERY_TIMEOUT,
            USER_AGENT,
        )?;

        let url = format!("{}/backend/api/files.php", base_url.trim_end_matches('/'));
        let response = client.get(url).query(&[("q", query)]).send().await?;
        if !response.status().is_success() {
            anyhow::bail!("fourtoutici returned HTTP {}", response.status());
        }

        let body = response.text().await?;
        let mut files = parse_files(&body)?;
        files.truncate(limit);
        Ok(files)
    }

    fn file_url(&self, base_url: &str, external_id: &str) -> String {
        format!(
            "{}/backend/api/download.php?file_id={}",
            base_url.trim_end_matches('/'),
            external_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_files_payload() {
        let body = r#"{
            "success": true,
            "files": [
                {
                    "file_id": "b51ea66c8c6d4c6ab75cf27a",
                    "file_name": "b51ea66c8c6d4c6ab75cf27a.epub",
                    "original_name": "BD Astérix le Gaulois 01 Astérix le Gaulois – Goscinny.pdf",
                    "size": 439863
                }
            ]
        }"#;

        let files = parse_files(body).expect("payload should parse");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].external_id, "b51ea66c8c6d4c6ab75cf27a");
        assert_eq!(
            files[0].filename,
            "BD Astérix le Gaulois 01 Astérix le Gaulois – Goscinny.pdf"
        );
        assert_eq!(
            files[0].match_name.as_deref(),
            Some("Astérix le Gaulois T01")
        );
        assert_eq!(files[0].size, Some(439863));
    }

    #[test]
    fn normalizes_real_fourtoutici_names() {
        // Shapes observed on the live catalogue.
        let cases = [
            (
                "BD Astérix le Gaulois 41 Astérix en Lusitanie – René Goscinny & Albert Uderzo.pdf",
                Some("Astérix le Gaulois T41"),
            ),
            (
                "BD Astérix le Gaulois 01 Astérix le Gaulois – René Goscinny & Albert Uderzo.pdf",
                Some("Astérix le Gaulois T01"),
            ),
            (
                "EBOOK Gon - Tome 5 La peur – Michael Grant.epub",
                Some("Gon T5"),
            ),
            // No standalone tome (a 4-digit year is not a tome): keep the raw name.
            (
                "BD Astérix & l'Empire du milieu, l'album du film – Fabrice Tarrin.pdf",
                None,
            ),
            ("One piece 2024 artbook.cbz", None),
        ];

        for (input, expected) in cases {
            assert_eq!(
                normalize_for_match(input),
                expected.map(str::to_string),
                "input: {input}"
            );
        }
    }

    #[test]
    fn preserves_tome_ranges() {
        // Range packs observed on the live catalogue: only the first tome is a
        // standalone token, the upper bound must survive as a real range.
        let cases = [
            (
                "MANGA One Piece 01 à T114 – Eiichirō Oda.pdf",
                "One Piece T01 à T114",
            ),
            (
                "MANGA Jujutsu Kaisen 00 a 30 PDF – Gege Akutami.pdf",
                "Jujutsu Kaisen T00 à T30",
            ),
            (
                "BD Blake et Mortimer 01 - 05 – Edgar P. Jacobs.pdf",
                "Blake et Mortimer T01 à T05",
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(
                normalize_for_match(input),
                Some(expected.to_string()),
                "input: {input}"
            );
        }
    }

    #[test]
    fn keeps_series_names_containing_digits() {
        // `No.8` is part of the series name, not the tome: the real tome is `09`.
        assert_eq!(
            normalize_for_match("MANGA Kaiju No.8 09 – Naoya Matsumoto.pdf").as_deref(),
            Some("Kaiju No.8 T09")
        );
    }

    #[test]
    fn ignores_range_connector_without_a_tome() {
        // `01 à venir` must not be read as a range.
        assert_eq!(
            normalize_for_match("BD Series 01 à venir – Auteur.pdf").as_deref(),
            Some("Series T01")
        );
    }

    #[test]
    fn falls_back_to_internal_name_and_drops_empty_entries() {
        let body = r#"{
            "success": true,
            "files": [
                {"file_id": "a", "file_name": "a.cbz"},
                {"file_id": "b", "file_name": "   "},
                {"file_id": "c"}
            ]
        }"#;

        let files = parse_files(body).expect("payload should parse");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "a.cbz");
        assert_eq!(files[0].size, None);
    }

    #[test]
    fn rejects_failed_payload() {
        let err = parse_files(r#"{"success": false, "files": []}"#).unwrap_err();
        assert!(err.to_string().contains("rejected"));
    }

    #[test]
    fn rejects_malformed_payload() {
        assert!(parse_files("not json").is_err());
    }

    #[test]
    fn builds_absolute_download_url() {
        let provider = FourtouticiProvider;
        assert_eq!(
            provider.file_url("https://fourtoutici.cc/", "abc"),
            "https://fourtoutici.cc/backend/api/download.php?file_id=abc"
        );
    }
}
