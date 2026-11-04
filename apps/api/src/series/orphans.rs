use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

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
    /// A wishlist download is in progress for this series (deletion discouraged).
    pub has_available_download: bool,
    /// Always `0`: orphans have no book by definition.
    pub books_count: i64,
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

// ─── Handler ─────────────────────────────────────────────────────────────────

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
    let offset = (page - 1) * limit;

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
            ) AS has_available_download,
            0::bigint AS books_count
        FROM series s
        LEFT JOIN libraries l ON l.id = s.library_id
        LEFT JOIN LATERAL (
            SELECT eml.provider, eml.external_url
            FROM external_metadata_links eml
            WHERE eml.series_id = s.id AND eml.status = 'approved'
            ORDER BY eml.is_primary DESC, eml.created_at DESC
            LIMIT 1
        ) ml ON TRUE
        WHERE {ORPHAN_FILTER}
        ORDER BY s.created_at ASC, lower(s.name) ASC
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
            has_available_download: row.get("has_available_download"),
            books_count: row.get("books_count"),
        })
        .collect();

    Ok(Json(OrphanSeriesPage {
        items,
        total,
        page,
        limit,
    }))
}
