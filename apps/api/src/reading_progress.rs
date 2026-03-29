use axum::{extract::{Extension, Path, State}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{auth::AuthUser, error::ApiError, state::AppState};

#[derive(Serialize, ToSchema)]
pub struct ReadingProgressResponse {
    /// Reading status: "unread", "reading", or "read"
    pub status: String,
    /// Current page (only set when status is "reading")
    pub current_page: Option<i32>,
    #[schema(value_type = Option<String>)]
    pub last_read_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateReadingProgressRequest {
    /// Reading status: "unread", "reading", or "read"
    pub status: String,
    /// Required when status is "reading", must be > 0
    pub current_page: Option<i32>,
}

/// Get reading progress for a book
#[utoipa::path(
    get,
    path = "/books/{id}/progress",
    tag = "reading-progress",
    params(
        ("id" = String, Path, description = "Book UUID"),
    ),
    responses(
        (status = 200, body = ReadingProgressResponse),
        (status = 404, description = "Book not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_reading_progress(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReadingProgressResponse>, ApiError> {
    let auth_user = user.ok_or_else(|| ApiError::bad_request("admin tokens cannot track reading progress"))?.0;
    // Verify book exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM books WHERE id = $1)")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;

    if !exists {
        return Err(ApiError::not_found("book not found"));
    }

    let row = sqlx::query(
        "SELECT status, current_page, last_read_at FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth_user.user_id)
    .fetch_optional(&state.pool)
    .await?;

    let response = match row {
        Some(r) => ReadingProgressResponse {
            status: r.get("status"),
            current_page: r.get("current_page"),
            last_read_at: r.get("last_read_at"),
        },
        None => ReadingProgressResponse {
            status: "unread".to_string(),
            current_page: None,
            last_read_at: None,
        },
    };

    Ok(Json(response))
}

/// Update reading progress for a book
#[utoipa::path(
    patch,
    path = "/books/{id}/progress",
    tag = "reading-progress",
    params(
        ("id" = String, Path, description = "Book UUID"),
    ),
    request_body = UpdateReadingProgressRequest,
    responses(
        (status = 200, body = ReadingProgressResponse),
        (status = 404, description = "Book not found"),
        (status = 422, description = "Validation error (missing or invalid current_page for status 'reading')"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_reading_progress(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateReadingProgressRequest>,
) -> Result<Json<ReadingProgressResponse>, ApiError> {
    let auth_user = user.ok_or_else(|| ApiError::bad_request("admin tokens cannot track reading progress"))?.0;
    // Validate status value
    if !["unread", "reading", "read"].contains(&body.status.as_str()) {
        return Err(ApiError::bad_request(format!(
            "invalid status '{}': must be one of unread, reading, read",
            body.status
        )));
    }

    // Validate current_page for "reading" status
    if body.status == "reading" {
        match body.current_page {
            None => {
                return Err(ApiError::unprocessable_entity(
                    "current_page is required when status is 'reading'",
                ))
            }
            Some(p) if p <= 0 => {
                return Err(ApiError::unprocessable_entity(
                    "current_page must be greater than 0",
                ))
            }
            _ => {}
        }
    }

    // Verify book exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM books WHERE id = $1)")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;

    if !exists {
        return Err(ApiError::not_found("book not found"));
    }

    // current_page is only stored for "reading" status
    let current_page = if body.status == "reading" {
        body.current_page
    } else {
        None
    };

    let row = sqlx::query(
        r#"
        INSERT INTO book_reading_progress (book_id, user_id, status, current_page, last_read_at, updated_at)
        VALUES ($1, $2, $3, $4, NOW(), NOW())
        ON CONFLICT (book_id, user_id) DO UPDATE
          SET status       = EXCLUDED.status,
              current_page = EXCLUDED.current_page,
              last_read_at = NOW(),
              updated_at   = NOW()
        RETURNING status, current_page, last_read_at
        "#,
    )
    .bind(id)
    .bind(auth_user.user_id)
    .bind(&body.status)
    .bind(current_page)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(ReadingProgressResponse {
        status: row.get("status"),
        current_page: row.get("current_page"),
        last_read_at: row.get("last_read_at"),
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct MarkSeriesReadRequest {
    /// Series name (use "unclassified" for books without series)
    pub series: String,
    /// Status to set: "read" or "unread"
    pub status: String,
}

#[derive(Serialize, ToSchema)]
pub struct MarkSeriesReadResponse {
    pub updated: i64,
}

/// Mark all books in a series as read or unread
#[utoipa::path(
    post,
    path = "/series/mark-read",
    tag = "reading-progress",
    request_body = MarkSeriesReadRequest,
    responses(
        (status = 200, body = MarkSeriesReadResponse),
        (status = 422, description = "Invalid status"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn mark_series_read(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Json(body): Json<MarkSeriesReadRequest>,
) -> Result<Json<MarkSeriesReadResponse>, ApiError> {
    let auth_user = user.ok_or_else(|| ApiError::bad_request("admin tokens cannot track reading progress"))?.0;
    if !["read", "unread"].contains(&body.status.as_str()) {
        return Err(ApiError::bad_request(
            "status must be 'read' or 'unread'",
        ));
    }

    let series_filter = if body.series == "unclassified" {
        "series_id IS NULL"
    } else {
        "series_id = $1"
    };

    let sql = if body.status == "unread" {
        // Delete progress records to reset to unread (scoped to this user)
        if body.series == "unclassified" {
            format!(
                r#"
                WITH target_books AS (
                    SELECT id FROM books WHERE {series_filter}
                )
                DELETE FROM book_reading_progress
                WHERE book_id IN (SELECT id FROM target_books) AND user_id = $1
                "#
            )
        } else {
            format!(
                r#"
                WITH target_books AS (
                    SELECT id FROM books WHERE {series_filter}
                )
                DELETE FROM book_reading_progress
                WHERE book_id IN (SELECT id FROM target_books) AND user_id = $2
                "#
            )
        }
    } else if body.series == "unclassified" {
        format!(
            r#"
            INSERT INTO book_reading_progress (book_id, user_id, status, current_page, last_read_at, updated_at)
            SELECT id, $1, 'read', NULL, NOW(), NOW()
            FROM books
            WHERE {series_filter}
            ON CONFLICT (book_id, user_id) DO UPDATE
              SET status = 'read',
                  current_page = NULL,
                  last_read_at = NOW(),
                  updated_at = NOW()
            "#
        )
    } else {
        format!(
            r#"
            INSERT INTO book_reading_progress (book_id, user_id, status, current_page, last_read_at, updated_at)
            SELECT id, $2, 'read', NULL, NOW(), NOW()
            FROM books
            WHERE {series_filter}
            ON CONFLICT (book_id, user_id) DO UPDATE
              SET status = 'read',
                  current_page = NULL,
                  last_read_at = NOW(),
                  updated_at = NOW()
            "#
        )
    };

    let result = if body.series == "unclassified" {
        // $1 = user_id (no series bind needed)
        sqlx::query(&sql)
            .bind(auth_user.user_id)
            .execute(&state.pool)
            .await?
    } else {
        // $1 = series_id (UUID), $2 = user_id
        let series_uuid: Uuid = body.series.parse().map_err(|_| ApiError::bad_request("invalid series id"))?;
        sqlx::query(&sql)
            .bind(series_uuid)
            .bind(auth_user.user_id)
            .execute(&state.pool)
            .await?
    };

    Ok(Json(MarkSeriesReadResponse {
        updated: result.rows_affected() as i64,
    }))
}
