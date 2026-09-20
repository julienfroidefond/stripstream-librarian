use std::collections::HashSet;

use sqlx::PgPool;
use uuid::Uuid;

use crate::metadata_providers::{BookCandidate, SeriesCandidate};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Extracted series metadata fields ready for DB upsert.
pub(crate) struct SeriesFields {
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub publishers: Vec<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    pub status: Option<String>,
    pub genres: Vec<String>,
    pub cover_url: Option<String>,
}

/// Result of matching an external book to a local book.
pub(crate) struct MatchedBook<'a> {
    pub ext_book: &'a BookCandidate,
    pub local_book_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalBookForMatching {
    pub id: Uuid,
    pub volume: i32,
    pub title: String,
    pub isbn: Option<String>,
    pub volume_type: String,
}

type LocalBookRow = (Uuid, Option<i32>, String, Option<String>, String);

/// Pre-update series row returned by upsert for callers that need diff/reporting.
pub(crate) type ExistingSeriesRow = sqlx::postgres::PgRow;

/// Pre-update book row returned by push for callers that need diff/reporting.
pub(crate) type ExistingBookRow = sqlx::postgres::PgRow;

// ---------------------------------------------------------------------------
// Series field extraction
// ---------------------------------------------------------------------------

/// Extract series metadata fields from a metadata_json blob, optionally falling
/// back to a SeriesCandidate struct for fields missing from the JSON.
pub(crate) async fn extract_series_fields(
    pool: &PgPool,
    metadata_json: &serde_json::Value,
    candidate: Option<&SeriesCandidate>,
    total_volumes_override: Option<i32>,
) -> SeriesFields {
    let description = metadata_json
        .get("description")
        .and_then(|d| d.as_str())
        .map(String::from)
        .or_else(|| candidate.and_then(|c| c.description.clone()));

    let authors = extract_string_array(metadata_json, "authors")
        .or_else(|| candidate.map(|c| c.authors.clone()))
        .unwrap_or_default();

    let publishers = extract_string_array(metadata_json, "publishers")
        .or_else(|| candidate.map(|c| c.publishers.clone()))
        .unwrap_or_default();

    let start_year = metadata_json
        .get("start_year")
        .and_then(|y| y.as_i64())
        .map(|y| y as i32)
        .or_else(|| candidate.and_then(|c| c.start_year));

    let total_volumes = total_volumes_override.or_else(|| candidate.and_then(|c| c.total_volumes));

    let genres = extract_string_array(metadata_json, "genres").unwrap_or_default();

    let cover_url = metadata_json
        .get("cover_url")
        .and_then(|c| c.as_str())
        .filter(|c| !c.is_empty())
        .map(String::from)
        .or_else(|| candidate.and_then(|c| c.cover_url.clone()));

    let status = if let Some(raw) = metadata_json.get("status").and_then(|s| s.as_str()) {
        Some(super::handlers::normalize_series_status(pool, raw).await)
    } else {
        None
    };

    SeriesFields {
        description,
        authors,
        publishers,
        start_year,
        total_volumes,
        status,
        genres,
        cover_url,
    }
}

/// Extract provider rating fields from a metadata_json blob.
/// Returns (rating, rating_count, rating_scale) — all optional.
pub(crate) fn extract_provider_rating(
    metadata_json: &serde_json::Value,
) -> (Option<f64>, Option<i64>, Option<f64>) {
    let rating = metadata_json
        .get("rating")
        .and_then(|v| v.as_f64())
        .filter(|&r| r > 0.0);
    let rating_count = metadata_json.get("rating_count").and_then(|v| v.as_i64());
    let rating_scale = metadata_json.get("rating_scale").and_then(|v| v.as_f64());
    (rating, rating_count, rating_scale)
}

fn extract_string_array(json: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    json.get(key)
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .filter(|v: &Vec<String>| !v.is_empty())
}

// ---------------------------------------------------------------------------
// Series metadata upsert
// ---------------------------------------------------------------------------

