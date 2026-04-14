use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::metadata_providers;
use super::shared_sync;

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
         AND b.volume_type IN ('regular', 'integral')",
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
    let fields = shared_sync::extract_series_fields(
        pool, &candidate.metadata_json, Some(candidate), None,
    ).await;

    shared_sync::upsert_series_metadata(pool, library_id, series_name, &fields)
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

    shared_sync::delete_link_book_metadata(pool, link_id)
        .await
        .map_err(|e| e.to_string())?;

    let local_books = shared_sync::fetch_local_books(pool, library_id, series_name)
        .await
        .map_err(|e| e.to_string())?;

    let matched = shared_sync::match_books(&books, &local_books);

    for m in &matched {
        shared_sync::insert_external_book_metadata(pool, link_id, m.local_book_id, m.ext_book)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(book_id) = m.local_book_id {
            shared_sync::push_book_metadata(pool, book_id, m.ext_book)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    shared_sync::update_link_synced_at(pool, link_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
