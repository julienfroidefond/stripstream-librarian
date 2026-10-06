use std::collections::{HashMap, HashSet};

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::metadata_providers::BookCandidate;

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
// Multi-provider merge (primary + secondary fallback)
// ---------------------------------------------------------------------------

/// One approved metadata link, used as input to the multi-provider merge.
#[derive(Debug, Clone)]
pub(crate) struct LinkMetadata {
    pub id: Uuid,
    pub is_primary: bool,
    pub provider: String,
    pub external_id: String,
    pub metadata_json: serde_json::Value,
    pub total_volumes_external: Option<i32>,
}

fn json_non_empty_str(json: &serde_json::Value, key: &str) -> Option<String> {
    json.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Fetch the approved links of a series, ordered primary-first then by approval
/// time (deterministic secondary order).
pub(crate) async fn fetch_approved_links(
    pool: &PgPool,
    series_id: Uuid,
) -> Result<Vec<LinkMetadata>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT id, is_primary, provider, external_id, metadata_json, total_volumes_external
        FROM external_metadata_links
        WHERE series_id = $1 AND status = 'approved'
        ORDER BY is_primary DESC, approved_at ASC NULLS LAST, id ASC
        "#,
    )
    .bind(series_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| LinkMetadata {
            id: r.get("id"),
            is_primary: r.get("is_primary"),
            provider: r.get("provider"),
            external_id: r.get("external_id"),
            metadata_json: r.get("metadata_json"),
            total_volumes_external: r.get("total_volumes_external"),
        })
        .collect())
}

/// Merge series fields across approved links: primary first, then secondaries,
/// taking the first non-empty value for each field. Genres are taken from the
/// primary (the app / AI tagging remains the source of truth).
///
/// `status` is returned raw; the caller normalizes it (needs a DB pool).
pub(crate) fn merge_series_fields(links: &[LinkMetadata]) -> SeriesFields {
    let mut ordered: Vec<&LinkMetadata> = links.iter().collect();
    ordered.sort_by_key(|l| !l.is_primary); // primary (true) first; stable

    let description = ordered
        .iter()
        .find_map(|l| json_non_empty_str(&l.metadata_json, "description"));
    let authors = ordered
        .iter()
        .find_map(|l| extract_string_array(&l.metadata_json, "authors"))
        .unwrap_or_default();
    let publishers = ordered
        .iter()
        .find_map(|l| extract_string_array(&l.metadata_json, "publishers"))
        .unwrap_or_default();
    let start_year = ordered.iter().find_map(|l| {
        l.metadata_json
            .get("start_year")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
    });
    let total_volumes = ordered.iter().find_map(|l| l.total_volumes_external);
    let status = ordered
        .iter()
        .find_map(|l| json_non_empty_str(&l.metadata_json, "status"));
    // Genres do not participate in the priority merge: only the primary link
    // seeds them (the app / AI tagging remains the source of truth).
    let genres = ordered
        .first()
        .and_then(|l| extract_string_array(&l.metadata_json, "genres"))
        .unwrap_or_default();
    let cover_url = ordered
        .iter()
        .find_map(|l| json_non_empty_str(&l.metadata_json, "cover_url"));

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
// Multi-provider book merge (primary + secondary fallback)
// ---------------------------------------------------------------------------

/// One `external_book_metadata` row (matched to a local book), used as input to
/// the cross-provider book merge.
#[derive(Debug, Clone)]
pub(crate) struct ExternalBookRow {
    pub book_id: Uuid,
    pub external_book_id: Option<String>,
    pub volume_number: Option<i32>,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub isbn: Option<String>,
    pub summary: Option<String>,
    pub cover_url: Option<String>,
    pub page_count: Option<i32>,
    pub language: Option<String>,
    pub publish_date: Option<String>,
    pub metadata_json: serde_json::Value,
}

fn row_to_external_book(row: &sqlx::postgres::PgRow) -> ExternalBookRow {
    ExternalBookRow {
        book_id: row.get("book_id"),
        external_book_id: row.get("external_book_id"),
        volume_number: row.get("volume_number"),
        title: row.get("title"),
        authors: row.get("authors"),
        isbn: row.get("isbn"),
        summary: row.get("summary"),
        cover_url: row.get("cover_url"),
        page_count: row.get("page_count"),
        language: row.get("language"),
        publish_date: row.get("publish_date"),
        metadata_json: row.get("metadata_json"),
    }
}

/// Fetch the matched external books of a series' approved links, ordered
/// primary-first then by approval time. Rows without a local `book_id` are
/// skipped: they cannot be merged onto a local book.
pub(crate) async fn fetch_approved_external_books(
    pool: &PgPool,
    series_id: Uuid,
) -> Result<Vec<ExternalBookRow>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT ebm.book_id, ebm.external_book_id, ebm.volume_number, ebm.title,
               ebm.authors, ebm.isbn, ebm.summary, ebm.cover_url, ebm.page_count,
               ebm.language, ebm.publish_date, ebm.metadata_json
        FROM external_book_metadata ebm
        JOIN external_metadata_links eml ON eml.id = ebm.link_id
        WHERE eml.series_id = $1
          AND eml.status = 'approved'
          AND ebm.book_id IS NOT NULL
        ORDER BY eml.is_primary DESC, eml.approved_at ASC NULLS LAST, eml.id ASC,
                 ebm.volume_number NULLS LAST, ebm.id ASC
        "#,
    )
    .bind(series_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(row_to_external_book).collect())
}

