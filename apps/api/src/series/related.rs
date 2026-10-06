use axum::extract::{Extension, Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{auth::AuthUser, error::ApiError, state::AppState};

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
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
    Query(query): Query<RelatedSeriesQuery>,
) -> Result<Json<Vec<RelatedSeriesItem>>, ApiError> {
    let user_id: Option<Uuid> = user.map(|u| u.0.user_id);
    let limit = query.limit.unwrap_or(10).clamp(1, 50);

    // Check series exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
        .bind(series_id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(ApiError::not_found("series not found"));
    }

    let rows = sqlx::query(
        r#"
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
        reading_list_scores AS (
            SELECT rli2.series_id, COUNT(*)::bigint AS cnt
            FROM reading_list_items rli1
            JOIN reading_list_items rli2 ON rli2.list_id = rli1.list_id
            WHERE rli1.series_id = $1 AND rli2.series_id != $1
            GROUP BY rli2.series_id
        ),
        candidates AS (
            SELECT s.id, s.authors, s.genres, s.publishers
            FROM series s, ref
            WHERE s.id != $1
              AND (
                s.authors && ref.authors
                OR s.genres && ref.genres
                OR s.publishers && ref.publishers
                OR EXISTS (
                    SELECT 1 FROM reading_list_items rli1
                    JOIN reading_list_items rli2 ON rli2.list_id = rli1.list_id
                    WHERE rli1.series_id = $1 AND rli2.series_id = s.id
                )
              )
              AND ($3::uuid IS NULL OR NOT EXISTS (
                  SELECT 1 FROM user_genre_restrictions ugr
                  WHERE ugr.user_id = $3 AND ugr.genre = ANY(s.genres)
              ))
        ),
        book_counts AS (
            SELECT b.series_id, COUNT(*) AS book_count
            FROM books b
            JOIN candidates c ON c.id = b.series_id
            GROUP BY b.series_id
        ),
        first_books AS (
            SELECT DISTINCT ON (b.series_id)
                b.series_id, b.id AS first_book_id, b.updated_at AS first_book_updated_at
            FROM books b
            JOIN candidates c ON c.id = b.series_id
            ORDER BY b.series_id,
                CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END,
                b.volume ASC NULLS LAST,
                b.created_at ASC
        ),
        meta_links AS (
            SELECT DISTINCT ON (eml.series_id) eml.series_id, eml.provider
            FROM external_metadata_links eml
            JOIN candidates c ON c.id = eml.series_id
            WHERE eml.status = 'approved'
            ORDER BY eml.series_id, eml.is_primary DESC, eml.created_at DESC
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
                CASE WHEN s.publishers && ref.publishers THEN 1 ELSE 0 END +
                COALESCE(rlsc.cnt, 0) * 5
            ) AS score,
            (asc_.cnt IS NOT NULL) AS has_same_author,
            (gsc_.cnt IS NOT NULL) AS has_same_genre,
            (s.publishers && ref.publishers) AS has_same_publisher,
            (rlsc.cnt IS NOT NULL) AS has_same_reading_list
        FROM series s
        CROSS JOIN ref
        LEFT JOIN author_scores asc_ ON asc_.series_id = s.id
        LEFT JOIN genre_scores gsc_ ON gsc_.series_id = s.id
        LEFT JOIN reading_list_scores rlsc ON rlsc.series_id = s.id
        LEFT JOIN book_counts bc ON bc.series_id = s.id
        LEFT JOIN first_books fb ON fb.series_id = s.id
        LEFT JOIN meta_links ml ON ml.series_id = s.id
        WHERE s.id != $1
          AND COALESCE(bc.book_count, 0) > 0
          AND (
            s.authors && ref.authors
            OR s.genres && ref.genres
            OR s.publishers && ref.publishers
            OR rlsc.cnt IS NOT NULL
          )
          AND ($3::uuid IS NULL OR NOT EXISTS (
              SELECT 1 FROM user_genre_restrictions ugr
              WHERE ugr.user_id = $3 AND ugr.genre = ANY(s.genres)
          ))
        ORDER BY score DESC, COALESCE(bc.book_count, 0) DESC
        LIMIT $2
    "#,
    )
    .bind(series_id)
    .bind(limit)
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| {
            let has_same_author: bool = row.get("has_same_author");
            let has_same_genre: bool = row.get("has_same_genre");
            let has_same_publisher: bool = row.get("has_same_publisher");
            let has_same_reading_list: bool = row.get("has_same_reading_list");
            let mut match_reasons = Vec::new();
            if has_same_reading_list {
                match_reasons.push("same_reading_list".to_string());
            }
            if has_same_author {
                match_reasons.push("same_author".to_string());
            }
            if has_same_genre {
                match_reasons.push("same_genre".to_string());
            }
            if has_same_publisher {
                match_reasons.push("same_publisher".to_string());
            }

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
        })
        .collect();

    Ok(Json(items))
}
