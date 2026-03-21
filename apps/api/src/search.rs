use axum::{extract::{Query, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct SearchQuery {
    #[schema(value_type = String, example = "batman")]
    pub q: String,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<String>,
    #[schema(value_type = Option<String>, example = "cbz")]
    pub r#type: Option<String>,
    #[schema(value_type = Option<String>, example = "cbz")]
    pub kind: Option<String>,
    #[schema(value_type = Option<usize>, example = 20)]
    pub limit: Option<usize>,
}

#[derive(Serialize, ToSchema)]
pub struct SeriesHit {
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub name: String,
    pub book_count: i64,
    pub books_read_count: i64,
    #[schema(value_type = String)]
    pub first_book_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub struct SearchResponse {
    pub hits: serde_json::Value,
    pub series_hits: Vec<SeriesHit>,
    pub estimated_total_hits: Option<u64>,
    pub processing_time_ms: Option<u64>,
}

/// Search books across all libraries
#[utoipa::path(
    get,
    path = "/search",
    tag = "books",
    params(
        ("q" = String, Query, description = "Search query (books + series via PostgreSQL full-text)"),
        ("library_id" = Option<String>, Query, description = "Filter by library ID"),
        ("type" = Option<String>, Query, description = "Filter by type (cbz, cbr, pdf, epub)"),
        ("kind" = Option<String>, Query, description = "Filter by kind (alias for type)"),
        ("limit" = Option<usize>, Query, description = "Max results per type (max 100)"),
    ),
    responses(
        (status = 200, body = SearchResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn search_books(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, ApiError> {
    if query.q.trim().is_empty() {
        return Err(ApiError::bad_request("q is required"));
    }

    let limit_val = query.limit.unwrap_or(20).clamp(1, 100) as i64;
    let q_pattern = format!("%{}%", query.q);
    let library_id_uuid: Option<Uuid> = query.library_id.as_deref()
        .and_then(|s| s.parse().ok());
    let kind_filter: Option<&str> = query.r#type.as_deref().or(query.kind.as_deref());

    let start = std::time::Instant::now();

    // Book search via PostgreSQL ILIKE on title, authors, series
    let books_sql = r#"
        SELECT b.id, b.library_id, b.kind, b.title,
            COALESCE(b.authors, CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END) as authors,
            b.series, b.volume, b.language
        FROM books b
        LEFT JOIN series_metadata sm
            ON sm.library_id = b.library_id
            AND sm.name = COALESCE(NULLIF(b.series, ''), 'unclassified')
        WHERE (
            b.title ILIKE $1
            OR b.series ILIKE $1
            OR EXISTS (SELECT 1 FROM unnest(
                COALESCE(b.authors, CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END)
                || COALESCE(sm.authors, ARRAY[]::text[])
            ) AS a WHERE a ILIKE $1)
        )
        AND ($2::uuid IS NULL OR b.library_id = $2)
        AND ($3::text IS NULL OR b.kind = $3)
        ORDER BY
            CASE WHEN b.title ILIKE $1 THEN 0 ELSE 1 END,
            b.title ASC
        LIMIT $4
    "#;

    let series_sql = r#"
        WITH sorted_books AS (
            SELECT
                library_id,
                COALESCE(NULLIF(series, ''), 'unclassified') as name,
                id,
                ROW_NUMBER() OVER (
                    PARTITION BY library_id, COALESCE(NULLIF(series, ''), 'unclassified')
                    ORDER BY
                        REGEXP_REPLACE(LOWER(title), '[0-9]+', '', 'g'),
                        COALESCE((REGEXP_MATCH(LOWER(title), '\d+'))[1]::int, 0),
                        title ASC
                ) as rn
            FROM books
            WHERE ($2::uuid IS NULL OR library_id = $2)
        ),
        series_counts AS (
            SELECT
                sb.library_id,
                sb.name,
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM sorted_books sb
            LEFT JOIN book_reading_progress brp ON brp.book_id = sb.id
            GROUP BY sb.library_id, sb.name
        )
        SELECT sc.library_id, sc.name, sc.book_count, sc.books_read_count, sb.id as first_book_id
        FROM series_counts sc
        JOIN sorted_books sb ON sb.library_id = sc.library_id AND sb.name = sc.name AND sb.rn = 1
        WHERE sc.name ILIKE $1
        ORDER BY sc.name ASC
        LIMIT $4
    "#;

    let (books_rows, series_rows) = tokio::join!(
        sqlx::query(books_sql)
            .bind(&q_pattern)
            .bind(library_id_uuid)
            .bind(kind_filter)
            .bind(limit_val)
            .fetch_all(&state.pool),
        sqlx::query(series_sql)
            .bind(&q_pattern)
            .bind(library_id_uuid)
            .bind(kind_filter) // unused in series query but keeps bind positions consistent
            .bind(limit_val)
            .fetch_all(&state.pool)
    );

    let elapsed_ms = start.elapsed().as_millis() as u64;

    // Build book hits as JSON array (same shape as before)
    let books_rows = books_rows.map_err(|e| ApiError::internal(format!("book search failed: {e}")))?;
    let hits: Vec<serde_json::Value> = books_rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.get::<Uuid, _>("id").to_string(),
                "library_id": row.get::<Uuid, _>("library_id").to_string(),
                "kind": row.get::<String, _>("kind"),
                "title": row.get::<String, _>("title"),
                "authors": row.get::<Vec<String>, _>("authors"),
                "series": row.get::<Option<String>, _>("series"),
                "volume": row.get::<Option<i32>, _>("volume"),
                "language": row.get::<Option<String>, _>("language"),
            })
        })
        .collect();

    let estimated_total_hits = hits.len() as u64;

    // Series hits
    let series_hits: Vec<SeriesHit> = series_rows
        .unwrap_or_default()
        .iter()
        .map(|row| SeriesHit {
            library_id: row.get("library_id"),
            name: row.get("name"),
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
        })
        .collect();

    Ok(Json(SearchResponse {
        hits: serde_json::Value::Array(hits),
        series_hits,
        estimated_total_hits: Some(estimated_total_hits),
        processing_time_ms: Some(elapsed_ms),
    }))
}
