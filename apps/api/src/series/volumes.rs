use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

/// Regular volume numbers owned for a series.
#[derive(Serialize, ToSchema)]
pub struct SeriesVolumesDto {
    /// Distinct regular volume numbers, ascending. Empty when the series has no
    /// regular book (HS, one-shots, integrals and NULL volumes are excluded).
    pub owned_volumes: Vec<i32>,
}

/// List the regular volume numbers owned for a series.
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
    let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(ApiError::not_found(format!(
            "series '{}' not found",
            series_id
        )));
    }

    let owned_volumes: Vec<i32> = sqlx::query_scalar(
        "SELECT DISTINCT volume FROM books \
         WHERE series_id = $1 AND volume_type = 'regular' AND volume IS NOT NULL \
         ORDER BY volume",
    )
    .bind(series_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(SeriesVolumesDto { owned_volumes }))
}
