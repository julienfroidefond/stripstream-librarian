use axum::extract::{Query, State};
use axum::Extension;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::{error::ApiError, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct RecommendationsQuery {
    /// Number of source series to base recommendations on (default 3, max 5)
    #[schema(value_type = Option<i64>, example = 3)]
    pub sources: Option<i64>,
    /// Max recommendations to return (default 20, max 50)
    #[schema(value_type = Option<i64>, example = 20)]
    pub limit: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct RecommendedSeriesItem {
    #[schema(value_type = String)]
    pub series_id: Uuid,
    pub name: String,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub series_status: Option<String>,
    pub cover_url: Option<String>,
    pub book_count: i64,
    #[schema(value_type = Option<String>)]
    pub first_book_id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub first_book_updated_at: Option<DateTime<Utc>>,
    pub metadata_provider: Option<String>,
    pub score: i64,
    /// Names of the recently-read series that triggered this recommendation
    pub because_of: Vec<String>,
    /// Match reasons: "same_genre", "same_author", "same_publisher"
    pub match_reasons: Vec<String>,
}

/// Get personalised series recommendations based on the user's recent reading history
#[utoipa::path(
    get,
    path = "/series/recommendations",
    tag = "series",
    params(
        ("sources" = Option<i64>, Query, description = "Number of recently-read source series to use (default 3, max 5)"),
        ("limit" = Option<i64>, Query, description = "Max recommendations to return (default 20, max 50)"),
    ),
    responses(
        (status = 200, body = Vec<RecommendedSeriesItem>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_recommendations(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Query(query): Query<RecommendationsQuery>,
) -> Result<Json<Vec<RecommendedSeriesItem>>, ApiError> {
    let user_id: Option<Uuid> = user.map(|u| u.0.user_id);
    let n_sources = query.sources.unwrap_or(3).clamp(1, 5);
    let limit = query.limit.unwrap_or(20).clamp(1, 50);

    // If no authenticated user, return empty (recommendations are personal)
    let Some(uid) = user_id else {
        return Ok(Json(vec![]));
    };

    let rows = sqlx::query(
        r#"
        -- Step 1: last N series the user has read/is reading, ordered by recency
        WITH source_series AS (
            SELECT
                s.id          AS series_id,
                s.name        AS series_name,
                s.authors     AS authors,
                s.genres      AS genres,
                s.publishers  AS publishers,
                MAX(brp.last_read_at) AS last_read_at
            FROM book_reading_progress brp
            JOIN books b ON b.id = brp.book_id
            JOIN series s ON s.id = b.series_id
            WHERE brp.user_id = $1
              AND brp.status IN ('read', 'reading')
            GROUP BY s.id, s.name, s.authors, s.genres, s.publishers
            ORDER BY last_read_at DESC NULLS LAST
            LIMIT $2
        ),
        -- Step 2: aggregate all genres, authors, publishers from source series
        source_attrs AS (
            SELECT
                array_agg(DISTINCT g) FILTER (WHERE g IS NOT NULL)  AS all_genres,
                array_agg(DISTINCT a) FILTER (WHERE a IS NOT NULL)  AS all_authors,
                array_agg(DISTINCT p) FILTER (WHERE p IS NOT NULL)  AS all_publishers
            FROM source_series ss
            LEFT JOIN LATERAL unnest(ss.genres)     AS g ON TRUE
            LEFT JOIN LATERAL unnest(ss.authors)    AS a ON TRUE
            LEFT JOIN LATERAL unnest(ss.publishers) AS p ON TRUE
        ),
        -- Step 3: series the user has already touched (any reading progress)
        started AS (
            SELECT DISTINCT b.series_id
            FROM book_reading_progress brp
            JOIN books b ON b.id = brp.book_id
            WHERE brp.user_id = $1
        ),
        -- Step 4: score every other series
        genre_scores AS (
            SELECT s.id AS series_id, COUNT(*)::bigint AS cnt
            FROM series s
            CROSS JOIN source_attrs sa
            JOIN LATERAL unnest(s.genres) sg ON TRUE
            JOIN LATERAL unnest(sa.all_genres) rg ON sg = rg
            WHERE s.id NOT IN (SELECT series_id FROM started)
            GROUP BY s.id
        ),
        author_scores AS (
            SELECT s.id AS series_id, COUNT(*)::bigint AS cnt
            FROM series s
            CROSS JOIN source_attrs sa
            JOIN LATERAL unnest(s.authors) sa2 ON TRUE
            JOIN LATERAL unnest(sa.all_authors) ra ON sa2 = ra
            WHERE s.id NOT IN (SELECT series_id FROM started)
            GROUP BY s.id
        ),
        publisher_scores AS (
            SELECT s.id AS series_id, 1::bigint AS cnt
            FROM series s
            CROSS JOIN source_attrs sa
            WHERE s.id NOT IN (SELECT series_id FROM started)
              AND s.publishers && sa.all_publishers
        ),
        reading_list_scores AS (
            SELECT rli2.series_id, COUNT(*)::bigint AS cnt
            FROM source_series src
            JOIN reading_list_items rli1 ON rli1.series_id = src.series_id
            JOIN reading_list_items rli2 ON rli2.list_id = rli1.list_id
            WHERE rli2.series_id NOT IN (SELECT series_id FROM started)
              AND rli2.series_id != src.series_id
            GROUP BY rli2.series_id
        ),
        -- Step 5: for each candidate, which source series match?
        because_of AS (
            SELECT
                cand.id AS series_id,
                array_agg(DISTINCT ss.series_name) AS source_names
            FROM series cand
            JOIN source_series ss ON (
                cand.authors    && ss.authors    OR
                cand.genres     && ss.genres     OR
                cand.publishers && ss.publishers OR
                EXISTS (
                    SELECT 1 FROM reading_list_items rli1
                    JOIN reading_list_items rli2 ON rli2.list_id = rli1.list_id
                    WHERE rli1.series_id = ss.series_id AND rli2.series_id = cand.id
                )
            )
            WHERE cand.id NOT IN (SELECT series_id FROM started)
            GROUP BY cand.id
        ),
        first_books AS (
            SELECT DISTINCT ON (series_id)
                series_id, id AS first_book_id, updated_at AS first_book_updated_at
            FROM books
            ORDER BY series_id,
                CASE WHEN volume_type = 'regular' THEN 0 ELSE 1 END,
                volume ASC NULLS LAST,
                created_at ASC
        ),
        meta_links AS (
            SELECT DISTINCT ON (series_id) series_id, provider
            FROM external_metadata_links
            WHERE status = 'approved'
            ORDER BY series_id, created_at DESC
        ),
        book_counts AS (
            SELECT series_id, COUNT(*) AS book_count
            FROM books
            GROUP BY series_id
        )
        SELECT
            s.id           AS series_id,
            s.name,
            s.library_id,
            s.status       AS series_status,
            s.cover_url,
            COALESCE(bc.book_count, 0) AS book_count,
            fb.first_book_id,
            fb.first_book_updated_at,
            ml.provider    AS metadata_provider,
            (
                COALESCE(gs.cnt, 0) * 2 +
                COALESCE(as_.cnt, 0) * 3 +
                COALESCE(ps.cnt, 0) * 1 +
                COALESCE(rls.cnt, 0) * 5
            )              AS score,
            bo.source_names,
            (as_.cnt IS NOT NULL)   AS has_same_author,
            (gs.cnt IS NOT NULL)    AS has_same_genre,
            (ps.cnt IS NOT NULL)    AS has_same_publisher,
            (rls.cnt IS NOT NULL)   AS has_same_reading_list
        FROM series s
        LEFT JOIN genre_scores      gs  ON gs.series_id  = s.id
        LEFT JOIN author_scores     as_ ON as_.series_id = s.id
        LEFT JOIN publisher_scores  ps  ON ps.series_id  = s.id
        LEFT JOIN reading_list_scores rls ON rls.series_id = s.id
        LEFT JOIN because_of        bo  ON bo.series_id  = s.id
        LEFT JOIN first_books       fb  ON fb.series_id  = s.id
        LEFT JOIN meta_links        ml  ON ml.series_id  = s.id
        LEFT JOIN book_counts       bc  ON bc.series_id  = s.id
        WHERE (bo.series_id IS NOT NULL OR rls.cnt IS NOT NULL)  -- must match at least one source
          AND COALESCE(bc.book_count, 0) > 0
        ORDER BY score DESC, COALESCE(bc.book_count, 0) DESC
        LIMIT $3
        "#,
    )
    .bind(uid)
    .bind(n_sources)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| {
            let has_same_reading_list: bool = row.get("has_same_reading_list");
            let has_same_author: bool = row.get("has_same_author");
            let has_same_genre: bool = row.get("has_same_genre");
            let has_same_publisher: bool = row.get("has_same_publisher");
            let mut match_reasons = Vec::new();
            if has_same_reading_list { match_reasons.push("same_reading_list".to_string()); }
            if has_same_author { match_reasons.push("same_author".to_string()); }
            if has_same_genre { match_reasons.push("same_genre".to_string()); }
            if has_same_publisher { match_reasons.push("same_publisher".to_string()); }

            let because_of: Vec<String> = row
                .try_get::<Vec<String>, _>("source_names")
                .unwrap_or_default();

            RecommendedSeriesItem {
                series_id: row.get("series_id"),
                name: row.get("name"),
                library_id: row.get("library_id"),
                series_status: row.get("series_status"),
                cover_url: row.get("cover_url"),
                book_count: row.get("book_count"),
                first_book_id: row.get("first_book_id"),
                first_book_updated_at: row.get("first_book_updated_at"),
                metadata_provider: row.get("metadata_provider"),
                score: row.get("score"),
                because_of,
                match_reasons,
            }
        })
        .collect();

    Ok(Json(items))
}
