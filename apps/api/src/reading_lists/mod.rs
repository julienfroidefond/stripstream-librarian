use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct ReadingListDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub series_count: i64,
    pub preview_covers: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct ReadingListDetailDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub items: Vec<ReadingListSeriesDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct ReadingListSeriesDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    pub cover_url: Option<String>,
    #[schema(value_type = String)]
    pub first_book_id: Option<Uuid>,
    pub first_book_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub provider: Option<String>,
    pub external_id: Option<String>,
    pub external_url: Option<String>,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub library_name: String,
    pub position: i32,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateReadingListRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateReadingListRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct AddSeriesRequest {
    #[schema(value_type = String)]
    pub series_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct ReorderSeriesRequest {
    #[schema(value_type = Vec<String>)]
    pub series_ids: Vec<Uuid>,
}

// ---------------------------------------------------------------------------
// Admin handlers
// ---------------------------------------------------------------------------

/// Create a reading list
#[utoipa::path(
    post,
    path = "/reading-lists",
    tag = "reading-lists",
    request_body = CreateReadingListRequest,
    responses(
        (status = 200, body = ReadingListDto),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn create_reading_list(
    State(state): State<AppState>,
    Json(body): Json<CreateReadingListRequest>,
) -> Result<Json<ReadingListDto>, ApiError> {
    if body.name.trim().is_empty() {
        return Err(ApiError::bad_request("name cannot be empty"));
    }

    let row = sqlx::query(
        r#"
        INSERT INTO reading_lists (name, description)
        VALUES ($1, $2)
        RETURNING id, name, description, created_at, updated_at
        "#,
    )
    .bind(body.name.trim())
    .bind(body.description.as_deref())
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(ReadingListDto {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        series_count: 0,
        preview_covers: vec![],
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }))
}

/// Update a reading list name / description
#[utoipa::path(
    patch,
    path = "/reading-lists/{id}",
    tag = "reading-lists",
    params(("id" = String, Path, description = "Reading list UUID")),
    request_body = UpdateReadingListRequest,
    responses(
        (status = 200, body = ReadingListDto),
        (status = 404, description = "Not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_reading_list(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateReadingListRequest>,
) -> Result<Json<ReadingListDto>, ApiError> {
    let row = sqlx::query(
        r#"
        UPDATE reading_lists
        SET
            name        = COALESCE($2, name),
            description = CASE WHEN $3::boolean THEN $4 ELSE description END,
            updated_at  = NOW()
        WHERE id = $1
        RETURNING id, name, description, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(body.name.as_deref().map(str::trim))
    .bind(body.description.is_some())
    .bind(body.description.as_deref())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("reading list not found"))?;

    let series_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reading_list_items WHERE list_id = $1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(ReadingListDto {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        series_count,
        preview_covers: vec![],
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }))
}

/// Delete a reading list
#[utoipa::path(
    delete,
    path = "/reading-lists/{id}",
    tag = "reading-lists",
    params(("id" = String, Path, description = "Reading list UUID")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_reading_list(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let result = sqlx::query("DELETE FROM reading_lists WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("reading list not found"));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Add a series to a reading list
#[utoipa::path(
    post,
    path = "/reading-lists/{id}/series",
    tag = "reading-lists",
    params(("id" = String, Path, description = "Reading list UUID")),
    request_body = AddSeriesRequest,
    responses(
        (status = 204, description = "Added"),
        (status = 400, description = "Already in list or invalid"),
        (status = 404, description = "Not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn add_series(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddSeriesRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM reading_lists WHERE id = $1)")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;

    if !exists {
        return Err(ApiError::not_found("reading list not found"));
    }

    let max_pos: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(position) FROM reading_list_items WHERE list_id = $1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let next_pos = max_pos.map(|p| p + 1).unwrap_or(0);

    let result = sqlx::query(
        "INSERT INTO reading_list_items (list_id, series_id, position) VALUES ($1, $2, $3) ON CONFLICT (list_id, series_id) DO NOTHING",
    )
    .bind(id)
    .bind(body.series_id)
    .bind(next_pos)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::bad_request("series already in reading list"));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Remove a series from a reading list
#[utoipa::path(
    delete,
    path = "/reading-lists/{id}/series/{series_id}",
    tag = "reading-lists",
    params(
        ("id" = String, Path, description = "Reading list UUID"),
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 204, description = "Removed"),
        (status = 404, description = "Not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn remove_series(
    State(state): State<AppState>,
    Path((id, series_id)): Path<(Uuid, Uuid)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let result = sqlx::query(
        "DELETE FROM reading_list_items WHERE list_id = $1 AND series_id = $2",
    )
    .bind(id)
    .bind(series_id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("series not found in reading list"));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Reorder series in a reading list (full replacement, body = ordered series_ids)
#[utoipa::path(
    put,
    path = "/reading-lists/{id}/series/reorder",
    tag = "reading-lists",
    params(("id" = String, Path, description = "Reading list UUID")),
    request_body = ReorderSeriesRequest,
    responses(
        (status = 204, description = "Reordered"),
        (status = 400, description = "Invalid series IDs"),
        (status = 404, description = "Not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn reorder_series(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ReorderSeriesRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM reading_lists WHERE id = $1)")
            .bind(id)
            .fetch_one(&state.pool)
            .await?;

    if !exists {
        return Err(ApiError::not_found("reading list not found"));
    }

    let mut tx = state.pool.begin().await?;

    for (pos, series_id) in body.series_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE reading_list_items SET position = $3 WHERE list_id = $1 AND series_id = $2",
        )
        .bind(id)
        .bind(series_id)
        .bind(pos as i32)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Shared read helpers
// ---------------------------------------------------------------------------

async fn fetch_list_dto(pool: &sqlx::PgPool, id: Uuid) -> Result<ReadingListDto, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT rl.id, rl.name, rl.description, rl.created_at, rl.updated_at,
               COUNT(rli.id)::bigint AS series_count,
               ARRAY(
                   SELECT COALESCE(fb.id::text, s.cover_url)
                   FROM reading_list_items rli2
                   JOIN series s ON s.id = rli2.series_id
                   LEFT JOIN LATERAL (
                       SELECT b.id FROM books b WHERE b.series_id = s.id
                       ORDER BY CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END, b.volume NULLS LAST
                       LIMIT 1
                   ) fb ON TRUE
                   WHERE rli2.list_id = rl.id
                     AND (fb.id IS NOT NULL OR s.cover_url IS NOT NULL)
                   ORDER BY rli2.position
                   LIMIT 5
               ) AS preview_covers
        FROM reading_lists rl
        LEFT JOIN reading_list_items rli ON rli.list_id = rl.id
        WHERE rl.id = $1
        GROUP BY rl.id
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("reading list not found"))?;

    Ok(ReadingListDto {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        series_count: row.get("series_count"),
        preview_covers: row.get::<Vec<String>, _>("preview_covers"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

async fn fetch_list_items(
    pool: &sqlx::PgPool,
    list_id: Uuid,
) -> Result<Vec<ReadingListSeriesDto>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            s.id           AS series_id,
            s.name         AS series_name,
            s.cover_url,
            l.id           AS library_id,
            l.name         AS library_name,
            rli.position,
            eml.provider,
            eml.external_id,
            eml.external_url,
            (SELECT b.id FROM books b WHERE b.series_id = s.id
             ORDER BY CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END, b.volume NULLS LAST
             LIMIT 1) AS first_book_id,
            (SELECT b.updated_at FROM books b WHERE b.series_id = s.id
             ORDER BY CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END, b.volume NULLS LAST
             LIMIT 1) AS first_book_updated_at
        FROM reading_list_items rli
        JOIN series s ON s.id = rli.series_id
        JOIN libraries l ON l.id = s.library_id
        LEFT JOIN LATERAL (
            SELECT provider, external_id, external_url
            FROM external_metadata_links
            WHERE series_id = s.id
              AND status = 'approved'
            ORDER BY approved_at DESC NULLS LAST
            LIMIT 1
        ) eml ON true
        WHERE rli.list_id = $1
        ORDER BY rli.position, rli.created_at
        "#,
    )
    .bind(list_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ReadingListSeriesDto {
            id: r.get("series_id"),
            name: r.get("series_name"),
            cover_url: r.get("cover_url"),
            first_book_id: r.get("first_book_id"),
            first_book_updated_at: r.get("first_book_updated_at"),
            provider: r.get("provider"),
            external_id: r.get("external_id"),
            external_url: r.get("external_url"),
            library_id: r.get("library_id"),
            library_name: r.get("library_name"),
            position: r.get("position"),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Client + Admin read handlers
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct SeriesMembershipDto {
    #[schema(value_type = String)]
    pub series_id: Uuid,
    #[schema(value_type = String)]
    pub list_id: Uuid,
}

/// Get all series→reading-list memberships (for mixed grid view)
#[utoipa::path(
    get,
    path = "/reading-lists/memberships",
    tag = "reading-lists",
    responses(
        (status = 200, body = Vec<SeriesMembershipDto>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_memberships(
    State(state): State<AppState>,
) -> Result<Json<Vec<SeriesMembershipDto>>, ApiError> {
    let rows = sqlx::query("SELECT series_id, list_id FROM reading_list_items")
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(
        rows.into_iter()
            .map(|r| SeriesMembershipDto {
                series_id: r.get("series_id"),
                list_id: r.get("list_id"),
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct ListReadingListsQuery {
    pub series_id: Option<Uuid>,
}

/// List all reading lists (with series count), optionally filtered by series membership
#[utoipa::path(
    get,
    path = "/reading-lists",
    tag = "reading-lists",
    params(("series_id" = Option<String>, Query, description = "Filter to lists containing this series UUID")),
    responses(
        (status = 200, body = Vec<ReadingListDto>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_reading_lists(
    State(state): State<AppState>,
    Query(query): Query<ListReadingListsQuery>,
) -> Result<Json<Vec<ReadingListDto>>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT rl.id, rl.name, rl.description, rl.created_at, rl.updated_at,
               COUNT(rli.id)::bigint AS series_count,
               ARRAY(
                   SELECT COALESCE(fb.id::text, s.cover_url)
                   FROM reading_list_items rli2
                   JOIN series s ON s.id = rli2.series_id
                   LEFT JOIN LATERAL (
                       SELECT b.id FROM books b WHERE b.series_id = s.id
                       ORDER BY CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END, b.volume NULLS LAST
                       LIMIT 1
                   ) fb ON TRUE
                   WHERE rli2.list_id = rl.id
                     AND (fb.id IS NOT NULL OR s.cover_url IS NOT NULL)
                   ORDER BY rli2.position
                   LIMIT 5
               ) AS preview_covers
        FROM reading_lists rl
        LEFT JOIN reading_list_items rli ON rli.list_id = rl.id
        WHERE ($1::uuid IS NULL OR EXISTS (
            SELECT 1 FROM reading_list_items rli_f
            WHERE rli_f.list_id = rl.id AND rli_f.series_id = $1
        ))
        GROUP BY rl.id
        ORDER BY rl.name
        "#,
    )
    .bind(query.series_id)
    .fetch_all(&state.pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|r| ReadingListDto {
            id: r.get("id"),
            name: r.get("name"),
            description: r.get("description"),
            series_count: r.get("series_count"),
            preview_covers: r.get::<Vec<String>, _>("preview_covers"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

    Ok(Json(items))
}

/// Get a reading list with its ordered series
#[utoipa::path(
    get,
    path = "/reading-lists/{id}",
    tag = "reading-lists",
    params(("id" = String, Path, description = "Reading list UUID")),
    responses(
        (status = 200, body = ReadingListDetailDto),
        (status = 404, description = "Not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_reading_list(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReadingListDetailDto>, ApiError> {
    let dto = fetch_list_dto(&state.pool, id).await?;
    let items = fetch_list_items(&state.pool, id).await?;

    Ok(Json(ReadingListDetailDto {
        id: dto.id,
        name: dto.name,
        description: dto.description,
        items,
        created_at: dto.created_at,
        updated_at: dto.updated_at,
    }))
}
