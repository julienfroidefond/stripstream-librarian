use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::metadata_providers;

// ---------------------------------------------------------------------------
// Search evaluation
// ---------------------------------------------------------------------------

pub(super) enum SearchOutcome {
    AutoMatch(metadata_providers::SeriesCandidate),
    NoResults,
    TooManyResults(i32, Option<metadata_providers::SeriesCandidate>),
    LowConfidence(metadata_providers::SeriesCandidate),
    Error(String),
}

pub(super) async fn search_and_evaluate(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    detailed: bool,
) -> SearchOutcome {
    let provider = match metadata_providers::get_provider(provider_name) {
        Some(p) => p,
        None => return SearchOutcome::Error(format!("Unknown provider: {provider_name}")),
    };

    let mut config = super::config::load_provider_config(pool, provider_name).await;
    config.detailed = detailed;

    let mut candidates = match provider.search_series(series_name, &config).await {
        Ok(c) => c,
        Err(e) => return SearchOutcome::Error(e),
    };

    if candidates.is_empty() {
        return SearchOutcome::NoResults;
    }

    // Boost confidence based on local book count vs candidate total_volumes
    let local_count: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books b \
         JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 AND LOWER(unaccent(s.name)) = LOWER(unaccent($2)) \
         AND b.volume_type = 'regular'",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_one(pool)
    .await
    .ok();

    if let Some(count) = local_count {
        super::config::boost_confidence_by_book_count(&mut candidates, count);
    }

    // Only auto-match if exactly 1 result with confidence == 1.0
    if candidates.len() == 1 && (candidates[0].confidence - 1.0).abs() < f32::EPSILON {
        return SearchOutcome::AutoMatch(candidates.into_iter().next().unwrap());
    }

    // Check if best candidate has perfect confidence
    let best = candidates.into_iter().next().unwrap();
    if (best.confidence - 1.0).abs() < f32::EPSILON {
        return SearchOutcome::AutoMatch(best);
    }

    if best.confidence < 1.0 {
        return SearchOutcome::LowConfidence(best);
    }

    SearchOutcome::TooManyResults(0, None)
}

pub(super) async fn auto_apply(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    candidate: &metadata_providers::SeriesCandidate,
) -> Result<Uuid, String> {
    // Resolve series_id from series name
    let series_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM series WHERE library_id = $1 AND name = $2",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("Series '{}' not found in library", series_name))?;

    // Create the external_metadata_link
    let metadata_json = &candidate.metadata_json;
    let row = sqlx::query(
        r#"
        INSERT INTO external_metadata_links
            (library_id, series_id, provider, external_id, external_url, status, confidence, metadata_json, total_volumes_external)
        VALUES ($1, $2, $3, $4, $5, 'approved', $6, $7, $8)
        ON CONFLICT (series_id, provider)
        DO UPDATE SET
            external_id = EXCLUDED.external_id,
            external_url = EXCLUDED.external_url,
            status = 'approved',
            confidence = EXCLUDED.confidence,
            metadata_json = EXCLUDED.metadata_json,
            total_volumes_external = EXCLUDED.total_volumes_external,
            matched_at = NOW(),
            approved_at = NOW(),
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(library_id)
    .bind(series_id)
    .bind(provider_name)
    .bind(&candidate.external_id)
    .bind(&candidate.external_url)
    .bind(candidate.confidence)
    .bind(metadata_json)
    .bind(candidate.total_volumes)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let link_id: Uuid = row.get("id");

    // Sync series metadata
    sync_series_from_candidate(pool, library_id, series_name, candidate).await?;

    // Sync books
    sync_books_from_provider(pool, link_id, library_id, series_name, provider_name, &candidate.external_id).await?;

    Ok(link_id)
}

/// Sync series metadata from a candidate (simplified version for batch use)
async fn sync_series_from_candidate(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    candidate: &metadata_providers::SeriesCandidate,
) -> Result<(), String> {
    let description = candidate.metadata_json
        .get("description")
        .and_then(|d| d.as_str())
        .or(candidate.description.as_deref());
    let authors = &candidate.authors;
    let publishers = &candidate.publishers;
    let start_year = candidate.start_year;
    let total_volumes = candidate.total_volumes;
    let status = if let Some(raw) = candidate.metadata_json.get("status").and_then(|s| s.as_str()) {
        Some(super::handlers::normalize_series_status(pool, raw).await)
    } else {
        None
    };
    let status = status.as_deref();

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
    .bind(description)
    .bind(publishers)
    .bind(start_year)
    .bind(total_volumes)
    .bind(status)
    .bind(authors)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Sync books from provider (simplified for batch use)
async fn sync_books_from_provider(
    pool: &PgPool,
    link_id: Uuid,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    external_id: &str,
) -> Result<(), String> {
    let provider = metadata_providers::get_provider(provider_name)
        .ok_or_else(|| format!("Unknown provider: {provider_name}"))?;

    let config = super::config::load_provider_config(pool, provider_name).await;

    let books = provider
        .get_series_books(external_id, &config)
        .await
        .map_err(|e| format!("provider books error: {e}"))?;

    // Delete existing book metadata for this link
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

        // Push metadata to matched local book
        if let Some(book_id) = local_book_id {
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
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
    }

    // Update synced_at on the link
    sqlx::query("UPDATE external_metadata_links SET synced_at = NOW(), updated_at = NOW() WHERE id = $1")
        .bind(link_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