/// Fetch the existing series row (for diff/reporting), then upsert series metadata
/// respecting locked_fields. Returns the pre-update row if the series existed.
pub(crate) async fn upsert_series_metadata(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    fields: &SeriesFields,
) -> Result<Option<ExistingSeriesRow>, sqlx::Error> {
    // Fetch existing state for callers that need before/after comparison
    let existing = sqlx::query(
        r#"SELECT description, publishers, start_year, total_volumes, status, authors, locked_fields
           FROM series WHERE library_id = $1 AND norm_text(name) = norm_text($2)"#,
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO series (id, library_id, name, description, publishers, start_year, total_volumes, status, authors, genres, cover_url, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW())
        ON CONFLICT (library_id, name)
        DO UPDATE SET
            description = CASE
                WHEN (series.locked_fields->>'description')::boolean IS TRUE THEN series.description
                ELSE COALESCE(NULLIF(EXCLUDED.description, ''), series.description)
            END,
            publishers = CASE
                WHEN (series.locked_fields->>'publishers')::boolean IS TRUE THEN series.publishers
                WHEN array_length(EXCLUDED.publishers, 1) > 0 THEN EXCLUDED.publishers
                ELSE series.publishers
            END,
            start_year = CASE
                WHEN (series.locked_fields->>'start_year')::boolean IS TRUE THEN series.start_year
                ELSE COALESCE(EXCLUDED.start_year, series.start_year)
            END,
            total_volumes = CASE
                WHEN (series.locked_fields->>'total_volumes')::boolean IS TRUE THEN series.total_volumes
                ELSE COALESCE(EXCLUDED.total_volumes, series.total_volumes)
            END,
            status = CASE
                WHEN (series.locked_fields->>'status')::boolean IS TRUE THEN series.status
                ELSE COALESCE(EXCLUDED.status, series.status)
            END,
            authors = CASE
                WHEN (series.locked_fields->>'authors')::boolean IS TRUE THEN series.authors
                WHEN array_length(EXCLUDED.authors, 1) > 0 THEN EXCLUDED.authors
                ELSE series.authors
            END,
            genres = CASE
                WHEN array_length(series.genres, 1) > 0 THEN series.genres
                WHEN array_length(EXCLUDED.genres, 1) > 0 THEN EXCLUDED.genres
                ELSE series.genres
            END,
            cover_url = COALESCE(NULLIF(EXCLUDED.cover_url, ''), series.cover_url),
            updated_at = NOW()
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .bind(fields.description.as_deref())
    .bind(&fields.publishers)
    .bind(fields.start_year)
    .bind(fields.total_volumes)
    .bind(fields.status.as_deref())
    .bind(&fields.authors)
    .bind(&fields.genres)
    .bind(fields.cover_url.as_deref())
    .execute(pool)
    .await?;

    Ok(existing)
}

// ---------------------------------------------------------------------------
// Local book fetching & matching
// ---------------------------------------------------------------------------

/// Fetch local books for a series, ordered for matching.
pub(crate) async fn fetch_local_books(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
) -> Result<Vec<LocalBookForMatching>, sqlx::Error> {
    let rows: Vec<LocalBookRow> = sqlx::query_as(
        r#"
        SELECT b.id, b.volume, b.title, b.isbn, b.volume_type FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        WHERE b.library_id = $1
          AND COALESCE(s.name, 'unclassified') = $2
          AND b.volume_type IN ('regular', 'integral', 'oneshot')
        ORDER BY b.volume NULLS LAST,
                 REGEXP_REPLACE(LOWER(b.title), '[0-9].*$', ''),
                 COALESCE((REGEXP_MATCH(LOWER(b.title), '\d+'))[1]::int, 0),
                 b.title ASC
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_all(pool)
    .await?;

    let with_pos = rows
        .iter()
        .enumerate()
        .map(
            |(idx, (id, vol, title, isbn, volume_type))| LocalBookForMatching {
                id: *id,
                volume: vol.unwrap_or((idx + 1) as i32),
                title: title.clone(),
                isbn: isbn.clone(),
                volume_type: volume_type.clone(),
            },
        )
        .collect();

    Ok(with_pos)
}

/// Match external books to local books using volume number then title containment.
/// Returns matched pairs preserving the external book order.
pub(crate) fn match_books<'a>(
    ext_books: &'a [BookCandidate],
    local_books: &[LocalBookForMatching],
) -> Vec<MatchedBook<'a>> {
    match_books_with_policy(ext_books, local_books, false)
}

/// Match books for the new HTML providers. This is deliberately opt-in so
/// historical providers keep their existing regular/integral matching behavior.
pub(crate) fn match_books_for_new_provider<'a>(
    ext_books: &'a [BookCandidate],
    local_books: &[LocalBookForMatching],
) -> Vec<MatchedBook<'a>> {
    match_books_with_policy(ext_books, local_books, true)
}

