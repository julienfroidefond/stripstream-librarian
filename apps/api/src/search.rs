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

/// Search books across all libraries using Meilisearch
#[utoipa::path(
    get,
    path = "/search",
    tag = "books",
    params(
        ("q" = String, Query, description = "Search query (books via Meilisearch + series via ILIKE)"),
        ("library_id" = Option<String>, Query, description = "Filter by library ID"),
        ("type" = Option<String>, Query, description = "Filter by type (cbz, cbr, pdf)"),
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

    let mut filters: Vec<String> = Vec::new();
    if let Some(library_id) = query.library_id.as_deref() {
        filters.push(format!("library_id = '{}'", library_id.replace('"', "")));
    }
    let kind_filter = query.r#type.as_deref().or(query.kind.as_deref());
    if let Some(kind) = kind_filter {
        filters.push(format!("kind = '{}'", kind.replace('"', "")));
    }

    let body = serde_json::json!({
        "q": query.q,
        "limit": query.limit.unwrap_or(20).clamp(1, 100),
        "filter": if filters.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(filters.join(" AND ")) }
    });

    let limit_val = query.limit.unwrap_or(20).clamp(1, 100);
    let q_pattern = format!("%{}%", query.q);
    let library_id_uuid: Option<uuid::Uuid> = query.library_id.as_deref()
        .and_then(|s| s.parse().ok());

    // Recherche Meilisearch (books) + séries PG en parallèle
    let client = reqwest::Client::new();
    let url = format!("{}/indexes/books/search", state.meili_url.trim_end_matches('/'));
    let meili_fut = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", state.meili_master_key))
        .json(&body)
        .send();

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
            WHERE ($1::uuid IS NULL OR library_id = $1)
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
        WHERE sc.name ILIKE $2
        ORDER BY sc.name ASC
        LIMIT $3
    "#;

    let series_fut = sqlx::query(series_sql)
        .bind(library_id_uuid)
        .bind(&q_pattern)
        .bind(limit_val as i64)
        .fetch_all(&state.pool);

    let (meili_resp, series_rows) = tokio::join!(meili_fut, series_fut);

    // Traitement Meilisearch
    let meili_resp = meili_resp.map_err(|e| ApiError::internal(format!("meili request failed: {e}")))?;
    let (hits, estimated_total_hits, processing_time_ms) = if !meili_resp.status().is_success() {
        let body = meili_resp.text().await.unwrap_or_default();
        if body.contains("index_not_found") {
            (serde_json::json!([]), Some(0u64), Some(0u64))
        } else {
            return Err(ApiError::internal(format!("meili error: {body}")));
        }
    } else {
        let payload: serde_json::Value = meili_resp.json().await
            .map_err(|e| ApiError::internal(format!("invalid meili response: {e}")))?;
        (
            payload.get("hits").cloned().unwrap_or_else(|| serde_json::json!([])),
            payload.get("estimatedTotalHits").and_then(|v| v.as_u64()),
            payload.get("processingTimeMs").and_then(|v| v.as_u64()),
        )
    };

    // Traitement séries
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
        hits,
        series_hits,
        estimated_total_hits,
        processing_time_ms,
    }))
}
