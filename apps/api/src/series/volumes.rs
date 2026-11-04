use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

/// Volume coverage of a series.
///
/// Mirrors the canonical missing-volume rule used by the admin UI: numbered
/// `regular` and `integral` books count as owned, while HS and one-shots never
/// do. A single unnumbered integral marks the whole series as complete.
#[derive(Serialize, ToSchema)]
pub struct SeriesVolumesDto {
    /// Distinct owned volume numbers, ascending. Numbered `regular` and
    /// `integral` books count; HS, one-shots and NULL volumes are excluded.
    pub owned_volumes: Vec<i32>,
    /// True when the series owns an unnumbered integral. The whole series is
    /// then considered complete, so no result should be highlighted.
    pub has_unnumbered_integral: bool,
}

/// List the volume coverage of a series.
///
/// Used by the backoffice to highlight Telegram search results against the whole
/// series instead of only the currently displayed page.
#[utoipa::path(
    get,
    path = "/series/{series_id}/volumes",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, body = SeriesVolumesDto),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_owned_volumes(
    State(state): State<AppState>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesVolumesDto>, ApiError> {
    let row = sqlx::query(
        "SELECT COALESCE(ARRAY_AGG(DISTINCT b.volume ORDER BY b.volume) \
                FILTER (WHERE b.volume_type IN ('regular','integral') AND b.volume IS NOT NULL), \
                ARRAY[]::int[]) AS owned_volumes, \
                COUNT(b.id) FILTER (WHERE b.volume_type = 'integral' AND b.volume IS NULL) > 0 \
                AS has_unnumbered_integral \
         FROM series s \
         LEFT JOIN books b ON b.series_id = s.id \
         WHERE s.id = $1 \
         GROUP BY s.id",
    )
    .bind(series_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found(format!("series '{}' not found", series_id)))?;

    let owned_volumes: Vec<i32> = row.get("owned_volumes");
    let has_unnumbered_integral: bool = row.get("has_unnumbered_integral");

    Ok(Json(SeriesVolumesDto {
        owned_volumes,
        has_unnumbered_integral,
    }))
}