fn normalize_isbn(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn match_books_with_policy<'a>(
    ext_books: &'a [BookCandidate],
    local_books: &[LocalBookForMatching],
    allow_oneshots: bool,
) -> Vec<MatchedBook<'a>> {
    let mut matched_ids: HashSet<Uuid> = HashSet::new();

    ext_books
        .iter()
        .enumerate()
        .map(|(ext_idx, book)| {
            let is_vol_zero = book.volume_number == Some(0);
            let is_oneshot = allow_oneshots
                && book
                    .metadata_json
                    .get("oneshot")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
            let ext_vol = book.volume_number.unwrap_or((ext_idx + 1) as i32);

            // Strategy 1: Match by volume number (skip vol 0 — T0 = HS in providers)
            let mut local_id: Option<Uuid> = if is_vol_zero || is_oneshot {
                None
            } else {
                local_books
                    .iter()
                    .find(|book| {
                        book.volume == ext_vol
                            && !matched_ids.contains(&book.id)
                            && (allow_oneshots || book.volume_type != "oneshot")
                    })
                    .map(|book| book.id)
            };

            // Strategy 2: Title containment (case-insensitive)
            if !is_vol_zero && !is_oneshot && local_id.is_none() {
                let ext_lower = book.title.to_lowercase();
                local_id = local_books
                    .iter()
                    .find(|local| {
                        if matched_ids.contains(&local.id)
                            || (!allow_oneshots && local.volume_type == "oneshot")
                        {
                            return false;
                        }
                        let local_lower = local.title.to_lowercase();
                        local_lower.contains(&ext_lower) || ext_lower.contains(&local_lower)
                    })
                    .map(|local| local.id);
            }

            // ISBN is additive: it is only considered when the historical
            // volume/title strategies did not find a book.
            if local_id.is_none() && allow_oneshots {
                if let Some(ext_isbn) = book.isbn.as_deref().map(normalize_isbn) {
                    local_id = local_books
                        .iter()
                        .find(|local| {
                            !matched_ids.contains(&local.id)
                                && local.isbn.as_deref().map(normalize_isbn).as_deref()
                                    == Some(ext_isbn.as_str())
                                && (is_oneshot == (local.volume_type == "oneshot"))
                        })
                        .map(|local| local.id);
                }
            }

            if let Some(id) = local_id {
                matched_ids.insert(id);
            }

            MatchedBook {
                ext_book: book,
                local_book_id: local_id,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// External book metadata insert
// ---------------------------------------------------------------------------

/// Insert a row into external_book_metadata.
pub(crate) async fn insert_external_book_metadata(
    pool: &PgPool,
    link_id: Uuid,
    local_book_id: Option<Uuid>,
    book: &BookCandidate,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO external_book_metadata
            (link_id, book_id, external_book_id, volume_number, title, authors, isbn, summary, cover_url, page_count, language, publish_date, metadata_json)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(link_id)
    .bind(local_book_id)
    .bind(&book.external_book_id)
    .bind(book.volume_number)
    .bind(&book.title)
    .bind(&book.authors)
    .bind(&book.isbn)
    .bind(&book.summary)
    .bind(&book.cover_url)
    .bind(book.page_count)
    .bind(&book.language)
    .bind(&book.publish_date)
    .bind(&book.metadata_json)
    .execute(pool)
    .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Push metadata to local book
// ---------------------------------------------------------------------------

/// Push external metadata to a matched local book, respecting locked_fields.
/// Returns the pre-update row for diff/reporting.
pub(crate) async fn push_book_metadata(
    pool: &PgPool,
    book_id: Uuid,
    ext_book: &BookCandidate,
) -> Result<ExistingBookRow, sqlx::Error> {
    let current = sqlx::query(
        "SELECT title, summary, isbn, publish_date, language, authors, locked_fields FROM books WHERE id = $1",
    )
    .bind(book_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE books SET
            summary = CASE
                WHEN (locked_fields->>'summary')::boolean IS TRUE THEN summary
                ELSE COALESCE(NULLIF($2, ''), summary)
            END,
            isbn = CASE
                WHEN (locked_fields->>'isbn')::boolean IS TRUE THEN isbn
                ELSE COALESCE(NULLIF($3, ''), isbn)
            END,
            publish_date = CASE
                WHEN (locked_fields->>'publish_date')::boolean IS TRUE THEN publish_date
                ELSE COALESCE(NULLIF($4, ''), publish_date)
            END,
            language = CASE
                WHEN (locked_fields->>'language')::boolean IS TRUE THEN language
                ELSE COALESCE(NULLIF($5, ''), language)
            END,
            authors = CASE
                WHEN (locked_fields->>'authors')::boolean IS TRUE THEN authors
                WHEN CARDINALITY($6::text[]) > 0 THEN $6
                ELSE authors
            END,
            author = CASE
                WHEN (locked_fields->>'authors')::boolean IS TRUE THEN author
                WHEN CARDINALITY($6::text[]) > 0 THEN $6[1]
                ELSE author
            END,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(book_id)
    .bind(&ext_book.summary)
    .bind(&ext_book.isbn)
    .bind(&ext_book.publish_date)
    .bind(&ext_book.language)
    .bind(&ext_book.authors)
    .execute(pool)
    .await?;

    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn external(
        title: &str,
        volume: Option<i32>,
        isbn: Option<&str>,
        oneshot: bool,
    ) -> BookCandidate {
        BookCandidate {
            external_book_id: title.to_string(),
            title: title.to_string(),
            volume_number: volume,
            authors: vec![],
            isbn: isbn.map(str::to_string),
            summary: None,
            cover_url: None,
            page_count: None,
            language: None,
            publish_date: None,
            metadata_json: serde_json::json!({"oneshot": oneshot}),
        }
    }

    fn local(
        id: Uuid,
        volume: i32,
        title: &str,
        isbn: Option<&str>,
        volume_type: &str,
    ) -> LocalBookForMatching {
        LocalBookForMatching {
            id,
            volume,
            title: title.to_string(),
            isbn: isbn.map(str::to_string),
            volume_type: volume_type.to_string(),
        }
    }

    #[test]
    fn historical_matching_does_not_move_existing_volume_match_to_isbn_match() {
        let existing = Uuid::new_v4();
        let isbn_match = Uuid::new_v4();
        let ext = external("Different title", Some(1), Some("978-2-0000-0000-0"), false);
        let books = vec![
            local(existing, 1, "Existing tome", None, "regular"),
            local(isbn_match, 9, "ISBN tome", Some("9782000000000"), "regular"),
        ];
        let external_books = [ext];
        let matched = match_books(&external_books, &books);
        assert_eq!(matched[0].local_book_id, Some(existing));
    }

    #[test]
    fn new_provider_uses_isbn_when_volume_and_title_do_not_match() {
        let expected = Uuid::new_v4();
        let ext = external("Different title", Some(8), Some("978-2-0000-0000-0"), false);
        let books = vec![local(
            expected,
            9,
            "ISBN tome",
            Some("9782000000000"),
            "regular",
        )];
        let external_books = [ext];
        let matched = match_books_for_new_provider(&external_books, &books);
        assert_eq!(matched[0].local_book_id, Some(expected));
    }

    #[test]
    fn one_shot_requires_one_shot_local_book() {
        let oneshot = Uuid::new_v4();
        let regular = Uuid::new_v4();
        let ext = external("Album unique", None, Some("978-2-0000-0000-0"), true);
        let books = vec![
            local(regular, 1, "Album unique", Some("9782000000000"), "regular"),
            local(oneshot, 1, "Album unique", Some("9782000000000"), "oneshot"),
        ];
        let external_books = [ext];
        let matched = match_books_for_new_provider(&external_books, &books);
        assert_eq!(matched[0].local_book_id, Some(oneshot));
    }
}

// ---------------------------------------------------------------------------
// Delete link book metadata
// ---------------------------------------------------------------------------

/// Delete all external_book_metadata rows for a given link.
pub(crate) async fn delete_link_book_metadata(
    pool: &PgPool,
    link_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM external_book_metadata WHERE link_id = $1")
        .bind(link_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Update synced_at on a link
// ---------------------------------------------------------------------------

pub(crate) async fn update_link_synced_at(pool: &PgPool, link_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE external_metadata_links SET synced_at = NOW(), updated_at = NOW() WHERE id = $1",
    )
    .bind(link_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Diff helpers (used by sync.rs and refresh_sync.rs for reporting)
// ---------------------------------------------------------------------------

/// Compare old/new for a nullable string field. Returns Some only if changed.
pub(crate) fn diff_opt_str(
    old: Option<&str>,
    new: Option<&str>,
) -> Option<(Option<serde_json::Value>, Option<serde_json::Value>)> {
    let new_val = new.filter(|s| !s.is_empty());
    match (old, new_val) {
        (Some(o), Some(n)) if o != n => Some((
            Some(serde_json::Value::String(o.to_string())),
            Some(serde_json::Value::String(n.to_string())),
        )),
        (None, Some(n)) => Some((None, Some(serde_json::Value::String(n.to_string())))),
        _ => None,
    }
}

/// Compare old/new for an optional i32 field.
pub(crate) fn diff_opt_i32(
    old: Option<i32>,
    new: Option<i32>,
) -> Option<(Option<serde_json::Value>, Option<serde_json::Value>)> {
    match (old, new) {
        (Some(o), Some(n)) if o != n => {
            Some((Some(serde_json::json!(o)), Some(serde_json::json!(n))))
        }
        (None, Some(n)) => Some((None, Some(serde_json::json!(n)))),
        _ => None,
    }
}

/// Compare old/new for a string vector field.
pub(crate) fn diff_str_vec(
    old: &[String],
    new: &[String],
) -> Option<(Option<serde_json::Value>, Option<serde_json::Value>)> {
    if new.is_empty() {
        return None;
    }
    if old != new {
        Some((Some(serde_json::json!(old)), Some(serde_json::json!(new))))
    } else {
        None
    }
}

/// Check if a field is locked in the locked_fields JSON.
pub(crate) fn is_field_locked(locked: &serde_json::Value, field: &str) -> bool {
    locked.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
}
