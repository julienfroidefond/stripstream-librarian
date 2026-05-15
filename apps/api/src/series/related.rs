use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct RelatedSeriesQuery {
    #[schema(value_type = Option<i64>, example = 10)]
    pub limit: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct RelatedSeriesItem {
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
    pub metadata_provider: Option<String>,
    pub cover_url: Option<String>,
    pub score: i64,
    /// Reasons for the match: "same_author", "same_genre", "same_publisher"
    pub match_reasons: Vec<String>,
}

/// Get related series (same author, genre, or publisher) ordered by weighted score
#[utoipa::path(
    get,
    path = "/series/{series_id}/related",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
        ("limit" = Option<i64>, Query, description = "Max results (default 10, max 50)"),
    ),
    responses(
        (status = 200, body = Vec<RelatedSeriesItem>),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_related_series(
    State(state): State<AppState>,
    Path(series_id): Path<Uuid>,
    Query(query): Query<RelatedSeriesQuery>,
) -> Result<Json<Vec<RelatedSeriesItem>>, ApiError> {
    let limit = query.limit.unwrap_or(10).clamp(1, 50);

    // Check series exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
        .bind(series_id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(ApiError::not_found("series not found"));
    }

    let rows = sqlx::query(r#"
        WITH ref AS (
            SELECT authors, genres, publishers
            FROM series
            WHERE id = $1
        ),
        author_scores AS (
            SELECT s.id AS series_id, COUNT(*)::bigint AS cnt
            FROM series s, ref, unnest(s.authors) sa, unnest(ref.authors) ra
            WHERE s.id != $1 AND sa = ra
            GROUP BY s.id
        ),
        genre_scores AS (
            SELECT s.id AS series_id, COUNT(*)::bigint AS cnt
            FROM series s, ref, unnest(s.genres) sg, unnest(ref.genres) rg
            WHERE s.id != $1 AND sg = rg
            GROUP BY s.id
        ),
        book_counts AS (
            SELECT series_id, COUNT(*) AS book_count
            FROM books
            GROUP BY series_id
        ),
        first_books AS (
            SELECT DISTINCT ON (series_id)
                series_id, id AS first_book_id, updated_at AS first_book_updated_at
            FROM books
            ORDER BY series_id, volume ASC NULLS LAST, created_at ASC
        ),
        meta_links AS (
            SELECT DISTINCT ON (series_id) series_id, provider
            FROM external_metadata_links
            WHERE status = 'approved'
            ORDER BY series_id, created_at DESC
        )
        SELECT
            s.id AS series_id,
            s.name,
            s.library_id,
            s.status AS series_status,
            s.cover_url,
            COALESCE(bc.book_count, 0) AS book_count,
            fb.first_book_id,
            fb.first_book_updated_at,
            ml.provider AS metadata_provider,
            (
                COALESCE(asc_.cnt, 0) * 3 +
                COALESCE(gsc_.cnt, 0) * 2 +
                CASE WHEN s.publishers && ref.publishers THEN 1 ELSE 0 END
            ) AS score,
            (asc_.cnt IS NOT NULL) AS has_same_author,
            (gsc_.cnt IS NOT NULL) AS has_same_genre,
            (s.publishers && ref.publishers) AS has_same_publisher
        FROM series s
        CROSS JOIN ref
        LEFT JOIN author_scores asc_ ON asc_.series_id = s.id
        LEFT JOIN genre_scores gsc_ ON gsc_.series_id = s.id
        LEFT JOIN book_counts bc ON bc.series_id = s.id
        LEFT JOIN first_books fb ON fb.series_id = s.id
        LEFT JOIN meta_links ml ON ml.series_id = s.id
        WHERE s.id != $1
          AND (
            s.authors && ref.authors
            OR s.genres && ref.genres
            OR s.publishers && ref.publishers
          )
        ORDER BY score DESC, COALESCE(bc.book_count, 0) DESC
        LIMIT $2
    "#)
    .bind(series_id)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    let items = rows.into_iter().map(|row| {
        let has_same_author: bool = row.get("has_same_author");
        let has_same_genre: bool = row.get("has_same_genre");
        let has_same_publisher: bool = row.get("has_same_publisher");
        let mut match_reasons = Vec::new();
        if has_same_author { match_reasons.push("same_author".to_string()); }
        if has_same_genre { match_reasons.push("same_genre".to_string()); }
        if has_same_publisher { match_reasons.push("same_publisher".to_string()); }

        RelatedSeriesItem {
            series_id: row.get("series_id"),
            name: row.get("name"),
            library_id: row.get("library_id"),
            series_status: row.get("series_status"),
            cover_url: row.get("cover_url"),
            book_count: row.get("book_count"),
            books_read_count: 0,
            first_book_id: row.get("first_book_id"),
            first_book_updated_at: row.get("first_book_updated_at"),
            metadata_provider: row.get("metadata_provider"),
            score: row.get("score"),
            match_reasons,
        }
    }).collect();

    Ok(Json(items))
}
