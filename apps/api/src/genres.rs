use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

#[derive(Serialize, ToSchema)]
pub struct GenreDto {
    pub name: String,
    pub series_count: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct ListGenresQuery {
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
}

#[derive(Deserialize, ToSchema)]
pub struct RenameGenreRequest {
    pub new_name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct AssignGenreRequest {
    pub genre: String,
    #[schema(value_type = Vec<String>)]
    pub series_ids: Vec<Uuid>,
}

/// List all genres with their series count, optionally filtered by library
#[utoipa::path(
    get,
    path = "/genres",
    tag = "genres",
    params(
        ("library_id" = Option<String>, Query, description = "Filter counts by library UUID"),
    ),
    responses(
        (status = 200, body = Vec<GenreDto>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_genres(
    State(state): State<AppState>,
    Query(query): Query<ListGenresQuery>,
) -> Result<Json<Vec<GenreDto>>, ApiError> {
    let rows = if let Some(lib_id) = query.library_id {
        sqlx::query(
            r#"
            SELECT g, COUNT(s.id)::bigint AS series_count
            FROM series s, unnest(s.genres) g
            WHERE s.library_id = $1
            GROUP BY g
            ORDER BY g
            "#,
        )
        .bind(lib_id)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT g, COUNT(s.id)::bigint AS series_count
            FROM series s, unnest(s.genres) g
            GROUP BY g
            ORDER BY g
            "#,
        )
        .fetch_all(&state.pool)
        .await?
    };

    let genres = rows
        .into_iter()
        .map(|r| GenreDto {
            name: r.get("g"),
            series_count: r.get("series_count"),
        })
        .collect();

    Ok(Json(genres))
}

/// Rename a genre across all series
#[utoipa::path(
    patch,
    path = "/genres/{name}",
    tag = "genres",
    params(("name" = String, Path, description = "Genre name to rename")),
    request_body = RenameGenreRequest,
    responses(
        (status = 200, description = "Renamed"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn rename_genre(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<RenameGenreRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let new_name = body.new_name.trim().to_string();
    if new_name.is_empty() {
        return Err(ApiError::bad_request("genre name cannot be empty"));
    }

    sqlx::query(
        "UPDATE series SET genres = array_replace(genres, $1, $2), updated_at = NOW() WHERE $1 = ANY(genres)",
    )
    .bind(&name)
    .bind(&new_name)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Remove a genre from all series
#[utoipa::path(
    delete,
    path = "/genres/{name}",
    tag = "genres",
    params(("name" = String, Path, description = "Genre name to delete")),
    responses(
        (status = 200, description = "Deleted"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_genre(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    sqlx::query(
        "UPDATE series SET genres = array_remove(genres, $1), updated_at = NOW() WHERE $1 = ANY(genres)",
    )
    .bind(&name)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Assign a genre to a list of series (bulk)
#[utoipa::path(
    post,
    path = "/genres/assign",
    tag = "genres",
    request_body = AssignGenreRequest,
    responses(
        (status = 200, description = "Assigned"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn assign_genre(
    State(state): State<AppState>,
    Json(body): Json<AssignGenreRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let genre = body.genre.trim().to_string();
    if genre.is_empty() {
        return Err(ApiError::bad_request("genre cannot be empty"));
    }
    if body.series_ids.is_empty() {
        return Err(ApiError::bad_request("series_ids cannot be empty"));
    }

    sqlx::query(
        r#"
        UPDATE series
        SET genres = CASE WHEN $1 = ANY(genres) THEN genres ELSE array_append(genres, $1) END,
            updated_at = NOW()
        WHERE id = ANY($2)
        "#,
    )
    .bind(&genre)
    .bind(&body.series_ids)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize, ToSchema)]
pub struct UntaggedSeriesQuery {
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
}

/// List series without any genre (for bulk tagging)
#[utoipa::path(
    get,
    path = "/genres/untagged-series",
    tag = "genres",
    params(("library_id" = Option<String>, Query, description = "Filter by library ID")),
    responses(
        (status = 200, body = Vec<crate::series::SeriesItem>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn untagged_series(
    State(state): State<AppState>,
    Query(query): Query<UntaggedSeriesQuery>,
) -> Result<Json<Vec<crate::series::SeriesItem>>, ApiError> {
    let lib_cond = if query.library_id.is_some() {
        "AND s.library_id = $1"
    } else {
        "AND TRUE"
    };

    let sql = format!(
        r#"
        SELECT
            s.id AS series_id,
            s.name,
            s.library_id,
            s.status AS series_status,
            s.cover_url,
            COUNT(b.id)::bigint AS book_count,
            0::bigint AS books_read_count,
            NULL::bigint AS missing_count,
            NULL::text AS metadata_provider,
            NULL::integer AS anilist_id,
            NULL::text AS anilist_url,
            (SELECT b2.id FROM books b2 WHERE b2.series_id = s.id ORDER BY b2.volume NULLS LAST LIMIT 1) AS first_book_id,
            (SELECT b2.updated_at FROM books b2 WHERE b2.series_id = s.id ORDER BY b2.volume NULLS LAST LIMIT 1) AS first_book_updated_at
        FROM series s
        LEFT JOIN books b ON b.series_id = s.id
        WHERE cardinality(s.genres) = 0 {lib_cond}
        GROUP BY s.id
        ORDER BY LOWER(s.name)
        LIMIT 500
        "#
    );

    let rows = if let Some(lib_id) = query.library_id {
        sqlx::query(&sql).bind(lib_id).fetch_all(&state.pool).await?
    } else {
        sqlx::query(&sql).fetch_all(&state.pool).await?
    };

    use sqlx::Row as _;
    let items = rows
        .into_iter()
        .map(|r| crate::series::SeriesItem {
            series_id: r.get("series_id"),
            name: r.get("name"),
            library_id: r.get("library_id"),
            series_status: r.get("series_status"),
            cover_url: r.get("cover_url"),
            book_count: r.get("book_count"),
            books_read_count: r.get("books_read_count"),
            missing_count: r.get("missing_count"),
            metadata_provider: r.get("metadata_provider"),
            anilist_id: r.get("anilist_id"),
            anilist_url: r.get("anilist_url"),
            first_book_id: r.get("first_book_id"),
            first_book_updated_at: r.get("first_book_updated_at"),
        })
        .collect();

    Ok(Json(items))
}
