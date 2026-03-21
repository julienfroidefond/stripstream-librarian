use axum::{extract::{Query, State}, Json};
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

    let q_pattern = query.q.as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("%{s}%"));

    // Aggregate unique authors from books.authors + books.author + series_metadata.authors
    let sql = format!(
        r#"
        WITH all_authors AS (
            SELECT DISTINCT UNNEST(
                COALESCE(
                    NULLIF(authors, '{{}}'),
                    CASE WHEN author IS NOT NULL AND author != '' THEN ARRAY[author] ELSE ARRAY[]::text[] END
                )
            ) AS name
            FROM books
            UNION
            SELECT DISTINCT UNNEST(authors) AS name
            FROM series_metadata
            WHERE authors != '{{}}'
        ),
        filtered AS (
            SELECT name FROM all_authors
            WHERE ($1::text IS NULL OR name ILIKE $1)
        ),
        book_counts AS (
            SELECT
                f.name AS author_name,
                COUNT(DISTINCT b.id) AS book_count
            FROM filtered f
            LEFT JOIN books b ON (
                f.name = ANY(
                    COALESCE(
                        NULLIF(b.authors, '{{}}'),
                        CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END
                    )
                )
            )
            GROUP BY f.name
        ),
        series_counts AS (
            SELECT
                f.name AS author_name,
                COUNT(DISTINCT (sm.library_id, sm.name)) AS series_count
            FROM filtered f
            LEFT JOIN series_metadata sm ON (
                f.name = ANY(sm.authors) AND sm.authors != '{{}}'
            )
            GROUP BY f.name
        )
        SELECT
            f.name,
            COALESCE(bc.book_count, 0) AS book_count,
            COALESCE(sc.series_count, 0) AS series_count
        FROM filtered f
        LEFT JOIN book_counts bc ON bc.author_name = f.name
        LEFT JOIN series_counts sc ON sc.author_name = f.name
        ORDER BY {order_clause}
        LIMIT $2 OFFSET $3
        "#
    );

    let count_sql = r#"
        WITH all_authors AS (
            SELECT DISTINCT UNNEST(
                COALESCE(
                    NULLIF(authors, '{}'),
                    CASE WHEN author IS NOT NULL AND author != '' THEN ARRAY[author] ELSE ARRAY[]::text[] END
                )
            ) AS name
            FROM books
            UNION
            SELECT DISTINCT UNNEST(authors) AS name
            FROM series_metadata
            WHERE authors != '{}'
        )
        SELECT COUNT(*) AS total
        FROM all_authors
        WHERE ($1::text IS NULL OR name ILIKE $1)
    "#;

    let (rows, count_row) = tokio::join!(
        sqlx::query(&sql)
            .bind(q_pattern.as_deref())
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool),
        sqlx::query(count_sql)
            .bind(q_pattern.as_deref())
            .fetch_one(&state.pool)
    );

    let rows = rows.map_err(|e| ApiError::internal(format!("authors query failed: {e}")))?;
    let total: i64 = count_row
        .map_err(|e| ApiError::internal(format!("authors count failed: {e}")))?
        .get("total");

    let items: Vec<AuthorItem> = rows
        .iter()
        .map(|r| AuthorItem {
            name: r.get("name"),
            book_count: r.get("book_count"),
            series_count: r.get("series_count"),
        })
        .collect();

    Ok(Json(AuthorsPageResponse {
        items,
        total,
        page,
        limit,
    }))
}
