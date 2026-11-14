//! Normalization of the series `metadata_json` stored on a metadata link.
//!
//! The series sync (`metadata::field_policy`) only reads `description`,
//! `authors`, `publishers`, `start_year`, `cover_url` and `status` from the
//! stored `metadata_json`. Several providers (Google Books, OpenLibrary,
//! ComicVine, AniList, SensCritique) only expose some of them as typed
//! [`SeriesCandidate`] fields, so every server-side path that stores a link
//! (manual match, batch auto-match, refresh) goes through these helpers. This
//! mirrors what the backoffice used to do on its own before calling
//! `POST /metadata/match`.

use serde_json::{Map, Value};

use super::SeriesCandidate;

/// Typed series fields a provider candidate may carry outside its JSON blob.
#[derive(Debug, Default, Clone, Copy)]
pub struct CandidateSeriesFields<'a> {
    pub description: Option<&'a str>,
    pub authors: &'a [String],
    pub publishers: &'a [String],
    pub start_year: Option<i32>,
    pub cover_url: Option<&'a str>,
}

impl SeriesCandidate {
    /// Typed series fields of this candidate.
    pub fn series_fields(&self) -> CandidateSeriesFields<'_> {
        CandidateSeriesFields {
            description: self.description.as_deref(),
            authors: &self.authors,
            publishers: &self.publishers,
            start_year: self.start_year,
            cover_url: self.cover_url.as_deref(),
        }
    }

    /// The `metadata_json` to store on a link: the provider blob enriched with
    /// the candidate's typed series fields.
    pub fn stored_metadata_json(&self) -> Value {
        enrich_series_metadata_json(&self.metadata_json, self.series_fields())
    }
}

/// Return `metadata_json` with the non-empty typed fields written in. A typed
/// value wins over the blob (same precedence as the backoffice did); an empty
/// typed value never erases what the blob already carries.
pub fn enrich_series_metadata_json(metadata_json: &Value, fields: CandidateSeriesFields) -> Value {
    let mut object = match metadata_json {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };

    if let Some(description) = non_blank(fields.description) {
        object.insert("description".into(), Value::from(description));
    }
    let authors = non_blank_list(fields.authors);
    if !authors.is_empty() {
        object.insert("authors".into(), Value::from(authors));
    }
    let publishers = non_blank_list(fields.publishers);
    if !publishers.is_empty() {
        object.insert("publishers".into(), Value::from(publishers));
    }
    if let Some(year) = fields.start_year {
        object.insert("start_year".into(), Value::from(year));
    }
    if let Some(cover_url) = non_blank(fields.cover_url) {
        object.insert("cover_url".into(), Value::from(cover_url));
    }

    Value::Object(object)
}

/// Merge a fresh provider snapshot over the stored one (refresh).
///
/// Every key of `fresh` with a meaningful value replaces the stored value;
/// keys the provider no longer returns (absent, `null`, empty string or empty
/// array) keep their stored value, so a refresh never loses enriched fields.
pub fn merge_series_snapshot(stored: &Value, fresh: &Value) -> Value {
    let mut merged = match stored {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    match fresh {
        Value::Object(fresh) => {
            for (key, value) in fresh {
                if is_meaningful(value) || !merged.contains_key(key) {
                    merged.insert(key.clone(), value.clone());
                }
            }
        }
        _ => return Value::Object(merged),
    }
    Value::Object(merged)
}

fn is_meaningful(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        _ => true,
    }
}

fn non_blank(value: Option<&str>) -> Option<&str> {
    value.filter(|s| !s.trim().is_empty())
}

fn non_blank_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "tests/series_metadata.rs"]
mod tests;
