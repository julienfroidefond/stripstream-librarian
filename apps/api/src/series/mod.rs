pub mod archive;
pub mod create;
pub mod favorites;
pub(crate) mod helpers;
pub mod list;
pub mod merge;
pub mod ongoing;
pub mod ratings;
pub mod recommendations;
pub mod related;
#[cfg(test)]
mod tests;
pub mod update;

pub use archive::*;
pub use create::*;
pub use favorites::*;
pub(crate) use helpers::{get_or_create_series, resolve_library_id};
pub use list::*;
pub use merge::*;
pub use ongoing::*;
pub use ratings::*;
pub use recommendations::*;
pub use related::*;
pub use update::*;

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

// ─── Structs ─────────────────────────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct SeriesLookup {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub struct SeriesItem {
    pub name: String,
    #[schema(value_type = String)]
    pub series_id: Uuid,
    pub book_count: i64,
    pub books_read_count: i64,
    #[schema(value_type = Option<String>)]
    pub first_book_id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub first_book_updated_at: Option<DateTime<Utc>>,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub series_status: Option<String>,
    pub missing_count: Option<i64>,
    pub metadata_provider: Option<String>,
    pub anilist_id: Option<i32>,
    pub anilist_url: Option<String>,
    pub cover_url: Option<String>,
    pub start_year: Option<i32>,
    pub genres: Vec<String>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub user_rating: Option<i16>,
    pub community_score: Option<f32>,
}

