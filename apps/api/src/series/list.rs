use axum::extract::Extension;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use uuid::Uuid;

use super::catalog::{self, SeriesRequest};
use super::{ListAllSeriesQuery, SeriesPage};
use crate::{auth::AuthUser, error::ApiError, state::AppState};

// ─── List series (per library) ───────────────────────────────────────────────

/// List all series in a library with pagination
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series",
    tag = "series",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("q" = Option<String>, Query, description = "Filter by series name (case- and accent-insensitive, partial match)"),
        ("reading_status" = Option<String>, Query, description = "Filter by reading status, comma-separated (e.g. 'unread,reading')"),
        ("series_status" = Option<String>, Query, description = "Filter by series status (e.g. 'ongoing', 'ended')"),
        ("has_missing" = Option<String>, Query, description = "Only series with missing volumes: 'true'"),
        ("metadata_provider" = Option<String>, Query, description = "Filter by metadata provider: a provider name (e.g. 'google_books'), 'linked' (any provider), or 'unlinked' (no provider)"),
        ("gap" = Option<String>, Query, description = "Filter by metadata gap: 'no_description', 'no_genre', 'no_authors', 'no_publishers', 'no_year', 'no_cover', or 'no_community_score'"),
        ("author" = Option<String>, Query, description = "Filter by author name (matches in series.authors or book-level authors)"),
        ("has_books" = Option<String>, Query, description = "Only series with at least one book: 'true'"),
        ("no_books" = Option<String>, Query, description = "Only series with no books (wishlist): 'true'"),
        ("shelved" = Option<String>, Query, description = "Filter by shelf state for the current user: 'true' for series set aside, 'false' for the others"),
        ("genre" = Option<String>, Query, description = "Filter by genre (exact match, case-sensitive)"),
        ("volume_type" = Option<String>, Query, description = "Filter series by book volume type: 'regular', 'oneshot', 'hs', 'integral'"),
        ("rated_only" = Option<String>, Query, description = "Only series rated by the current user: 'true'"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
        ("sort" = Option<String>, Query, description = "Sort order: 'title' (default), 'latest' (most recently added first), 'release_date' (series start year descending), or 'community_score' (best rated first). Unknown values are rejected with 400"),
    ),
    responses(
        (status = 200, body = SeriesPage),
        (status = 400, description = "Invalid query value"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_series(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(library_id): Path<Uuid>,
    Query(query): Query<ListAllSeriesQuery>,
) -> Result<Json<SeriesPage>, ApiError> {
    let request = SeriesRequest::from_list_library(query, library_id, user.map(|u| u.0.user_id))?;

    Ok(Json(catalog::query(&state.pool, request).await?.into()))
}

// ─── List all series (cross-library) ─────────────────────────────────────────

/// List all series across libraries with optional filtering and pagination
#[utoipa::path(
    get,
    path = "/series",
    tag = "series",
    params(
        ("q" = Option<String>, Query, description = "Filter by series name (case- and accent-insensitive, partial match)"),
        ("library_id" = Option<String>, Query, description = "Filter by library ID"),
        ("reading_status" = Option<String>, Query, description = "Filter by reading status, comma-separated (e.g. 'unread,reading')"),
        ("series_status" = Option<String>, Query, description = "Filter by series status (e.g. 'ongoing', 'ended')"),
        ("has_missing" = Option<String>, Query, description = "Only series with missing volumes: 'true'"),
        ("metadata_provider" = Option<String>, Query, description = "Filter by metadata provider: a provider name (e.g. 'google_books'), 'linked' (any provider), or 'unlinked' (no provider)"),
        ("shelved" = Option<String>, Query, description = "Filter by shelf state for the current user: 'true' for series set aside, 'false' for the others"),
        ("gap" = Option<String>, Query, description = "Filter by metadata gap: 'no_description', 'no_genre', 'no_authors', 'no_publishers', 'no_year', 'no_cover', or 'no_community_score'"),
        ("author" = Option<String>, Query, description = "Filter by author name (matches in series.authors or book-level authors)"),
        ("has_books" = Option<String>, Query, description = "Only series with at least one book: 'true'"),
        ("no_books" = Option<String>, Query, description = "Only series with no books (wishlist): 'true'"),
        ("genre" = Option<String>, Query, description = "Filter by genre (exact match, case-sensitive)"),
        ("volume_type" = Option<String>, Query, description = "Filter series by book volume type: 'regular', 'oneshot', 'hs', 'integral'"),
        ("rated_only" = Option<String>, Query, description = "Only series rated by the current user: 'true'"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
        ("sort" = Option<String>, Query, description = "Sort order: 'title' (default), 'latest' (most recently added first), 'release_date' (series start year descending; undated series last by title), or 'community_score' (best rated first). Unknown values are rejected with 400"),
    ),
    responses(
        (status = 200, body = SeriesPage),
        (status = 400, description = "Invalid query value"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_all_series(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Query(query): Query<ListAllSeriesQuery>,
) -> Result<Json<SeriesPage>, ApiError> {
    let request = SeriesRequest::from_list_all(query, user.map(|u| u.0.user_id))?;

    Ok(Json(catalog::query(&state.pool, request).await?.into()))
}
