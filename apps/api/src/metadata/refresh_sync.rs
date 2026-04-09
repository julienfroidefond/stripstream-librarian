use serde::Serialize;
use sqlx::{PgPool, Row};
use tracing::info;
use uuid::Uuid;

use crate::metadata_providers;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single field change: old -> new
#[derive(Serialize, Clone)]
pub(crate) struct FieldDiff {
    pub(crate) field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) old: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) new: Option<serde_json::Value>,
}

/// Per-book changes
#[derive(Serialize, Clone)]
pub(crate) struct BookDiff {
    pub(crate) book_id: String,
    pub(crate) title: String,
    pub(crate) volume: Option<i32>,
    pub(crate) changes: Vec<FieldDiff>,
}

// ---------------------------------------------------------------------------
// Refresh a single link
// ---------------------------------------------------------------------------

/// Refresh a single approved metadata link: re-fetch from provider, compare, sync, return diff
pub(crate) async fn refresh_link(
    pool: &PgPool,
    link_id: Uuid,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    external_id: &str,
) -> Result<super::refresh::SeriesRefreshResult, String> {
    let provider = metadata_providers::get_provider(provider_name)
        .ok_or_else(|| format!("Unknown provider: {provider_name}"))?;

    let config = super::config::load_provider_config(pool, provider_name).await;

    let mut series_changes: Vec<FieldDiff> = Vec::new();
    let mut book_changes: Vec<BookDiff> = Vec::new();

    // -- Series-level refresh --
    let candidates = provider
        .search_series(series_name, &config)
        .await
        .map_err(|e| format!("provider search error: {e}"))?;

    let candidate = candidates
        .iter()
        .find(|c| c.external_id == external_id)
        .or_else(|| candidates.first());

    if let Some(candidate) = candidate {
        // Update link metadata_json
        sqlx::query(
            r#"
            UPDATE external_metadata_links
            SET metadata_json = $2,
                total_volumes_external = $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(link_id)
        .bind(&candidate.metadata_json)
        .bind(candidate.total_volumes)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        // Diff + sync series metadata
        series_changes = sync_series_with_diff(pool, library_id, series_name, candidate).await?;
    }

    // -- Book-level refresh --
    let books = provider
        .get_series_books(external_id, &config)
        .await
        .map_err(|e| format!("provider books error: {e}"))?;

    // Delete existing external_book_metadata for this link
    sqlx::query("DELETE FROM external_book_metadata WHERE link_id = $1")
        .bind(link_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Pre-fetch local books
    let local_books: Vec<(Uuid, Option<i32>, String)> = sqlx::query_as(
        r#"
        SELECT b.id, b.volume, b.title FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        WHERE b.library_id = $1
          AND COALESCE(s.name, 'unclassified') = $2
          AND b.volume_type = 'regular'
        ORDER BY b.volume NULLS LAST,
                 REGEXP_REPLACE(LOWER(b.title), '[0-9].*$', ''),
                 COALESCE((REGEXP_MATCH(LOWER(b.title), '\d+'))[1]::int, 0),
                 b.title ASC
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let local_books_with_pos: Vec<(Uuid, i32, String)> = local_books
        .iter()
        .enumerate()
        .map(|(idx, (id, vol, title))| (*id, vol.unwrap_or((idx + 1) as i32), title.clone()))
        .collect();

    let mut matched_local_ids = std::collections::HashSet::new();

    for (ext_idx, book) in books.iter().enumerate() {
        let ext_vol = book.volume_number.unwrap_or((ext_idx + 1) as i32);

        // Match by volume number
        let mut local_book_id: Option<Uuid> = local_books_with_pos
            .iter()
            .find(|(id, v, _)| *v == ext_vol && !matched_local_ids.contains(id))
            .map(|(id, _, _)| *id);

        // Match by title containment
        if local_book_id.is_none() {
            let ext_title_lower = book.title.to_lowercase();
            local_book_id = local_books_with_pos
                .iter()
                .find(|(id, _, local_title)| {
                    if matched_local_ids.contains(id) {
                        return false;
                    }
                    let local_lower = local_title.to_lowercase();
                    local_lower.contains(&ext_title_lower) || ext_title_lower.contains(&local_lower)
                })
                .map(|(id, _, _)| *id);
        }

        if let Some(id) = local_book_id {
            matched_local_ids.insert(id);
        }

        // Insert external_book_metadata
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
        .await
        .map_err(|e| e.to_string())?;

        // Diff + push metadata to matched local book
        if let Some(book_id) = local_book_id {
            let diffs = sync_book_with_diff(pool, book_id, book).await?;
            if !diffs.is_empty() {
                let local_title = local_books_with_pos
                    .iter()
                    .find(|(id, _, _)| *id == book_id)
                    .map(|(_, _, t)| t.clone())
                    .unwrap_or_default();
                book_changes.push(BookDiff {
                    book_id: book_id.to_string(),
                    title: local_title,
                    volume: book.volume_number,
                    changes: diffs,
                });
            }
        }
    }

    // Update synced_at on the link
    sqlx::query("UPDATE external_metadata_links SET synced_at = NOW(), updated_at = NOW() WHERE id = $1")
        .bind(link_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Re-match any external books that couldn't be matched during insert
    // (e.g., metadata was fetched before books were imported)
    let _ = rematch_unlinked_books(pool, library_id).await;

    let has_changes = !series_changes.is_empty() || !book_changes.is_empty();

    Ok(super::refresh::SeriesRefreshResult {
        series_name: series_name.to_string(),
        provider: provider_name.to_string(),
        status: if has_changes { "updated".to_string() } else { "unchanged".to_string() },
        series_changes,
        book_changes,
        error: None,
    })
}

// ---------------------------------------------------------------------------
// Diff helpers
// ---------------------------------------------------------------------------

/// Compare old/new for a nullable string field. Returns Some(FieldDiff) only if value actually changed.
fn diff_opt_str(field: &str, old: Option<&str>, new: Option<&str>) -> Option<FieldDiff> {
    let new_val = new.filter(|s| !s.is_empty());
    // Only report a change if there is a new non-empty value AND it differs from old
    match (old, new_val) {
        (Some(o), Some(n)) if o != n => Some(FieldDiff {
            field: field.to_string(),
            old: Some(serde_json::Value::String(o.to_string())),
            new: Some(serde_json::Value::String(n.to_string())),
        }),
        (None, Some(n)) => Some(FieldDiff {
            field: field.to_string(),
            old: None,
            new: Some(serde_json::Value::String(n.to_string())),
        }),
        _ => None,
    }
}

fn diff_opt_i32(field: &str, old: Option<i32>, new: Option<i32>) -> Option<FieldDiff> {
    match (old, new) {
        (Some(o), Some(n)) if o != n => Some(FieldDiff {
            field: field.to_string(),
            old: Some(serde_json::json!(o)),
            new: Some(serde_json::json!(n)),
        }),
        (None, Some(n)) => Some(FieldDiff {
            field: field.to_string(),
            old: None,
            new: Some(serde_json::json!(n)),
        }),
        _ => None,
    }
}

fn diff_str_vec(field: &str, old: &[String], new: &[String]) -> Option<FieldDiff> {
    if new.is_empty() {
        return None;
    }
    if old != new {
        Some(FieldDiff {
            field: field.to_string(),
            old: Some(serde_json::json!(old)),
            new: Some(serde_json::json!(new)),
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Series sync with diff tracking
// ---------------------------------------------------------------------------

async fn sync_series_with_diff(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    candidate: &metadata_providers::SeriesCandidate,
) -> Result<Vec<FieldDiff>, String> {
    let new_description = candidate.metadata_json
        .get("description")
        .and_then(|d| d.as_str())
        .or(candidate.description.as_deref());
    let new_authors = &candidate.authors;
    let new_publishers = &candidate.publishers;
    let new_start_year = candidate.start_year;
    let new_total_volumes = candidate.total_volumes;
    let new_status = if let Some(raw) = candidate.metadata_json.get("status").and_then(|s| s.as_str()) {
        Some(super::handlers::normalize_series_status(pool, raw).await)
    } else {
        None
    };
    let new_status = new_status.as_deref();

    // Fetch existing series metadata for diffing
    let existing = sqlx::query(
        r#"SELECT description, publishers, start_year, total_volumes, status, authors, locked_fields
           FROM series WHERE library_id = $1 AND name = $2"#,
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let locked = existing
        .as_ref()
        .map(|r| r.get::<serde_json::Value, _>("locked_fields"))
        .unwrap_or(serde_json::json!({}));
    let is_locked = |field: &str| -> bool {
        locked.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
    };

    // Build diffs (only for unlocked fields that actually change)
    let mut diffs: Vec<FieldDiff> = Vec::new();

    if !is_locked("description") {
        let old_desc: Option<String> = existing.as_ref().and_then(|r| r.get("description"));
        if let Some(d) = diff_opt_str("description", old_desc.as_deref(), new_description) {
            diffs.push(d);
        }
    }
    if !is_locked("authors") {
        let old_authors: Vec<String> = existing.as_ref().map(|r| r.get("authors")).unwrap_or_default();
        if let Some(d) = diff_str_vec("authors", &old_authors, new_authors) {
            diffs.push(d);
        }
    }
    if !is_locked("publishers") {
        let old_publishers: Vec<String> = existing.as_ref().map(|r| r.get("publishers")).unwrap_or_default();
        if let Some(d) = diff_str_vec("publishers", &old_publishers, new_publishers) {
            diffs.push(d);
        }
    }
    if !is_locked("start_year") {
        let old_year: Option<i32> = existing.as_ref().and_then(|r| r.get("start_year"));
        if let Some(d) = diff_opt_i32("start_year", old_year, new_start_year) {
            diffs.push(d);
        }
    }
    if !is_locked("total_volumes") {
        let old_vols: Option<i32> = existing.as_ref().and_then(|r| r.get("total_volumes"));
        if let Some(d) = diff_opt_i32("total_volumes", old_vols, new_total_volumes) {
            diffs.push(d);
        }
    }
    if !is_locked("status") {
        let old_status: Option<String> = existing.as_ref().and_then(|r| r.get("status"));
        if let Some(d) = diff_opt_str("status", old_status.as_deref(), new_status) {
            diffs.push(d);
        }
    }

    // Now do the actual upsert
    sqlx::query(
        r#"
        INSERT INTO series (id, library_id, name, description, publishers, start_year, total_volumes, status, authors, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
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
            updated_at = NOW()
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .bind(new_description)
    .bind(new_publishers)
    .bind(new_start_year)
    .bind(new_total_volumes)
    .bind(new_status)
    .bind(new_authors)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(diffs)
}

// ---------------------------------------------------------------------------
// Book sync with diff tracking
// ---------------------------------------------------------------------------

async fn sync_book_with_diff(
    pool: &PgPool,
    book_id: Uuid,
    ext_book: &metadata_providers::BookCandidate,
) -> Result<Vec<FieldDiff>, String> {
    // Fetch current book state
    let current = sqlx::query(
        "SELECT summary, isbn, publish_date, language, authors, locked_fields FROM books WHERE id = $1",
    )
    .bind(book_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let locked = current.get::<serde_json::Value, _>("locked_fields");
    let is_locked = |field: &str| -> bool {
        locked.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
    };

    // Build diffs
    let mut diffs: Vec<FieldDiff> = Vec::new();

    if !is_locked("summary") {
        let old: Option<String> = current.get("summary");
        if let Some(d) = diff_opt_str("summary", old.as_deref(), ext_book.summary.as_deref()) {
            diffs.push(d);
        }
    }
    if !is_locked("isbn") {
        let old: Option<String> = current.get("isbn");
        if let Some(d) = diff_opt_str("isbn", old.as_deref(), ext_book.isbn.as_deref()) {
            diffs.push(d);
        }
    }
    if !is_locked("publish_date") {
        let old: Option<String> = current.get("publish_date");
        if let Some(d) = diff_opt_str("publish_date", old.as_deref(), ext_book.publish_date.as_deref()) {
            diffs.push(d);
        }
    }
    if !is_locked("language") {
        let old: Option<String> = current.get("language");
        if let Some(d) = diff_opt_str("language", old.as_deref(), ext_book.language.as_deref()) {
            diffs.push(d);
        }
    }
    if !is_locked("authors") {
        let old: Vec<String> = current.get("authors");
        if let Some(d) = diff_str_vec("authors", &old, &ext_book.authors) {
            diffs.push(d);
        }
    }

    // Do the actual update
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
    .await
    .map_err(|e| e.to_string())?;

    Ok(diffs)
}

/// Re-match external_book_metadata rows that have book_id IS NULL
/// by joining on volume number with local books in the same series.
/// Called after scans/imports to fix metadata that was fetched before books existed.
pub async fn rematch_unlinked_books(pool: &PgPool, library_id: Uuid) -> Result<i64, String> {
    let result = sqlx::query(
        r#"
        UPDATE external_book_metadata ebm
        SET book_id = matched.book_id
        FROM (
            SELECT DISTINCT ON (ebm2.id)
                ebm2.id AS ebm_id,
                b.id AS book_id
            FROM external_book_metadata ebm2
            JOIN external_metadata_links eml ON eml.id = ebm2.link_id
            JOIN books b ON b.library_id = eml.library_id
                AND b.series_id = eml.series_id
                AND b.volume = ebm2.volume_number
                AND b.volume_type = 'regular'
            WHERE eml.library_id = $1
              AND ebm2.book_id IS NULL
              AND ebm2.volume_number IS NOT NULL
              AND eml.status = 'approved'
        ) matched
        WHERE ebm.id = matched.ebm_id
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let count = result.rows_affected() as i64;
    if count > 0 {
        info!("[METADATA] Re-matched {count} unlinked external books for library {library_id}");
    }
    Ok(count)
}
