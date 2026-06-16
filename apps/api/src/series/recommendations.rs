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
    /// Number of source series to base recommendations on (default 5, max 8)
    #[schema(value_type = Option<i64>, example = 5)]
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
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub genres: Vec<String>,
    pub score: i64,
    /// Raw similarity score (before community bonus)
    pub similarity_score: i64,
    /// Community score bonus (community_score * 10, 0 if no data)
    pub community_bonus: i64,
    /// Community score 0-5 (None if no provider ratings)
    pub community_score: Option<f32>,
    /// Points from shared authors
    pub author_pts: i64,
    /// Points from shared genres
    pub genre_pts: i64,
    /// Points from shared reading lists
    pub reading_list_pts: i64,
    /// Points from shared publisher
    pub publisher_pts: i64,
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
        ("sources" = Option<i64>, Query, description = "Number of recently-read source series to use (default 5, max 8)"),
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
    let n_sources = query.sources.unwrap_or(5).clamp(1, 8);
    let limit = query.limit.unwrap_or(20).clamp(1, 50);

    // If no authenticated user, return empty (recommendations are personal)
    let Some(uid) = user_id else {
        return Ok(Json(vec![]));
    };

    let rows = sqlx::query(
        r#"
        -- Step 1: source series from recent reading, with recency and engagement weights
        WITH source_base AS (
            SELECT
                s.id AS series_id,
                s.name AS series_name,
                COALESCE(s.authors, ARRAY[]::text[]) AS authors,
                COALESCE(s.genres, ARRAY[]::text[]) AS genres,
                COALESCE(s.publishers, ARRAY[]::text[]) AS publishers,
                MAX(brp.last_read_at) AS last_read_at,
                COUNT(*) FILTER (WHERE brp.status = 'read')::bigint AS read_books,
                COUNT(*) FILTER (WHERE brp.status = 'reading')::bigint AS reading_books,
                COUNT(*)::bigint AS touched_books
            FROM book_reading_progress brp
            JOIN books b ON b.id = brp.book_id
            JOIN series s ON s.id = b.series_id
            WHERE brp.user_id = $1
              AND brp.status IN ('read', 'reading')
            GROUP BY s.id, s.name, s.authors, s.genres, s.publishers
        ),
        source_ranked AS (
            SELECT
                sb.*,
                ROW_NUMBER() OVER (
                    ORDER BY sb.last_read_at DESC NULLS LAST, sb.read_books DESC, sb.reading_books DESC, sb.series_name
                ) AS recency_rank
            FROM source_base sb
        ),
        source_series AS (
            SELECT
                sr.*,
                (
                    CASE
                        WHEN sr.read_books > 0 THEN 6
                        WHEN sr.reading_books > 0 THEN 4
                        ELSE 2
                    END
                    + GREATEST($2 - sr.recency_rank, 0)
                )::bigint AS source_weight
            FROM source_ranked sr
            WHERE sr.recency_rank <= $2
        ),
        -- Step 2: series the user has already touched (any reading progress)
        started AS (
            SELECT DISTINCT b.series_id
            FROM book_reading_progress brp
            JOIN books b ON b.id = brp.book_id
            WHERE brp.user_id = $1
        ),
        genre_frequency AS (
            SELECT g.genre, COUNT(DISTINCT g.series_id)::bigint AS series_count
            FROM (
                SELECT s.id AS series_id, unnest(COALESCE(s.genres, ARRAY[]::text[])) AS genre
                FROM series s
            ) g
            GROUP BY g.genre
        ),
        -- Pre-compute reading list co-occurrences once (source → candidate via shared list)
        rl_cooccurrences AS (
            SELECT DISTINCT rli1.series_id AS source_series_id, rli2.series_id AS candidate_id
            FROM reading_list_items rli1
            JOIN source_series ss ON ss.series_id = rli1.series_id
            JOIN reading_list_items rli2 ON rli2.list_id = rli1.list_id
            WHERE rli2.series_id NOT IN (SELECT series_id FROM started)
              AND rli2.series_id != rli1.series_id
        ),
        -- Only keep (candidate, source) pairs with actual overlap — uses GIN indexes on authors/genres
        candidate_source_pairs AS (
            SELECT
                cand.id AS candidate_id,
                src.series_id AS source_series_id,
                src.series_name,
                src.authors AS source_authors,
                src.genres AS source_genres,
                src.publishers AS source_publishers,
                src.source_weight
            FROM series cand
            CROSS JOIN source_series src
            WHERE cand.id NOT IN (SELECT series_id FROM started)
              AND cand.id != src.series_id
              AND (
                  COALESCE(cand.authors, '{}') && src.authors
                  OR COALESCE(cand.genres, '{}') && src.genres
                  OR COALESCE(cand.publishers, '{}') && src.publishers
              )
            UNION
            -- Reading list co-occurrences not already captured by structural overlap
            SELECT
                rlc.candidate_id,
                rlc.source_series_id,
                src.series_name,
                src.authors AS source_authors,
                src.genres AS source_genres,
                src.publishers AS source_publishers,
                src.source_weight
            FROM rl_cooccurrences rlc
            JOIN source_series src ON src.series_id = rlc.source_series_id
        ),
        author_matches AS (
            SELECT
                csp.candidate_id,
                csp.source_series_id,
                COUNT(DISTINCT cand_author)::bigint AS shared_author_count
            FROM candidate_source_pairs csp
            JOIN series cand ON cand.id = csp.candidate_id
            JOIN LATERAL unnest(COALESCE(cand.authors, ARRAY[]::text[])) cand_author ON TRUE
            JOIN LATERAL unnest(csp.source_authors) src_author ON cand_author = src_author
            GROUP BY csp.candidate_id, csp.source_series_id
        ),
        genre_matches AS (
            SELECT
                csp.candidate_id,
                csp.source_series_id,
                COUNT(DISTINCT cand_genre)::bigint AS shared_genre_count,
                COALESCE(
                    SUM(
                        CASE
                            WHEN gf.series_count <= 5 THEN 4
                            WHEN gf.series_count <= 15 THEN 3
                            WHEN gf.series_count <= 40 THEN 2
                            ELSE 1
                        END
                    ),
                    0
                )::bigint AS weighted_genre_score
            FROM candidate_source_pairs csp
            JOIN series cand ON cand.id = csp.candidate_id
            JOIN LATERAL unnest(COALESCE(cand.genres, ARRAY[]::text[])) cand_genre ON TRUE
            JOIN LATERAL unnest(csp.source_genres) src_genre ON cand_genre = src_genre
            LEFT JOIN genre_frequency gf ON gf.genre = cand_genre
            GROUP BY csp.candidate_id, csp.source_series_id
        ),
        publisher_matches AS (
            SELECT
                csp.candidate_id,
                csp.source_series_id,
                (COALESCE(cand.publishers, ARRAY[]::text[]) && csp.source_publishers) AS has_same_publisher
            FROM candidate_source_pairs csp
            JOIN series cand ON cand.id = csp.candidate_id
        ),
        -- Replace per-pair EXISTS with a left join to the pre-computed co-occurrences
        reading_list_matches AS (
            SELECT
                csp.candidate_id,
                csp.source_series_id,
                (rlc.candidate_id IS NOT NULL) AS has_same_reading_list
            FROM candidate_source_pairs csp
            LEFT JOIN rl_cooccurrences rlc
                ON rlc.source_series_id = csp.source_series_id
                AND rlc.candidate_id = csp.candidate_id
        ),
        source_candidate_scores AS (
            SELECT
                csp.candidate_id,
                csp.source_series_id,
                csp.series_name,
                csp.source_weight,
                COALESCE(am.shared_author_count, 0) AS shared_author_count,
                COALESCE(gm.shared_genre_count, 0) AS shared_genre_count,
                COALESCE(gm.weighted_genre_score, 0) AS weighted_genre_score,
                COALESCE(pm.has_same_publisher, FALSE) AS has_same_publisher,
                COALESCE(rlm.has_same_reading_list, FALSE) AS has_same_reading_list,
                (
                    (
                        COALESCE(am.shared_author_count, 0) * 8 +
                        COALESCE(gm.weighted_genre_score, 0) * 2 +
                        CASE WHEN COALESCE(pm.has_same_publisher, FALSE) THEN 1 ELSE 0 END +
                        CASE WHEN COALESCE(rlm.has_same_reading_list, FALSE) THEN 10 ELSE 0 END +
                        CASE
                            WHEN COALESCE(rlm.has_same_reading_list, FALSE) AND COALESCE(am.shared_author_count, 0) > 0 THEN 8
                            WHEN COALESCE(am.shared_author_count, 0) > 0 AND COALESCE(gm.shared_genre_count, 0) > 0 THEN 6
                            WHEN COALESCE(rlm.has_same_reading_list, FALSE) AND COALESCE(gm.shared_genre_count, 0) > 0 THEN 5
                            WHEN COALESCE(am.shared_author_count, 0) > 1 THEN 4
                            WHEN COALESCE(gm.shared_genre_count, 0) > 1 THEN 3
                            WHEN COALESCE(am.shared_author_count, 0) > 0 AND COALESCE(pm.has_same_publisher, FALSE) THEN 2
                            ELSE 0
                        END
                    ) * csp.source_weight
                )::bigint AS pair_score
            FROM candidate_source_pairs csp
            LEFT JOIN author_matches am
                ON am.candidate_id = csp.candidate_id AND am.source_series_id = csp.source_series_id
            LEFT JOIN genre_matches gm
                ON gm.candidate_id = csp.candidate_id AND gm.source_series_id = csp.source_series_id
            LEFT JOIN publisher_matches pm
                ON pm.candidate_id = csp.candidate_id AND pm.source_series_id = csp.source_series_id
            LEFT JOIN reading_list_matches rlm
                ON rlm.candidate_id = csp.candidate_id AND rlm.source_series_id = csp.source_series_id
            WHERE
                COALESCE(rlm.has_same_reading_list, FALSE)
                OR COALESCE(am.shared_author_count, 0) > 0
                OR COALESCE(gm.shared_genre_count, 0) > 0
        ),
        community_scores AS (
            SELECT series_id,
                   (AVG(provider_rating / COALESCE(NULLIF(provider_rating_scale, 0), 10.0) * 5.0))::real AS community_score
            FROM external_metadata_links
            WHERE status = 'approved' AND provider_rating IS NOT NULL AND provider_rating > 0
            GROUP BY series_id
        ),
        candidate_scores AS (
            SELECT
                scs.candidate_id AS series_id,
                (SUM(scs.pair_score) + COALESCE(ROUND(comm.community_score * 10)::bigint, 0))::bigint AS score,
                SUM(scs.pair_score)::bigint AS similarity_score,
                COALESCE(ROUND(comm.community_score * 10)::bigint, 0) AS community_bonus,
                comm.community_score::real AS community_score,
                SUM(scs.shared_author_count * 8 * scs.source_weight)::bigint AS author_pts,
                SUM(scs.weighted_genre_score * 2 * scs.source_weight)::bigint AS genre_pts,
                SUM(CASE WHEN scs.has_same_reading_list THEN 10 * scs.source_weight ELSE 0 END)::bigint AS reading_list_pts,
                SUM(CASE WHEN scs.has_same_publisher THEN scs.source_weight ELSE 0 END)::bigint AS publisher_pts,
                COUNT(*)::bigint AS matched_source_count,
                array_agg(DISTINCT scs.series_name) AS source_names,
                BOOL_OR(scs.has_same_reading_list) AS has_same_reading_list,
                BOOL_OR(scs.shared_author_count > 0) AS has_same_author,
                BOOL_OR(scs.shared_genre_count > 0) AS has_same_genre,
                BOOL_OR(scs.has_same_publisher) AS has_same_publisher
            FROM source_candidate_scores scs
            LEFT JOIN community_scores comm ON comm.series_id = scs.candidate_id
            GROUP BY scs.candidate_id, comm.community_score
        )
        SELECT
            s.id AS series_id,
            s.name,
            s.library_id,
            s.status AS series_status,
            s.cover_url,
            s.description,
            COALESCE(s.authors, ARRAY[]::text[]) AS authors,
            COALESCE(s.genres, ARRAY[]::text[]) AS genres,
            COALESCE(bc.book_count, 0) AS book_count,
            fb.first_book_id,
            fb.first_book_updated_at,
            ml.provider AS metadata_provider,
            cs.score,
            cs.similarity_score,
            cs.community_bonus,
            cs.community_score,
            cs.author_pts,
            cs.genre_pts,
            cs.reading_list_pts,
            cs.publisher_pts,
            cs.source_names,
            cs.has_same_author,
            cs.has_same_genre,
            cs.has_same_publisher,
            cs.has_same_reading_list,
            cs.matched_source_count
        FROM series s
        JOIN candidate_scores cs ON cs.series_id = s.id
        LEFT JOIN LATERAL (
            SELECT b.id AS first_book_id, b.updated_at AS first_book_updated_at
            FROM books b
            WHERE b.series_id = s.id
            ORDER BY CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END, b.volume NULLS LAST, b.created_at ASC
            LIMIT 1
        ) fb ON TRUE
        LEFT JOIN LATERAL (
            SELECT eml.provider FROM external_metadata_links eml
            WHERE eml.series_id = s.id AND eml.status = 'approved'
            ORDER BY eml.created_at DESC LIMIT 1
        ) ml ON TRUE
        LEFT JOIN LATERAL (
            SELECT COUNT(*) AS book_count FROM books b WHERE b.series_id = s.id
        ) bc ON TRUE
        WHERE COALESCE(bc.book_count, 0) > 0
          AND NOT EXISTS (
              SELECT 1 FROM user_genre_restrictions ugr
              WHERE ugr.user_id = $1 AND ugr.genre = ANY(s.genres)
            )
        ORDER BY cs.score DESC, cs.matched_source_count DESC, COALESCE(bc.book_count, 0) DESC, s.name ASC
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
                description: row.get("description"),
                authors: row.try_get::<Vec<String>, _>("authors").unwrap_or_default(),
                genres: row.try_get::<Vec<String>, _>("genres").unwrap_or_default(),
                score: row.get("score"),
                similarity_score: row.get("similarity_score"),
                community_bonus: row.get("community_bonus"),
                community_score: row.get("community_score"),
                author_pts: row.get("author_pts"),
                genre_pts: row.get("genre_pts"),
                reading_list_pts: row.get("reading_list_pts"),
                publisher_pts: row.get("publisher_pts"),
                because_of,
                match_reasons,
            }
        })
        .collect();

    Ok(Json(items))
}
