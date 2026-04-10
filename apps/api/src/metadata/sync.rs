use sqlx::Row;
use uuid::Uuid;

use crate::{error::ApiError, metadata_providers, state::AppState};
use super::handlers::{FieldChange, SeriesSyncReport, BookSyncReport, normalize_series_status};

pub(crate) async fn sync_series_metadata(
    state: &AppState,
    library_id: Uuid,
    series_name: &str,
    metadata_json: &serde_json::Value,
    total_volumes: Option<i32>,
) -> Result<SeriesSyncReport, ApiError> {
    let description = metadata_json
        .get("description")
        .and_then(|d| d.as_str());
    let authors: Vec<String> = metadata_json
        .get("authors")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let publishers: Vec<String> = metadata_json
        .get("publishers")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let start_year = metadata_json
        .get("start_year")
        .and_then(|y| y.as_i64())
        .map(|y| y as i32);
    let genres: Vec<String> = metadata_json
        .get("genres")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let status = if let Some(raw) = metadata_json.get("status").and_then(|s| s.as_str()) {
        Some(normalize_series_status(&state.pool, raw).await)
    } else {
        None
    };
    let cover_url = metadata_json
        .get("cover_url")
        .and_then(|c| c.as_str())
        .filter(|c| !c.is_empty());

    // Fetch existing state (case/accent insensitive to match get_or_create_series behavior)
    let existing = sqlx::query(
        r#"SELECT description, publishers, start_year, total_volumes, status, authors, locked_fields
           FROM series WHERE library_id = $1 AND LOWER(unaccent(name)) = LOWER(unaccent($2))"#,
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(&state.pool)
    .await?;

    // Update existing series (case/accent insensitive match).
    // Respects locked_fields: only update fields that are NOT locked.
    sqlx::query(
        r#"
        UPDATE series SET
            description = CASE
                WHEN (locked_fields->>'description')::boolean IS TRUE THEN description
                ELSE COALESCE(NULLIF($3, ''), description)
            END,
            publishers = CASE
                WHEN (locked_fields->>'publishers')::boolean IS TRUE THEN publishers
                WHEN array_length($4::text[], 1) > 0 THEN $4
                ELSE publishers
            END,
            start_year = CASE
                WHEN (locked_fields->>'start_year')::boolean IS TRUE THEN start_year
                ELSE COALESCE($5, start_year)
            END,
            total_volumes = CASE
                WHEN (locked_fields->>'total_volumes')::boolean IS TRUE THEN total_volumes
                ELSE COALESCE($6, total_volumes)
            END,
            status = CASE
                WHEN (locked_fields->>'status')::boolean IS TRUE THEN status
                ELSE COALESCE($7, status)
            END,
            authors = CASE
                WHEN (locked_fields->>'authors')::boolean IS TRUE THEN authors
                WHEN array_length($8::text[], 1) > 0 THEN $8
                ELSE authors
            END,
            genres = CASE
                WHEN array_length($9::text[], 1) > 0 THEN $9
                ELSE genres
            END,
            cover_url = COALESCE(NULLIF($10, ''), cover_url),
            updated_at = NOW()
        WHERE library_id = $1 AND LOWER(unaccent(name)) = LOWER(unaccent($2))
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .bind(description)
    .bind(&publishers)
    .bind(start_year)
    .bind(total_volumes)
    .bind(&status)
    .bind(&authors)
    .bind(&genres)
    .bind(cover_url)
    .execute(&state.pool)
    .await?;

    // Build report
    let mut report = SeriesSyncReport::default();
    let locked = existing
        .as_ref()
        .map(|r| r.get::<serde_json::Value, _>("locked_fields"))
        .unwrap_or(serde_json::json!({}));
    let is_locked = |field: &str| -> bool {
        locked.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
    };

    // Helper: compare and record field changes
    struct FieldDef {
        name: &'static str,
        old: Option<serde_json::Value>,
        new: Option<serde_json::Value>,
    }

    let fields = vec![
        FieldDef {
            name: "description",
            old: existing.as_ref().and_then(|r| r.get::<Option<String>, _>("description")).map(serde_json::Value::String),
            new: description.map(|s| serde_json::Value::String(s.to_string())),
        },
        FieldDef {
            name: "authors",
            old: existing.as_ref().map(|r| serde_json::json!(r.get::<Vec<String>, _>("authors"))),
            new: if authors.is_empty() { None } else { Some(serde_json::json!(authors)) },
        },
        FieldDef {
            name: "publishers",
            old: existing.as_ref().map(|r| serde_json::json!(r.get::<Vec<String>, _>("publishers"))),
            new: if publishers.is_empty() { None } else { Some(serde_json::json!(publishers)) },
        },
        FieldDef {
            name: "start_year",
            old: existing.as_ref().and_then(|r| r.get::<Option<i32>, _>("start_year")).map(|y| serde_json::json!(y)),
            new: start_year.map(|y| serde_json::json!(y)),
        },
        FieldDef {
            name: "total_volumes",
            old: existing.as_ref().and_then(|r| r.get::<Option<i32>, _>("total_volumes")).map(|y| serde_json::json!(y)),
            new: total_volumes.map(|y| serde_json::json!(y)),
        },
        FieldDef {
            name: "status",
            old: existing.as_ref().and_then(|r| r.get::<Option<String>, _>("status")).map(serde_json::Value::String),
            new: status.as_ref().map(|s: &String| serde_json::Value::String(s.clone())),
        },
    ];

    for f in fields {
        // Skip if no new value to apply
        if f.new.is_none() {
            continue;
        }
        let change = FieldChange {
            field: f.name.to_string(),
            old_value: f.old.clone(),
            new_value: f.new.clone(),
        };
        if is_locked(f.name) {
            report.fields_skipped.push(change);
        } else if f.old != f.new {
            report.fields_updated.push(change);
        }
    }

    Ok(report)
}

pub(crate) async fn sync_books_metadata(
    state: &AppState,
    link_id: Uuid,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    external_id: &str,
) -> Result<(i64, Vec<BookSyncReport>, i64), ApiError> {
    let provider = metadata_providers::get_provider(provider_name)
        .or_else(|| metadata_providers::get_provider("google_books"))
        .ok_or_else(|| ApiError::internal(format!("unknown provider: {provider_name}")))?;

    let provider_config = super::config::load_provider_config(&state.pool, provider_name).await;

    let books = provider
        .get_series_books(external_id, &provider_config)
        .await
        .map_err(|e| ApiError::internal(format!("provider error: {e}")))?;

    // Delete existing book metadata for this link
    sqlx::query("DELETE FROM external_book_metadata WHERE link_id = $1")
        .bind(link_id)
        .execute(&state.pool)
        .await?;

    let mut matched_count: i64 = 0;
    let mut book_reports: Vec<BookSyncReport> = Vec::new();

    // Pre-fetch all local books for this series, sorted like the backoffice
    // (volume ASC NULLS LAST, then natural title sort)
    let local_books: Vec<(Uuid, Option<i32>, String)> = sqlx::query_as(
        r#"
        SELECT b.id, b.volume, b.title FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        WHERE b.library_id = $1
          AND COALESCE(s.name, 'unclassified') = $2
          AND b.volume_type IN ('regular', 'integral')
        ORDER BY b.volume NULLS LAST,
                 REGEXP_REPLACE(LOWER(b.title), '[0-9].*$', ''),
                 COALESCE((REGEXP_MATCH(LOWER(b.title), '\d+'))[1]::int, 0),
                 title ASC
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_all(&state.pool)
    .await?;

    // Build effective position for each local book: use volume if set, otherwise 1-based sort order
    let local_books_with_pos: Vec<(Uuid, i32, String)> = local_books
        .iter()
        .enumerate()
        .map(|(idx, (id, vol, title))| (*id, vol.unwrap_or((idx + 1) as i32), title.clone()))
        .collect();

    // Track which local books have already been matched to avoid double-matching
    let mut matched_local_ids = std::collections::HashSet::new();

    for (ext_idx, book) in books.iter().enumerate() {
        // Skip volume 0 from matching (T0 = hors-série in providers)
        let is_vol_zero = book.volume_number == Some(0);

        // Effective volume for the external book: provider volume_number, or 1-based position
        let ext_vol = book.volume_number.unwrap_or((ext_idx + 1) as i32);

        // Strategy 1: Match by effective volume number (skip vol 0 — T0 = HS in providers)
        let mut local_book_id: Option<Uuid> = if is_vol_zero {
            None
        } else {
            local_books_with_pos
                .iter()
                .find(|(id, v, _)| *v == ext_vol && !matched_local_ids.contains(id))
                .map(|(id, _, _)| *id)
        };

        // Strategy 2: External title contained in local title or vice-versa (case-insensitive)
        if !is_vol_zero && local_book_id.is_none() {
            let ext_title_lower = book.title.to_lowercase();
            local_book_id = local_books_with_pos.iter().find(|(id, _, local_title)| {
                if matched_local_ids.contains(id) {
                    return false;
                }
                let local_lower = local_title.to_lowercase();
                local_lower.contains(&ext_title_lower) || ext_title_lower.contains(&local_lower)
            }).map(|(id, _, _)| *id);
        }

        if let Some(id) = local_book_id {
            matched_local_ids.insert(id);
        }

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
        .execute(&state.pool)
        .await?;

        // Push external metadata to matched local book (respecting locked fields)
        if let Some(book_id) = local_book_id {
            // Fetch current state for report
            let current = sqlx::query(
                "SELECT title, summary, isbn, publish_date, language, authors, locked_fields FROM books WHERE id = $1"
            )
            .bind(book_id)
            .fetch_one(&state.pool)
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
            .bind(&book.summary)
            .bind(&book.isbn)
            .bind(&book.publish_date)
            .bind(&book.language)
            .bind(&book.authors)
            .execute(&state.pool)
            .await?;

            // Build per-book report
            let locked_fields = current.get::<serde_json::Value, _>("locked_fields");
            let is_locked = |field: &str| -> bool {
                locked_fields.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
            };

            let book_title: String = current.get("title");
            let mut fields_updated = Vec::new();
            let mut fields_skipped = Vec::new();

            // Check each syncable field
            let field_checks: Vec<(&str, Option<serde_json::Value>, Option<serde_json::Value>)> = vec![
                ("summary",
                    current.get::<Option<String>, _>("summary").map(|s| serde_json::json!(s)),
                    book.summary.as_ref().map(|s| serde_json::json!(s))),
                ("isbn",
                    current.get::<Option<String>, _>("isbn").map(|s| serde_json::json!(s)),
                    book.isbn.as_ref().map(|s| serde_json::json!(s))),
                ("publish_date",
                    current.get::<Option<String>, _>("publish_date").map(|s| serde_json::json!(s)),
                    book.publish_date.as_ref().map(|s| serde_json::json!(s))),
                ("language",
                    current.get::<Option<String>, _>("language").map(|s| serde_json::json!(s)),
                    book.language.as_ref().map(|s| serde_json::json!(s))),
                ("authors",
                    Some(serde_json::json!(current.get::<Vec<String>, _>("authors"))),
                    if book.authors.is_empty() { None } else { Some(serde_json::json!(&book.authors)) }),
            ];

            for (name, old, new) in field_checks {
                if new.is_none() { continue; }
                let change = FieldChange {
                    field: name.to_string(),
                    old_value: old.clone(),
                    new_value: new.clone(),
                };
                if is_locked(name) {
                    fields_skipped.push(change);
                } else if old != new {
                    fields_updated.push(change);
                }
            }

            // Only include books that had actual changes or skips
            if !fields_updated.is_empty() || !fields_skipped.is_empty() {
                book_reports.push(BookSyncReport {
                    book_id,
                    title: book_title,
                    volume: book.volume_number,
                    fields_updated,
                    fields_skipped,
                });
            }

            matched_count += 1;
        }
    }

    let unmatched = books.len() as i64 - matched_count;
    Ok((matched_count, book_reports, unmatched))
}

#[cfg(test)]
#[path = "tests/sync.rs"]
mod sync_tests;
