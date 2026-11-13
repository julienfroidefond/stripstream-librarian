use serde::Serialize;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use super::field_policy;
use super::shared_sync;
use super::types::FieldChange;
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

fn field_diff(change: FieldChange) -> FieldDiff {
    FieldDiff {
        field: change.field,
        old: change.old_value,
        new: change.new_value,
    }
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
    // Legacy links created before #112 may still carry a discovery source id
    // (`senscritique_bd`, `sc_trending_bd`, …). Resolve it to the metadata
    // provider that actually backs it instead of failing on an unknown provider.
    let provider = metadata_providers::resolve_provider(provider_name)
        .map_err(|unknown| format!("Unknown provider: {unknown}"))?;
    let provider_name = provider.name();

    let config = super::config::load_provider_config(pool, provider_name).await;

    let series_id: Uuid =
        sqlx::query_scalar("SELECT series_id FROM external_metadata_links WHERE id = $1")
            .bind(link_id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

    let mut book_changes: Vec<BookDiff> = Vec::new();

    // -- Series-level refresh --
    let candidate = provider
        .get_series(external_id, &config)
        .await
        .map_err(|e| format!("provider lookup by external ID failed: {e}"))?;

    // Store the provider's latest series data and rating notes on the link.
    super::links::record_provider_snapshot(
        pool,
        link_id,
        &candidate.metadata_json,
        candidate.total_volumes,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Merge series fields across all approved links (primary first), so the
    // result does not depend on which link triggered this refresh.
    let links = shared_sync::fetch_approved_links(pool, series_id)
        .await
        .map_err(|e| e.to_string())?;
    let series_report = field_policy::apply_series(pool, library_id, series_name, &links)
        .await
        .map_err(|e| e.to_string())?;
    // Locked fields are reported separately as skipped; a refresh diff only
    // lists actual updates.
    let series_changes: Vec<FieldDiff> = series_report
        .fields_updated
        .into_iter()
        .map(field_diff)
        .collect();

    // -- Book-level refresh --
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

    let matched = if provider_name == "bdtheque" || provider_name == "bdphile" {
        shared_sync::match_books_for_new_provider(&books, &local_books)
    } else {
        shared_sync::match_books(&books, &local_books)
    };

    for m in &matched {
        shared_sync::insert_external_book_metadata(pool, link_id, m.local_book_id, m.ext_book)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Push merged book metadata across all approved links (primary first).
    let book_reports = field_policy::apply_books(pool, series_id)
        .await
        .map_err(|e| e.to_string())?;
    for report in book_reports {
        if report.fields_updated.is_empty() {
            continue;
        }
        book_changes.push(BookDiff {
            book_id: report.book_id.to_string(),
            title: report.title,
            volume: report.volume,
            changes: report.fields_updated.into_iter().map(field_diff).collect(),
        });
    }

    super::links::mark_synced(pool, link_id)
        .await
        .map_err(|e| e.to_string())?;

    // Re-match any external books that couldn't be matched during insert
    let _ = rematch_unlinked_books(pool, library_id).await;

    let has_changes = !series_changes.is_empty() || !book_changes.is_empty();

    Ok(super::refresh::SeriesRefreshResult {
        series_name: series_name.to_string(),
        provider: provider_name.to_string(),
        status: if has_changes {
            "updated".to_string()
        } else {
            "unchanged".to_string()
        },
        series_changes,
        book_changes,
        error: None,
    })
}

/// Re-match external_book_metadata rows that have book_id IS NULL
/// by joining on volume number with local books in the same series.
///
/// At most one external row per (link, local book) is ever linked. Linking a
/// second edition of the same volume to the same book used to make the merge
/// pick a random row on every refresh (issue #65).
pub async fn rematch_unlinked_books(pool: &PgPool, library_id: Uuid) -> Result<i64, String> {
    let result = sqlx::query(
        r#"
        WITH candidates AS (
            -- One row per unlinked external book. When several local books share
            -- the same volume number, the lowest book id wins (deterministic).
            SELECT DISTINCT ON (ebm2.id)
                   ebm2.id AS ebm_id,
                   ebm2.link_id,
                   b.id AS book_id,
                   ebm2.external_book_id
            FROM external_book_metadata ebm2
            JOIN external_metadata_links eml ON eml.id = ebm2.link_id
            JOIN books b ON b.library_id = eml.library_id
                AND b.series_id = eml.series_id
                AND b.volume = ebm2.volume_number
                AND b.volume_type IN ('regular', 'integral')
            WHERE eml.library_id = $1
              AND ebm2.book_id IS NULL
              AND ebm2.volume_number IS NOT NULL
              AND ebm2.volume_number != 0
              AND eml.status = 'approved'
              AND NOT EXISTS (
                  SELECT 1 FROM external_book_metadata linked
                  WHERE linked.link_id = ebm2.link_id
                    AND linked.book_id = b.id
              )
            ORDER BY ebm2.id, b.id
        ), ranked AS (
            -- At most one external book per (link, local book). Duplicates by
            -- volume number are broken by the external book id, which is stable
            -- across refreshes, unlike the row id (a random UUID).
            SELECT DISTINCT ON (link_id, book_id)
                   ebm_id, book_id
            FROM candidates
            ORDER BY link_id, book_id,
                     external_book_id ASC NULLS LAST, ebm_id ASC
        )
        UPDATE external_book_metadata ebm
        SET book_id = ranked.book_id
        FROM ranked
        WHERE ebm.id = ranked.ebm_id
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

#[cfg(test)]
#[path = "tests/refresh_sync.rs"]
mod tests;
