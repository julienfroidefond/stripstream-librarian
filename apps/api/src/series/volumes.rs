use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;
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
    let coverage = crate::coverage::load_series_coverage(&state.pool, series_id)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("series '{}' not found", series_id)))?;

    Ok(Json(SeriesVolumesDto {
        owned_volumes: coverage.owned_volumes(),
        has_unnumbered_integral: coverage.has_unnumbered_integral(),
    }))
}
