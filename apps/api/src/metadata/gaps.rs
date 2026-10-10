//! Metadata gap detection: aggregated counts of series/books missing metadata.
//!
//! Powers the backoffice "Données → Métadonnées" page. Counts are intentionally
//! computed with the same predicates as the list endpoints (`gap=` filters) so the
//! chips and the filtered lists can never disagree.

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

#[derive(Deserialize)]
pub struct GapSummaryQuery {
    /// Restrict counts to a single library
    pub library_id: Option<Uuid>,
}

/// Aggregated metadata-gap counts across series and books.
#[derive(Serialize, ToSchema)]
pub struct GapSummary {
    pub series_total: i64,
    pub series_no_description: i64,
    pub series_no_genre: i64,
    pub series_no_authors: i64,
    pub series_no_publishers: i64,
    pub series_no_year: i64,
    pub series_no_cover: i64,
    pub series_no_community_score: i64,
    pub books_total: i64,
    pub books_no_summary: i64,
    pub books_no_isbn: i64,
    pub books_no_cover: i64,
    pub books_no_author: i64,
    pub books_no_publish_date: i64,
    pub books_no_language: i64,
    pub books_no_volume: i64,
}

const SERIES_GAP_SQL: &str = r#"
    SELECT
        COUNT(*) AS series_total,
        COUNT(*) FILTER (WHERE s.description IS NULL OR s.description = '') AS series_no_description,
        COUNT(*) FILTER (WHERE COALESCE(cardinality(s.genres), 0) = 0) AS series_no_genre,
        COUNT(*) FILTER (WHERE COALESCE(cardinality(s.authors), 0) = 0) AS series_no_authors,
        COUNT(*) FILTER (WHERE COALESCE(cardinality(s.publishers), 0) = 0) AS series_no_publishers,
        COUNT(*) FILTER (WHERE s.start_year IS NULL) AS series_no_year,
        COUNT(*) FILTER (WHERE s.cover_url IS NULL OR s.cover_url = '') AS series_no_cover,
        COUNT(*) FILTER (WHERE NOT EXISTS (
            SELECT 1 FROM external_metadata_links eml
            WHERE eml.series_id = s.id
              AND eml.status = 'approved'
              AND eml.provider_rating IS NOT NULL
              AND eml.provider_rating > 0
        )) AS series_no_community_score
    FROM series s
    WHERE ($1::uuid IS NULL OR s.library_id = $1)
"#;

const BOOKS_GAP_SQL: &str = r#"
    SELECT
        COUNT(*) AS books_total,
        COUNT(*) FILTER (WHERE summary IS NULL OR summary = '') AS books_no_summary,
        COUNT(*) FILTER (WHERE isbn IS NULL OR isbn = '') AS books_no_isbn,
        COUNT(*) FILTER (WHERE thumbnail_path IS NULL) AS books_no_cover,
        COUNT(*) FILTER (WHERE COALESCE(
            NULLIF(b.authors, '{}'),
            CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END
        ) = ARRAY[]::text[]) AS books_no_author,
        COUNT(*) FILTER (WHERE b.publish_date IS NULL OR b.publish_date = '') AS books_no_publish_date,
        COUNT(*) FILTER (WHERE b.language IS NULL OR b.language = '') AS books_no_language,
        COUNT(*) FILTER (WHERE b.volume IS NULL) AS books_no_volume
    FROM books b
    WHERE ($1::uuid IS NULL OR b.library_id = $1)
"#;

/// Count series and books with metadata gaps, optionally scoped to one library.
#[utoipa::path(
    get,
    path = "/metadata/gaps/summary",
    tag = "metadata",
    params(
        ("library_id" = Option<String>, Query, description = "Filter counts by library ID"),
    ),
    responses(
        (status = 200, body = GapSummary),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_gap_summary(
    State(state): State<AppState>,
    Query(query): Query<GapSummaryQuery>,
) -> Result<Json<GapSummary>, ApiError> {
    let library_id = query.library_id;

    let series_row = sqlx::query(SERIES_GAP_SQL)
        .bind(library_id)
        .fetch_one(&state.pool)
        .await?;

    let books_row = sqlx::query(BOOKS_GAP_SQL)
        .bind(library_id)
        .fetch_one(&state.pool)
        .await?;

    Ok(Json(GapSummary {
        series_total: series_row.get("series_total"),
        series_no_description: series_row.get("series_no_description"),
        series_no_genre: series_row.get("series_no_genre"),
        series_no_authors: series_row.get("series_no_authors"),
        series_no_publishers: series_row.get("series_no_publishers"),
        series_no_year: series_row.get("series_no_year"),
        series_no_cover: series_row.get("series_no_cover"),
        series_no_community_score: series_row.get("series_no_community_score"),
        books_total: books_row.get("books_total"),
        books_no_summary: books_row.get("books_no_summary"),
        books_no_isbn: books_row.get("books_no_isbn"),
        books_no_cover: books_row.get("books_no_cover"),
        books_no_author: books_row.get("books_no_author"),
        books_no_publish_date: books_row.get("books_no_publish_date"),
        books_no_language: books_row.get("books_no_language"),
        books_no_volume: books_row.get("books_no_volume"),
    }))
}

#[cfg(test)]
#[path = "tests/gaps.rs"]
mod tests;
