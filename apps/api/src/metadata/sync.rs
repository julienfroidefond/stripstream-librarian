use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::field_policy;
use super::shared_sync::{self, LinkMetadata};
use super::types::{BookSyncReport, SyncReport};
use crate::{error::ApiError, metadata_providers};

// ---------------------------------------------------------------------------
// Multi-link sync (primary + secondary fallback)
// ---------------------------------------------------------------------------

/// Sync a series from all its approved links: merge series fields (primary
/// first, secondaries as fallback) and/or merge book metadata across links.
///
/// This is the single sync entry point for approve / change-primary: it reads
/// every approved link, so the result is independent of which link triggered it.
pub(crate) async fn sync_series_from_links(
    pool: &PgPool,
    series_id: Uuid,
    sync_series: bool,
    sync_books: bool,
) -> Result<SyncReport, ApiError> {
    let series_row = sqlx::query("SELECT library_id, name FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("series not found"))?;
    let library_id: Uuid = series_row.get("library_id");
    let series_name: String = series_row.get("name");

    let links = shared_sync::fetch_approved_links(pool, series_id).await?;

    let mut report = SyncReport::default();

    if sync_series {
        report.series =
            Some(field_policy::apply_series(pool, library_id, &series_name, &links).await?);
    }

    if sync_books {
        let (matched, book_reports, unmatched) =
            sync_books_from_links(pool, series_id, library_id, &series_name, &links).await?;
        report.books_matched = matched;
        report.books = book_reports;
        report.books_unmatched = unmatched;

        if matched == 0 && unmatched == 0 {
            report.books_message = Some(
                "This provider does not have volume-level data for this series. \
                 Series metadata was synced, but book matching is not available."
                    .to_string(),
            );
        }

        for link in &links {
            shared_sync::update_link_synced_at(pool, link.id).await?;
        }
    }

    Ok(report)
}

/// Resolve the effective metadata provider name for a stored link (#112).
///
/// Discovery source ids (`senscritique_bd`, `sc_trending_bd`, …) are
/// canonicalized to the provider that implements them. A name that matches no
/// provider — e.g. a link created from a Prowlarr download — falls back to
/// Google Books, the historic behaviour. Returns `None` only when even the
/// fallback provider is unavailable.
fn resolve_link_provider_name(provider_name: &str) -> Option<String> {
    let canonical = metadata_providers::canonical_provider_name(provider_name);
    if metadata_providers::get_provider(canonical).is_some() {
        Some(canonical.to_string())
    } else {
        metadata_providers::get_provider("google_books").map(|_| "google_books".to_string())
    }
}

/// Fetch provider books for every approved link, match them, persist the
/// external rows, then push the merged (primary-first) metadata to local books.
async fn sync_books_from_links(
    pool: &PgPool,
    series_id: Uuid,
    library_id: Uuid,
    series_name: &str,
    links: &[LinkMetadata],
) -> Result<(i64, Vec<BookSyncReport>, i64), ApiError> {
    let mut matched_total: i64 = 0;
    let mut unmatched_total: i64 = 0;

    let local_books = shared_sync::fetch_local_books(pool, library_id, series_name).await?;

    for link in links {
        // Legacy links may carry a discovery source id (#112); resolve it before
        // looking up the provider so it does not silently fall back to Google Books.
        let link_provider = resolve_link_provider_name(&link.provider)
            .ok_or_else(|| ApiError::internal(format!("unknown provider: {}", link.provider)))?;
        let provider = metadata_providers::get_provider(&link_provider)
            .ok_or_else(|| ApiError::internal(format!("unknown provider: {link_provider}")))?;
        let provider_config = super::config::load_provider_config(pool, &link_provider).await;

        let books = provider
            .get_series_books(&link.external_id, &provider_config)
            .await
            .map_err(|e| ApiError::internal(format!("provider error: {e}")))?;

        shared_sync::delete_link_book_metadata(pool, link.id).await?;

        let matched = if link_provider == "bdtheque" || link_provider == "bdphile" {
            shared_sync::match_books_for_new_provider(&books, &local_books)
        } else {
            shared_sync::match_books(&books, &local_books)
        };

        let mut matched_count: i64 = 0;
        for m in &matched {
            shared_sync::insert_external_book_metadata(pool, link.id, m.local_book_id, m.ext_book)
                .await?;
            if m.local_book_id.is_some() {
                matched_count += 1;
            }
        }

        matched_total += matched_count;
        unmatched_total += books.len() as i64 - matched_count;
    }

    // Push merged metadata across all approved links (primary first).
    let book_reports = field_policy::apply_books(pool, series_id).await?;

    Ok((matched_total, book_reports, unmatched_total))
}

#[cfg(test)]
#[path = "tests/sync.rs"]
mod sync_tests;
