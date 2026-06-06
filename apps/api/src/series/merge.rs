use axum::extract::Extension;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::helpers::resolve_library_id;
use crate::{auth::AuthUser, error::ApiError, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct MergeSeriesRequest {
    /// The series to absorb (will be deleted after merge)
    #[schema(value_type = String)]
    pub source_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub struct MergeSeriesResponse {
    pub books_moved: u64,
    pub metadata_moved: u64,
    pub downloads_moved: u64,
}

/// Merge source series into target series.
/// Moves books, metadata links, and available downloads from source to target,
/// then deletes the source series.
#[utoipa::path(
    post,
    path = "/series/{target_id}/merge",
    tag = "series",
    params(
        ("target_id" = String, Path, description = "Target series UUID (kept)"),
    ),
    request_body = MergeSeriesRequest,
    responses(
        (status = 200, body = MergeSeriesResponse),
        (status = 400, description = "Cannot merge a series into itself"),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn merge_series(
    State(state): State<AppState>,
    _user: Option<Extension<AuthUser>>,
    Path(target_id): Path<Uuid>,
    Json(body): Json<MergeSeriesRequest>,
) -> Result<Json<MergeSeriesResponse>, ApiError> {
    let source_id = body.source_id;

    if target_id == source_id {
        return Err(ApiError::bad_request("Cannot merge a series into itself"));
    }

    // Verify both series exist and are in the same library
    let target_lib: Uuid = resolve_library_id(&state.pool, target_id).await?;
    let source_lib: Uuid = resolve_library_id(&state.pool, source_id).await?;
    if target_lib != source_lib {
        return Err(ApiError::bad_request(
            "Cannot merge series from different libraries",
        ));
    }

    let mut tx = state.pool.begin().await?;

    // 1. Move books from source to target
    let books_result = sqlx::query("UPDATE books SET series_id = $1 WHERE series_id = $2")
        .bind(target_id)
        .bind(source_id)
        .execute(&mut *tx)
        .await?;
    let books_moved = books_result.rows_affected();

    // 2. Move metadata links that target doesn't already have (by provider)
    let metadata_result = sqlx::query(
        "UPDATE external_metadata_links SET series_id = $1 \
         WHERE series_id = $2 \
         AND provider NOT IN (SELECT provider FROM external_metadata_links WHERE series_id = $1)",
    )
    .bind(target_id)
    .bind(source_id)
    .execute(&mut *tx)
    .await?;
    let metadata_moved = metadata_result.rows_affected();

    // Delete remaining source metadata links (duplicates of target's providers)
    sqlx::query("DELETE FROM external_metadata_links WHERE series_id = $1")
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

    // 3. Move available downloads that target doesn't already have
    let downloads_result = sqlx::query(
        "UPDATE available_downloads SET series_id = $1 \
         WHERE series_id = $2 \
         AND id NOT IN ( \
             SELECT ad2.id FROM available_downloads ad2 \
             WHERE ad2.series_id = $1 \
         )",
    )
    .bind(target_id)
    .bind(source_id)
    .execute(&mut *tx)
    .await?;
    let downloads_moved = downloads_result.rows_affected();

    // Delete remaining source downloads
    sqlx::query("DELETE FROM available_downloads WHERE series_id = $1")
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

    // 4. Move anilist links if target doesn't have one
    sqlx::query(
        "UPDATE anilist_series_links SET series_id = $1 \
         WHERE series_id = $2 \
         AND NOT EXISTS (SELECT 1 FROM anilist_series_links WHERE series_id = $1)",
    )
    .bind(target_id)
    .bind(source_id)
    .execute(&mut *tx)
    .await?;

    // Delete remaining source anilist links
    sqlx::query("DELETE FROM anilist_series_links WHERE series_id = $1")
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

    // 5. Delete the source series
    sqlx::query("DELETE FROM series WHERE id = $1")
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Json(MergeSeriesResponse {
        books_moved,
        metadata_moved,
        downloads_moved,
    }))
}
