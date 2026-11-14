use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use super::update::delete_series_core;
use crate::{error::ApiError, state::AppState};

// ─── Types ───────────────────────────────────────────────────────────────────

/// A series with zero books (discovery leftovers).
#[derive(Serialize, ToSchema)]
pub struct OrphanSeriesItem {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub library_name: Option<String>,
    /// Provider of the approved metadata link, if any.
    pub metadata_provider: Option<String>,
    /// External URL carried by the approved metadata link, if any.
    pub external_url: Option<String>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    /// Whole days elapsed since the series row was created.
    pub age_days: i64,
    /// A release for this series was detected on the wishlist. This is not an
    /// in-progress download: it is only a match that could be downloaded later.
    pub has_wishlist_release: bool,
    /// A `torrent_downloads` row for this series is still downloading,
    /// waiting for import, or importing. The series has no book yet, so it is
    /// excluded from bulk purges to avoid racing the import.
    pub has_active_download: bool,
}

#[derive(Serialize, ToSchema)]
pub struct OrphanSeriesPage {
    pub items: Vec<OrphanSeriesItem>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct OrphanSeriesQuery {
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 50)]
    pub limit: Option<i64>,
}

// ─── SQL ─────────────────────────────────────────────────────────────────────

const ORPHAN_FILTER: &str = "NOT EXISTS (SELECT 1 FROM books b WHERE b.series_id = s.id)";

// ─── Handlers ────────────────────────────────────────────────────────────────

/// List series that have no book at all, oldest first.
#[utoipa::path(
    get,
    path = "/admin/series/orphans",
    tag = "series",
    params(
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
    ),
    responses(
        (status = 200, body = OrphanSeriesPage),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_orphan_series(
    State(state): State<AppState>,
    Query(query): Query<OrphanSeriesQuery>,
) -> Result<Json<OrphanSeriesPage>, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1).saturating_mul(limit);

    let count_sql = format!("SELECT COUNT(*) FROM series s WHERE {ORPHAN_FILTER}");
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(count_sql.as_str()))
        .fetch_one(&state.pool)
        .await?;

    let data_sql = format!(
        r#"
        SELECT
            s.id,
            s.name,
            s.library_id,
            l.name AS library_name,
            ml.provider AS metadata_provider,
            ml.external_url,
            s.created_at,
            GREATEST(
                0,
                EXTRACT(EPOCH FROM (NOW() - s.created_at))::bigint / 86400
            ) AS age_days,
            EXISTS (
                SELECT 1 FROM available_downloads ad WHERE ad.series_id = s.id
            ) AS has_wishlist_release,
            EXISTS (
                SELECT 1 FROM torrent_downloads td
                WHERE td.library_id = s.library_id
                  AND norm_text(td.series_name) = norm_text(s.name)
                  AND td.status IN ('downloading', 'completed', 'importing')
            ) AS has_active_download
        FROM series s
        LEFT JOIN libraries l ON l.id = s.library_id
        LEFT JOIN LATERAL (
            SELECT eml.provider, eml.external_url
            FROM external_metadata_links eml
            WHERE eml.series_id = s.id
              AND eml.library_id = s.library_id
              AND eml.status = 'approved'
            ORDER BY eml.is_primary DESC, eml.created_at DESC
            LIMIT 1
        ) ml ON TRUE
        WHERE {ORPHAN_FILTER}
        ORDER BY s.created_at ASC, norm_text(s.name) ASC, s.id ASC
        LIMIT $1 OFFSET $2
        "#
    );

    let rows = sqlx::query(sqlx::AssertSqlSafe(data_sql.as_str()))
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;

    let items = rows
        .into_iter()
        .map(|row| OrphanSeriesItem {
            id: row.get("id"),
            name: row.get("name"),
            library_id: row.get("library_id"),
            library_name: row.get("library_name"),
            metadata_provider: row.get("metadata_provider"),
            external_url: row.get("external_url"),
            created_at: row.get("created_at"),
            age_days: row.get("age_days"),
            has_wishlist_release: row.get("has_wishlist_release"),
            has_active_download: row.get("has_active_download"),
        })
        .collect();

    Ok(Json(OrphanSeriesPage {
        items,
        total,
        page,
        limit,
    }))
}

/// Delete a series that still has no book, re-checking orphanhood under lock.
///
/// The check and the deletion share one transaction. `FOR UPDATE` conflicts
/// with the `FOR KEY SHARE` lock PostgreSQL takes on the referenced series row
/// when a book is inserted, so a concurrent import either completes before the
/// check or fails once the series row is gone. A series that acquired a book
/// since it was listed returns `409` instead of being deleted.
#[utoipa::path(
    delete,
    path = "/admin/series/orphans/{series_id}",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, description = "Orphan series deleted"),
        (status = 400, description = "Series id is not a valid UUID"),
        (status = 404, description = "Series not found"),
        (status = 409, description = "Series is no longer an orphan (it has books)"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_orphan_series(
    State(state): State<AppState>,
    // Typed extractor: any non-UUID segment (including path-traversal payloads
    // relayed by a proxy) is rejected with 400 before the handler runs.
    Path(series_id): Path<Uuid>,
) -> Result<Json<crate::responses::DeletedResponse>, ApiError> {
    let mut tx = state.pool.begin().await?;

    let library_id: Option<Uuid> =
        sqlx::query_scalar("SELECT library_id FROM series WHERE id = $1 FOR UPDATE")
            .bind(series_id)
            .fetch_optional(&mut *tx)
            .await?;
    let library_id = library_id.ok_or_else(|| ApiError::not_found("series not found"))?;

    let has_books =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM books WHERE series_id = $1)")
            .bind(series_id)
            .fetch_one(&mut *tx)
            .await?;
    if has_books {
        return Err(ApiError::conflict(
            "series is no longer an orphan: it has at least one book",
        ));
    }

    // An orphan has no book, so it has no files to delete: never let the shared
    // deletion helper derive a directory from the series name.
    let cleanup =
        delete_series_core(&mut tx, &state.library_root, library_id, series_id, false).await?;
    tx.commit().await?;
    cleanup.run(&state.pool, library_id).await?;

    Ok(Json(crate::responses::DeletedResponse::new(library_id)))
}
