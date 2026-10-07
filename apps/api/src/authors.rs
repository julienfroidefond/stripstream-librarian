use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

use crate::{error::ApiError, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct ListAuthorsQuery {
    #[schema(value_type = Option<String>, example = "batman")]
    pub q: Option<String>,
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 20)]
    pub limit: Option<i64>,
    /// Sort order: "name" (default), "books" (most books first)
    #[schema(value_type = Option<String>, example = "books")]
    pub sort: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct AuthorItem {
    pub name: String,
    pub book_count: i64,
    pub series_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct AuthorsPageResponse {
    pub items: Vec<AuthorItem>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

/// List all unique authors with book/series counts
#[utoipa::path(
    get,
    path = "/authors",
    tag = "authors",
    params(
        ("q" = Option<String>, Query, description = "Search by author name"),
        ("page" = Option<i64>, Query, description = "Page number (1-based)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 100)"),
        ("sort" = Option<String>, Query, description = "Sort: name (default) or books"),
    ),
    responses(
        (status = 200, body = AuthorsPageResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_authors(
    State(state): State<AppState>,
    Query(query): Query<ListAuthorsQuery>,
) -> Result<Json<AuthorsPageResponse>, ApiError> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;
    let sort = query.sort.as_deref().unwrap_or("name");

    let order_clause = match sort {
        "books" => "book_count DESC, name ASC",
        _ => "name ASC",
    };

    let q_pattern = query
        .q
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("%{s}%"));

    // Single query: collect authors from both book-level and series-level
    // metadata, with counts using a windowed total. Series like Astérix store
    // their authors on the series row and may have no local books.
    //
    // `author_agg` is aggregated once; the lateral join keeps the real total
    // even when the requested page is empty.
    let sql = format!(
        r#"
        WITH author_rows AS (
            SELECT UNNEST(
                COALESCE(
                    NULLIF(b.authors, '{{}}'),
                    CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END
                )
            ) AS author_name, b.id AS book_id, b.series_id
            FROM books b
            UNION ALL
            SELECT UNNEST(COALESCE(s.authors, ARRAY[]::text[])) AS author_name,
                   NULL::uuid AS book_id,
                   s.id AS series_id
            FROM series s
        ),
        author_agg AS (
            SELECT
                author_name AS name,
                COUNT(DISTINCT book_id) AS book_count,
                COUNT(DISTINCT series_id) AS series_count
            FROM author_rows
            WHERE author_name IS NOT NULL
              AND btrim(author_name) <> ''
              AND ($1::text IS NULL OR author_name ILIKE $1)
            GROUP BY author_name
        ),
        total AS (SELECT COUNT(*) AS total_count FROM author_agg)
        SELECT a.*, t.total_count
        FROM total t
        LEFT JOIN LATERAL (
            SELECT * FROM author_agg
            ORDER BY {order_clause}
            LIMIT $2 OFFSET $3
        ) a ON TRUE
        "#
    );

    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(q_pattern.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::internal(format!("authors query failed: {e}")))?;

    let total: i64 = rows.first().map(|r| r.get("total_count")).unwrap_or(0);

    let items: Vec<AuthorItem> = rows
        .iter()
        .filter_map(|r| {
            let name: Option<String> = r.get("name");
            let name = name?;
            Some(AuthorItem {
                name,
                book_count: r.get("book_count"),
                series_count: r.get("series_count"),
            })
        })
        .collect();

    Ok(Json(AuthorsPageResponse {
        items,
        total,
        page,
        limit,
    }))
}

#[cfg(test)]
#[path = "tests/authors.rs"]
mod tests;
