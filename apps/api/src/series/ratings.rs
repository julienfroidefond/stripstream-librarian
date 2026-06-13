use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::ApiError,
    integrations::{anilist::load_anilist_settings, anilist_rating_push::push_rating_to_anilist},
    state::AppState,
};

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct ProviderRating {
    pub provider: String,
    pub rating: f64,
    pub rating_scale: f64,
    pub rating_count: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct SeriesRatingsResponse {
    pub user_rating: Option<i16>,
    pub anilist_pulled_rating: Option<f64>,
    pub provider_ratings: Vec<ProviderRating>,
}

#[derive(Deserialize, ToSchema)]
pub struct SetRatingRequest {
    /// Half-star rating: 1 (0.5 stars) to 10 (5 stars)
    #[schema(minimum = 1, maximum = 10)]
    pub rating: i16,
}

// ─── GET /series/:series_id/ratings ──────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/series/{series_id}/ratings",
    tag = "series",
    params(("series_id" = String, Path, description = "Series UUID")),
    responses(
        (status = 200, body = SeriesRatingsResponse),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_ratings(
    State(state): State<AppState>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesRatingsResponse>, ApiError> {
    // Resolve authenticated user from token (use first admin user as fallback for single-user setups)
    let user_id = resolve_user_id(&state).await?;

    let user_rating: Option<i16> = sqlx::query_scalar(
        "SELECT rating FROM series_user_ratings WHERE user_id = $1 AND series_id = $2",
    )
    .bind(user_id)
    .bind(series_id)
    .fetch_optional(&state.pool)
    .await?;

    let anilist_pulled_rating: Option<f64> = sqlx::query_scalar(
        "SELECT user_score FROM anilist_series_links WHERE series_id = $1 LIMIT 1",
    )
    .bind(series_id)
    .fetch_optional(&state.pool)
    .await?
    .flatten();

    let provider_rows = sqlx::query(
        r#"
        SELECT provider, provider_rating, provider_rating_scale, provider_rating_count
        FROM external_metadata_links
        WHERE series_id = $1
          AND status = 'approved'
          AND provider_rating IS NOT NULL
        ORDER BY provider
        "#,
    )
    .bind(series_id)
    .fetch_all(&state.pool)
    .await?;

    let provider_ratings: Vec<ProviderRating> = provider_rows
        .iter()
        .map(|row| ProviderRating {
            provider: row.get("provider"),
            rating: row.get::<f32, _>("provider_rating") as f64,
            rating_scale: row
                .get::<Option<f32>, _>("provider_rating_scale")
                .unwrap_or(10.0) as f64,
            rating_count: row.get("provider_rating_count"),
        })
        .collect();

    Ok(Json(SeriesRatingsResponse {
        user_rating,
        anilist_pulled_rating,
        provider_ratings,
    }))
}

// ─── PUT /series/:series_id/rating ───────────────────────────────────────────

#[utoipa::path(
    put,
    path = "/series/{series_id}/rating",
    tag = "series",
    params(("series_id" = String, Path, description = "Series UUID")),
    request_body = SetRatingRequest,
    responses(
        (status = 204, description = "Rating saved"),
        (status = 400, description = "Invalid rating value"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn set_series_rating(
    State(state): State<AppState>,
    Path(series_id): Path<Uuid>,
    Json(body): Json<SetRatingRequest>,
) -> Result<StatusCode, ApiError> {
    if !(1..=10).contains(&body.rating) {
        return Err(ApiError::bad_request("rating must be between 1 and 10"));
    }

    let user_id = resolve_user_id(&state).await?;

    sqlx::query(
        r#"
        INSERT INTO series_user_ratings (user_id, series_id, rating, updated_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (user_id, series_id) DO UPDATE
            SET rating = EXCLUDED.rating, updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(series_id)
    .bind(body.rating)
    .execute(&state.pool)
    .await?;

    // Async push to AniList (non-blocking)
    let pool = state.pool.clone();
    let rating = body.rating;
    tokio::spawn(async move {
        push_rating_to_anilist(&pool, series_id, user_id, rating).await;
    });

    Ok(StatusCode::NO_CONTENT)
}

// ─── DELETE /series/:series_id/rating ────────────────────────────────────────

#[utoipa::path(
    delete,
    path = "/series/{series_id}/rating",
    tag = "series",
    params(("series_id" = String, Path, description = "Series UUID")),
    responses(
        (status = 204, description = "Rating deleted"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_series_rating(
    State(state): State<AppState>,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let user_id = resolve_user_id(&state).await?;

    sqlx::query(
        "DELETE FROM series_user_ratings WHERE user_id = $1 AND series_id = $2",
    )
    .bind(user_id)
    .bind(series_id)
    .execute(&state.pool)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

// ─── Helper ───────────────────────────────────────────────────────────────────

/// Resolve the local user ID to use for ratings.
/// Uses the AniList local_user_id if configured, otherwise falls back to
/// the first active user in the DB (single-user setup convenience).
async fn resolve_user_id(state: &AppState) -> Result<Uuid, ApiError> {
    // Try AniList local user first
    if let Ok((_, _, Some(uid))) = load_anilist_settings(&state.pool).await {
        return Ok(uid);
    }
    // Fallback: first user in DB
    let uid: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users ORDER BY created_at LIMIT 1")
        .fetch_optional(&state.pool)
        .await?;
    uid.ok_or_else(|| ApiError::internal("No user found"))
}
