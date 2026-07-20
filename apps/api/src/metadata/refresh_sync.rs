use serde::Serialize;
use sqlx::{PgPool, Row};
use tracing::info;
use uuid::Uuid;

use super::shared_sync::{self, diff_opt_i32, diff_opt_str, diff_str_vec, is_field_locked};
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
    let candidate = provider
        .get_series(external_id, &config)
        .await
        .map_err(|e| format!("provider lookup by external ID failed: {e}"))?;

    let (pr_rating, pr_rating_count, pr_rating_scale) =
        shared_sync::extract_provider_rating(&candidate.metadata_json);
    // Update link metadata_json and provider rating columns
    sqlx::query(
        r#"
            UPDATE external_metadata_links
            SET metadata_json = $2,
                total_volumes_external = $3,
                provider_rating = $4,
                provider_rating_count = $5,
                provider_rating_scale = $6,
                updated_at = NOW()
            WHERE id = $1
        "#,
    )
    .bind(link_id)
    .bind(&candidate.metadata_json)
    .bind(candidate.total_volumes)
    .bind(pr_rating)
    .bind(pr_rating_count)
    .bind(pr_rating_scale)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Diff + sync series metadata
    series_changes = sync_series_with_diff(pool, library_id, series_name, &candidate).await?;

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

    let matched = shared_sync::match_books(&books, &local_books);

    for m in &matched {
        shared_sync::insert_external_book_metadata(pool, link_id, m.local_book_id, m.ext_book)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(book_id) = m.local_book_id {
            let diffs = sync_book_with_diff(pool, book_id, m.ext_book).await?;
            if !diffs.is_empty() {
                let local_title = local_books
                    .iter()
                    .find(|(id, _, _)| *id == book_id)
                    .map(|(_, _, t)| t.clone())
                    .unwrap_or_default();
                book_changes.push(BookDiff {
                    book_id: book_id.to_string(),
                    title: local_title,
                    volume: m.ext_book.volume_number,
                    changes: diffs,
                });
            }
        }
    }

    shared_sync::update_link_synced_at(pool, link_id)
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

// ---------------------------------------------------------------------------
// Series sync with diff tracking
// ---------------------------------------------------------------------------

async fn sync_series_with_diff(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    candidate: &metadata_providers::SeriesCandidate,
) -> Result<Vec<FieldDiff>, String> {
    let fields =
        shared_sync::extract_series_fields(pool, &candidate.metadata_json, Some(candidate), None)
            .await;

    let existing = shared_sync::upsert_series_metadata(pool, library_id, series_name, &fields)
        .await
        .map_err(|e| e.to_string())?;

    // Build diffs from pre-update state
    let locked = existing
        .as_ref()
        .map(|r| r.get::<serde_json::Value, _>("locked_fields"))
        .unwrap_or(serde_json::json!({}));

    let mut diffs: Vec<FieldDiff> = Vec::new();

    if !is_field_locked(&locked, "description") {
        let old: Option<String> = existing.as_ref().and_then(|r| r.get("description"));
        if let Some((old_v, new_v)) = diff_opt_str(old.as_deref(), fields.description.as_deref()) {
            diffs.push(FieldDiff {
                field: "description".into(),
                old: old_v,
                new: new_v,
            });
        }
    }
    if !is_field_locked(&locked, "authors") {
        let old: Vec<String> = existing
            .as_ref()
            .map(|r| r.get("authors"))
            .unwrap_or_default();
        if let Some((old_v, new_v)) = diff_str_vec(&old, &fields.authors) {
            diffs.push(FieldDiff {
                field: "authors".into(),
                old: old_v,
                new: new_v,
            });
        }
    }
    if !is_field_locked(&locked, "publishers") {
        let old: Vec<String> = existing
            .as_ref()
            .map(|r| r.get("publishers"))
            .unwrap_or_default();
        if let Some((old_v, new_v)) = diff_str_vec(&old, &fields.publishers) {
            diffs.push(FieldDiff {
                field: "publishers".into(),
                old: old_v,
                new: new_v,
            });
        }
    }
    if !is_field_locked(&locked, "start_year") {
        let old: Option<i32> = existing.as_ref().and_then(|r| r.get("start_year"));
        if let Some((old_v, new_v)) = diff_opt_i32(old, fields.start_year) {
            diffs.push(FieldDiff {
                field: "start_year".into(),
                old: old_v,
                new: new_v,
            });
        }
    }
    if !is_field_locked(&locked, "total_volumes") {
        let old: Option<i32> = existing.as_ref().and_then(|r| r.get("total_volumes"));
        if let Some((old_v, new_v)) = diff_opt_i32(old, fields.total_volumes) {
            diffs.push(FieldDiff {
                field: "total_volumes".into(),
                old: old_v,
                new: new_v,
            });
        }
    }
    if !is_field_locked(&locked, "status") {
        let old: Option<String> = existing.as_ref().and_then(|r| r.get("status"));
        if let Some((old_v, new_v)) = diff_opt_str(old.as_deref(), fields.status.as_deref()) {
            diffs.push(FieldDiff {
                field: "status".into(),
                old: old_v,
                new: new_v,
            });
        }
    }

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
    let current = shared_sync::push_book_metadata(pool, book_id, ext_book)
        .await
        .map_err(|e| e.to_string())?;

    let locked = current.get::<serde_json::Value, _>("locked_fields");
    let mut diffs: Vec<FieldDiff> = Vec::new();

    if !is_field_locked(&locked, "summary") {
        let old: Option<String> = current.get("summary");
        if let Some((old_v, new_v)) = diff_opt_str(old.as_deref(), ext_book.summary.as_deref()) {
            diffs.push(FieldDiff {
                field: "summary".into(),
                old: old_v,
                new: new_v,
            });
        }
    }
    if !is_field_locked(&locked, "isbn") {
        let old: Option<String> = current.get("isbn");
        if let Some((old_v, new_v)) = diff_opt_str(old.as_deref(), ext_book.isbn.as_deref()) {
            diffs.push(FieldDiff {
                field: "isbn".into(),
                old: old_v,
                new: new_v,
            });
        }
    }
    if !is_field_locked(&locked, "publish_date") {
        let old: Option<String> = current.get("publish_date");
        if let Some((old_v, new_v)) = diff_opt_str(old.as_deref(), ext_book.publish_date.as_deref())
        {
            diffs.push(FieldDiff {
                field: "publish_date".into(),
                old: old_v,
                new: new_v,
            });
        }
    }
    if !is_field_locked(&locked, "language") {
        let old: Option<String> = current.get("language");
        if let Some((old_v, new_v)) = diff_opt_str(old.as_deref(), ext_book.language.as_deref()) {
            diffs.push(FieldDiff {
                field: "language".into(),
                old: old_v,
                new: new_v,
            });
        }
    }
    if !is_field_locked(&locked, "authors") {
        let old: Vec<String> = current.get("authors");
        if let Some((old_v, new_v)) = diff_str_vec(&old, &ext_book.authors) {
            diffs.push(FieldDiff {
                field: "authors".into(),
                old: old_v,
                new: new_v,
            });
        }
    }

    Ok(diffs)
}

/// Re-match external_book_metadata rows that have book_id IS NULL
/// by joining on volume number with local books in the same series.
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
                AND b.volume_type IN ('regular', 'integral')
            WHERE eml.library_id = $1
              AND ebm2.book_id IS NULL
              AND ebm2.volume_number IS NOT NULL
              AND ebm2.volume_number != 0
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