#[derive(Serialize, ToSchema)]
pub struct SeriesPage {
    pub items: Vec<SeriesItem>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct ListSeriesQuery {
    #[schema(value_type = Option<String>, example = "dragon")]
    pub q: Option<String>,
    #[schema(value_type = Option<String>, example = "unread,reading")]
    pub reading_status: Option<String>,
    /// Filter by series status (e.g. "ongoing", "ended")
    #[schema(value_type = Option<String>, example = "ongoing")]
    pub series_status: Option<String>,
    /// Filter series with missing books: "true" to show only series with missing books
    #[schema(value_type = Option<String>, example = "true")]
    pub has_missing: Option<String>,
    /// Filter by metadata provider: a provider name (e.g. "google_books"), "linked" (any provider), or "unlinked" (no provider)
    #[schema(value_type = Option<String>, example = "google_books")]
    pub metadata_provider: Option<String>,
    /// Only return series with at least one book: "true" to exclude empty series
    #[schema(value_type = Option<String>, example = "true")]
    pub has_books: Option<String>,
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 50)]
    pub limit: Option<i64>,
    /// Sort order: "title" (default), "latest" (most recently added first), or "release_date" (series start year descending)
    #[schema(value_type = Option<String>, example = "release_date")]
    pub sort: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ListAllSeriesQuery {
    #[schema(value_type = Option<String>, example = "dragon")]
    pub q: Option<String>,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    #[schema(value_type = Option<String>, example = "unread,reading")]
    pub reading_status: Option<String>,
    /// Filter by series status (e.g. "ongoing", "ended")
    #[schema(value_type = Option<String>, example = "ongoing")]
    pub series_status: Option<String>,
    /// Filter series with missing books: "true" to show only series with missing books
    #[schema(value_type = Option<String>, example = "true")]
    pub has_missing: Option<String>,
    /// Filter by metadata provider: a provider name (e.g. "google_books"), "linked" (any provider), or "unlinked" (no provider)
    #[schema(value_type = Option<String>, example = "google_books")]
    pub metadata_provider: Option<String>,
    /// Filter by metadata gap: "no_description", "no_genre", "no_authors", "no_publishers", "no_year", "no_cover", or "no_community_score"
    #[schema(value_type = Option<String>, example = "no_genre")]
    pub gap: Option<String>,
    /// Filter by author name (matches in series.authors or book-level authors)
    #[schema(value_type = Option<String>, example = "Toriyama")]
    pub author: Option<String>,
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 50)]
    pub limit: Option<i64>,
    /// Only return series with at least one book: "true" to exclude empty series
    #[schema(value_type = Option<String>, example = "true")]
    pub has_books: Option<String>,
    /// Only return series with no books (wishlist): "true" to show only empty series
    #[schema(value_type = Option<String>, example = "true")]
    pub no_books: Option<String>,
    /// Sort order: "title" (default), "latest" (most recently added first), or "release_date" (series start year descending)
    #[schema(value_type = Option<String>, example = "release_date")]
    pub sort: Option<String>,
    /// Filter by genre (exact match, case-sensitive)
    #[schema(value_type = Option<String>, example = "Fantasy")]
    pub genre: Option<String>,
    /// Filter series by book volume type: "regular", "oneshot", "hs", "integral"
    #[schema(value_type = Option<String>, example = "oneshot")]
    pub volume_type: Option<String>,
    /// Only return series rated by the current user: "true"
    #[schema(value_type = Option<String>, example = "true")]
    pub rated_only: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct OngoingQuery {
    #[schema(value_type = Option<i64>, example = 10)]
    pub limit: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct SeriesMetadata {
    /// Name of the series
    pub series_name: String,
    /// Authors of the series (series-level metadata, distinct from per-book author field)
    pub authors: Vec<String>,
    pub genres: Vec<String>,
    pub description: Option<String>,
    pub publishers: Vec<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    /// Series status: "ongoing", "ended", "hiatus", "cancelled", or null
    pub status: Option<String>,
    /// Convenience: author from first book (for pre-filling the per-book apply section)
    pub book_author: Option<String>,
    pub book_language: Option<String>,
    /// Fields locked from external metadata sync, e.g. {"authors": true, "description": true}
    pub locked_fields: serde_json::Value,
}

// ─── Lookup by name ──────────────────────────────────────────────────────────

/// Look up a series by name within a library. Returns its UUID and name.
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series/by-name/{name}",
    tag = "series",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("name" = String, Path, description = "Series name (URL-encoded)"),
    ),
    responses(
        (status = 200, body = SeriesLookup),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_by_name(
    State(state): State<AppState>,
    Path((library_id, name)): Path<(Uuid, String)>,
) -> Result<Json<SeriesLookup>, ApiError> {
    let row = sqlx::query(
        "SELECT id, library_id, name FROM series WHERE library_id = $1 AND LOWER(name) = LOWER($2)",
    )
    .bind(library_id)
    .bind(&name)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found(format!("series '{}' not found", name)))?;

    Ok(Json(SeriesLookup {
        id: row.get("id"),
        library_id: row.get("library_id"),
        name: row.get("name"),
    }))
}

// ─── Series statuses ─────────────────────────────────────────────────────────

/// List all distinct genre values present across all series
#[utoipa::path(
    get,
    path = "/series/genres",
    tag = "series",
    responses(
        (status = 200, body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn series_genres(State(state): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT unnest(genres) AS g FROM series WHERE cardinality(genres) > 0 ORDER BY g",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

/// List all distinct series status values present in the database
#[utoipa::path(
    get,
    path = "/series/statuses",
    tag = "series",
    responses(
        (status = 200, body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn series_statuses(State(state): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"SELECT DISTINCT s FROM (
            SELECT LOWER(status) AS s FROM series WHERE status IS NOT NULL
            UNION
            SELECT mapped_status AS s FROM status_mappings WHERE mapped_status IS NOT NULL
        ) t ORDER BY s"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

/// List distinct raw provider statuses from external metadata links
#[utoipa::path(
    get,
    path = "/series/provider-statuses",
    tag = "series",
    responses(
        (status = 200, body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn provider_statuses(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"SELECT DISTINCT lower(metadata_json->>'status') AS s
           FROM external_metadata_links
           WHERE metadata_json->>'status' IS NOT NULL
             AND metadata_json->>'status' != ''
           ORDER BY s"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}