/// Merge external book rows per local book: the primary link's row wins, and
/// each missing field is filled from the first secondary that has it. Returns
/// one merged `BookCandidate` per local book, in first-seen order.
pub(crate) fn merge_book_rows(rows: &[ExternalBookRow]) -> Vec<(Uuid, BookCandidate)> {
    let mut merged: Vec<(Uuid, BookCandidate)> = Vec::new();
    let mut index: HashMap<Uuid, usize> = HashMap::new();

    for row in rows {
        let candidate = BookCandidate {
            external_book_id: row.external_book_id.clone().unwrap_or_default(),
            title: row.title.clone().unwrap_or_default(),
            volume_number: row.volume_number,
            authors: row.authors.clone(),
            isbn: row.isbn.clone(),
            summary: row.summary.clone(),
            cover_url: row.cover_url.clone(),
            page_count: row.page_count,
            language: row.language.clone(),
            publish_date: row.publish_date.clone(),
            metadata_json: row.metadata_json.clone(),
        };

        match index.get(&row.book_id) {
            Some(&i) => merge_missing_book_fields(&mut merged[i].1, &candidate),
            None => {
                index.insert(row.book_id, merged.len());
                merged.push((row.book_id, candidate));
            }
        }
    }

    merged
}

fn merge_missing_book_fields(base: &mut BookCandidate, other: &BookCandidate) {
    if base.summary.is_none() {
        base.summary = other.summary.clone();
    }
    if base.isbn.is_none() {
        base.isbn = other.isbn.clone();
    }
    if base.publish_date.is_none() {
        base.publish_date = other.publish_date.clone();
    }
    if base.language.is_none() {
        base.language = other.language.clone();
    }
    if base.cover_url.is_none() {
        base.cover_url = other.cover_url.clone();
    }
    if base.page_count.is_none() {
        base.page_count = other.page_count;
    }
    if base.authors.is_empty() {
        base.authors = other.authors.clone();
    }
    if base.external_book_id.is_empty() {
        base.external_book_id = other.external_book_id.clone();
    }
    if base.title.is_empty() {
        base.title = other.title.clone();
    }
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
// Primary link management
// ---------------------------------------------------------------------------

/// Promote `link_id` to primary if the series currently has no approved primary
/// link. Used when approving/creating a link. Returns true if it was promoted.
pub(crate) async fn promote_if_no_primary(
    pool: &PgPool,
    series_id: Uuid,
    link_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE external_metadata_links
        SET is_primary = true, updated_at = NOW()
        WHERE id = $2
          AND status = 'approved'
          AND NOT EXISTS (
              SELECT 1 FROM external_metadata_links e2
              WHERE e2.series_id = $1
                AND e2.status = 'approved'
                AND e2.is_primary
          )
        "#,
    )
    .bind(series_id)
    .bind(link_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Promote the oldest approved link of a series to primary if none is primary.
/// Used after rejecting/deleting the current primary. Returns the promoted id.
pub(crate) async fn promote_oldest_approved(
    pool: &PgPool,
    series_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let promoted: Option<Uuid> = sqlx::query_scalar(
        r#"
        UPDATE external_metadata_links
        SET is_primary = true, updated_at = NOW()
        WHERE id = (
            SELECT e2.id FROM external_metadata_links e2
            WHERE e2.series_id = $1
              AND e2.status = 'approved'
              AND NOT EXISTS (
                  SELECT 1 FROM external_metadata_links e3
                  WHERE e3.series_id = $1
                    AND e3.status = 'approved'
                    AND e3.is_primary
              )
            ORDER BY e2.approved_at ASC NULLS LAST, e2.id ASC
            LIMIT 1
        )
        RETURNING id
        "#,
    )
    .bind(series_id)
    .fetch_optional(pool)
    .await?;
    Ok(promoted)
}

/// Set `link_id` as the primary link of its series, clearing any other primary.
/// The caller must have verified the link is approved.
pub(crate) async fn set_primary_link(
    pool: &PgPool,
    series_id: Uuid,
    link_id: Uuid,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE external_metadata_links SET is_primary = false, updated_at = NOW()
         WHERE series_id = $1 AND is_primary",
    )
    .bind(series_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE external_metadata_links SET is_primary = true, updated_at = NOW()
         WHERE id = $1 AND series_id = $2 AND status = 'approved'",
    )
    .bind(link_id)
    .bind(series_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
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

    fn link(provider: &str, is_primary: bool, json: serde_json::Value) -> LinkMetadata {
        LinkMetadata {
            id: Uuid::new_v4(),
            is_primary,
            provider: provider.to_string(),
            external_id: "ext".to_string(),
            metadata_json: json,
            total_volumes_external: None,
        }
    }

    #[test]
    fn merge_series_fields_primary_wins_and_secondary_fills() {
        let primary = link(
            "google_books",
            true,
            serde_json::json!({
                "description": "from primary",
                "authors": ["A"],
                "start_year": 1999,
            }),
        );
        let secondary = link(
            "bdtheque",
            false,
            serde_json::json!({
                "description": "from secondary",
                "publishers": ["P"],
                "start_year": 2001,
                "cover_url": "http://cover",
            }),
        );
        // Deliberately pass secondary first: the merge must reorder by primary.
        let merged = merge_series_fields(&[secondary, primary]);
        assert_eq!(merged.description.as_deref(), Some("from primary"));
        assert_eq!(merged.authors, vec!["A".to_string()]);
        assert_eq!(merged.publishers, vec!["P".to_string()]);
        assert_eq!(merged.start_year, Some(1999));
        assert_eq!(merged.cover_url.as_deref(), Some("http://cover"));
    }

    #[test]
    fn merge_series_fields_genres_only_from_primary() {
        let primary = link(
            "google_books",
            true,
            serde_json::json!({"genres": ["Action"]}),
        );
        let secondary = link("bdtheque", false, serde_json::json!({"genres": ["Drame"]}));
        let merged = merge_series_fields(&[secondary, primary]);
        assert_eq!(merged.genres, vec!["Action".to_string()]);
    }

    #[test]
    fn merge_series_fields_genres_not_filled_from_secondary() {
        let primary = link("google_books", true, serde_json::json!({}));
        let secondary = link("bdtheque", false, serde_json::json!({"genres": ["Drame"]}));
        let merged = merge_series_fields(&[secondary, primary]);
        assert!(merged.genres.is_empty());
    }

    fn ext_row(
        book_id: Uuid,
        summary: Option<&str>,
        isbn: Option<&str>,
        authors: Vec<&str>,
    ) -> ExternalBookRow {
        ExternalBookRow {
            book_id,
            external_book_id: Some("ext".to_string()),
            volume_number: Some(1),
            title: Some("T".to_string()),
            authors: authors.into_iter().map(String::from).collect(),
            isbn: isbn.map(String::from),
            summary: summary.map(String::from),
            cover_url: None,
            page_count: None,
            language: None,
            publish_date: None,
            metadata_json: serde_json::json!({}),
        }
    }

    #[test]
    fn merge_book_rows_primary_wins_and_secondary_fills() {
        let book = Uuid::new_v4();
        let rows = vec![
            ext_row(book, Some("primary summary"), None, vec!["A"]),
            ext_row(book, Some("secondary summary"), Some("123"), vec!["B"]),
        ];
        let merged = merge_book_rows(&rows);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].0, book);
        assert_eq!(merged[0].1.summary.as_deref(), Some("primary summary"));
        assert_eq!(merged[0].1.isbn.as_deref(), Some("123"));
        assert_eq!(merged[0].1.authors, vec!["A".to_string()]);
    }

    #[test]
    fn merge_book_rows_keeps_distinct_books_separate() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let rows = vec![
            ext_row(a, Some("s1"), None, vec![]),
            ext_row(b, Some("s2"), None, vec![]),
        ];
        let merged = merge_book_rows(&rows);
        assert_eq!(merged.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Primary link management (DB)
    // -----------------------------------------------------------------------

    async fn seed_series(pool: &sqlx::PgPool) -> (Uuid, Uuid) {
        let lib_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'test', '/libraries/test')",
        )
        .bind(lib_id)
        .execute(pool)
        .await
        .unwrap();

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Test')")
            .bind(series_id)
            .bind(lib_id)
            .execute(pool)
            .await
            .unwrap();

        (lib_id, series_id)
    }

    async fn seed_link(
        pool: &sqlx::PgPool,
        lib_id: Uuid,
        series_id: Uuid,
        provider: &str,
        status: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO external_metadata_links (id, library_id, series_id, provider, external_id, status) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(lib_id)
        .bind(series_id)
        .bind(provider)
        .bind(format!("ext:{provider}"))
        .bind(status)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn promote_if_no_primary_only_first_wins(pool: sqlx::PgPool) {
        let (lib_id, series_id) = seed_series(&pool).await;
        let a = seed_link(&pool, lib_id, series_id, "google_books", "approved").await;
        let b = seed_link(&pool, lib_id, series_id, "bdtheque", "approved").await;

        assert!(promote_if_no_primary(&pool, series_id, a).await.unwrap());
        assert!(!promote_if_no_primary(&pool, series_id, b).await.unwrap());

        let primary: Uuid = sqlx::query_scalar(
            "SELECT id FROM external_metadata_links WHERE series_id = $1 AND is_primary AND status = 'approved'",
        )
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(primary, a);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn set_primary_link_switches_and_clears_previous(pool: sqlx::PgPool) {
        let (lib_id, series_id) = seed_series(&pool).await;
        let a = seed_link(&pool, lib_id, series_id, "google_books", "approved").await;
        let b = seed_link(&pool, lib_id, series_id, "bdtheque", "approved").await;

        set_primary_link(&pool, series_id, a).await.unwrap();
        set_primary_link(&pool, series_id, b).await.unwrap();

        let primaries: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM external_metadata_links WHERE series_id = $1 AND is_primary",
        )
        .bind(series_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(primaries, vec![b]);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn unique_index_blocks_two_approved_primaries(pool: sqlx::PgPool) {
        let (lib_id, series_id) = seed_series(&pool).await;
        let a = seed_link(&pool, lib_id, series_id, "google_books", "approved").await;
        let b = seed_link(&pool, lib_id, series_id, "bdtheque", "approved").await;
        set_primary_link(&pool, series_id, a).await.unwrap();

        let err = sqlx::query("UPDATE external_metadata_links SET is_primary = true WHERE id = $1")
            .bind(b)
            .execute(&pool)
            .await;
        assert!(
            err.is_err(),
            "two approved primaries must violate the unique index"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn promote_oldest_approved_after_reject(pool: sqlx::PgPool) {
        let (lib_id, series_id) = seed_series(&pool).await;
        let a = seed_link(&pool, lib_id, series_id, "google_books", "approved").await;
        let b = seed_link(&pool, lib_id, series_id, "bdtheque", "approved").await;
        set_primary_link(&pool, series_id, a).await.unwrap();

        // Reject the primary (mirrors reject_metadata).
        sqlx::query(
            "UPDATE external_metadata_links SET status = 'rejected', is_primary = false WHERE id = $1",
        )
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();

        let promoted = promote_oldest_approved(&pool, series_id).await.unwrap();
        assert_eq!(promoted, Some(b));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn promote_oldest_approved_noop_when_primary_exists(pool: sqlx::PgPool) {
        let (lib_id, series_id) = seed_series(&pool).await;
        let a = seed_link(&pool, lib_id, series_id, "google_books", "approved").await;
        let _b = seed_link(&pool, lib_id, series_id, "bdtheque", "approved").await;
        set_primary_link(&pool, series_id, a).await.unwrap();

        let promoted = promote_oldest_approved(&pool, series_id).await.unwrap();
        assert_eq!(promoted, None);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn fetch_approved_links_orders_primary_first(pool: sqlx::PgPool) {
        let (lib_id, series_id) = seed_series(&pool).await;
        let a = seed_link(&pool, lib_id, series_id, "google_books", "approved").await;
        let b = seed_link(&pool, lib_id, series_id, "bdtheque", "approved").await;
        set_primary_link(&pool, series_id, b).await.unwrap();

        let links = fetch_approved_links(&pool, series_id).await.unwrap();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].id, b);
        assert!(links[0].is_primary);
        assert_eq!(links[1].id, a);
        assert!(!links[1].is_primary);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn merge_across_stored_links_primary_wins(pool: sqlx::PgPool) {
        let (lib_id, series_id) = seed_series(&pool).await;
        let primary = seed_link(&pool, lib_id, series_id, "google_books", "approved").await;
        let secondary = seed_link(&pool, lib_id, series_id, "bdtheque", "approved").await;
        set_primary_link(&pool, series_id, primary).await.unwrap();

        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, format, series_id) \
             VALUES ($1, $2, 'Vol 1', 'comic', 'cbz', $3)",
        )
        .bind(book_id)
        .bind(lib_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        // Primary provides the summary; secondary provides the ISBN.
        sqlx::query(
            "INSERT INTO external_book_metadata (id, link_id, external_book_id, title, volume_number, book_id, summary) \
             VALUES (gen_random_uuid(), $1, 'p1', 'Vol 1', 1, $2, 'primary summary')",
        )
        .bind(primary)
        .bind(book_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO external_book_metadata (id, link_id, external_book_id, title, volume_number, book_id, isbn) \
             VALUES (gen_random_uuid(), $1, 's1', 'Vol 1', 1, $2, '9782000000000')",
        )
        .bind(secondary)
        .bind(book_id)
        .execute(&pool)
        .await
        .unwrap();

        let rows = fetch_approved_external_books(&pool, series_id)
            .await
            .unwrap();
        let merged = merge_book_rows(&rows);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].0, book_id);
        assert_eq!(merged[0].1.summary.as_deref(), Some("primary summary"));
        assert_eq!(merged[0].1.isbn.as_deref(), Some("9782000000000"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn sync_series_from_links_respects_locked_fields(pool: sqlx::PgPool) {
        let (lib_id, series_id) = seed_series(&pool).await;
        sqlx::query(
            "UPDATE series SET description = 'kept', locked_fields = '{\"description\": true}'::jsonb \
             WHERE id = $1",
        )
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        let primary = seed_link(&pool, lib_id, series_id, "google_books", "approved").await;
        sqlx::query(
            "UPDATE external_metadata_links \
             SET is_primary = true, metadata_json = '{\"description\": \"from provider\"}'::jsonb \
             WHERE id = $1",
        )
        .bind(primary)
        .execute(&pool)
        .await
        .unwrap();

        crate::metadata::sync::sync_series_from_links(&pool, series_id, true, false)
            .await
            .unwrap();

        let description: Option<String> =
            sqlx::query_scalar("SELECT description FROM series WHERE id = $1")
                .bind(series_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(description.as_deref(), Some("kept"));
    }
}
