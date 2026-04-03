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
    tag = "search",
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
            s.name AS series, b.volume, b.language
        FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        WHERE (
            b.title ILIKE $1
            OR s.name ILIKE $1
            OR EXISTS (SELECT 1 FROM unnest(
                COALESCE(b.authors, CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END)
                || COALESCE(s.authors, ARRAY[]::text[])
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
                b.library_id,
                COALESCE(s.name, 'unclassified') as name,
                b.id,
                ROW_NUMBER() OVER (
                    PARTITION BY b.library_id, COALESCE(s.name, 'unclassified')
                    ORDER BY
                        REGEXP_REPLACE(LOWER(b.title), '[0-9]+', '', 'g'),
                        COALESCE((REGEXP_MATCH(LOWER(b.title), '\d+'))[1]::int, 0),
                        b.title ASC
                ) as rn
            FROM books b
            LEFT JOIN series s ON s.id = b.series_id
            WHERE ($2::uuid IS NULL OR b.library_id = $2)
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    async fn create_test_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(name)
            .bind(format!("/libraries/{name}"))
            .execute(pool)
            .await
            .unwrap();
        id
    }

    async fn create_test_series(pool: &sqlx::PgPool, library_id: Uuid, name: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO series (id, library_id, name, created_at, updated_at) VALUES (gen_random_uuid(), $1, $2, NOW(), NOW()) RETURNING id",
        )
        .bind(library_id)
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn create_test_book(
        pool: &sqlx::PgPool,
        library_id: Uuid,
        series_id: Option<Uuid>,
        title: &str,
        kind: &str,
        author: Option<&str>,
        authors: &[&str],
    ) -> Uuid {
        let id = Uuid::new_v4();
        let authors_vec: Vec<String> = authors.iter().map(|a| a.to_string()).collect();
        sqlx::query(
            "INSERT INTO books (id, library_id, series_id, title, kind, author, authors) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(library_id)
        .bind(series_id)
        .bind(title)
        .bind(kind)
        .bind(author)
        .bind(&authors_vec)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    /// The books search SQL from the handler, extracted for testing.
    const BOOKS_SQL: &str = r#"
        SELECT b.id, b.library_id, b.kind, b.title,
            COALESCE(b.authors, CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END) as authors,
            s.name AS series, b.volume, b.language
        FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        WHERE (
            b.title ILIKE $1
            OR s.name ILIKE $1
            OR EXISTS (SELECT 1 FROM unnest(
                COALESCE(b.authors, CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END)
                || COALESCE(s.authors, ARRAY[]::text[])
            ) AS a WHERE a ILIKE $1)
        )
        AND ($2::uuid IS NULL OR b.library_id = $2)
        AND ($3::text IS NULL OR b.kind = $3)
        ORDER BY
            CASE WHEN b.title ILIKE $1 THEN 0 ELSE 1 END,
            b.title ASC
        LIMIT $4
    "#;

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn search_by_book_title(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "comics").await;
        create_test_book(&pool, lib_id, None, "Batman Year One", "comic", None, &[]).await;
        create_test_book(&pool, lib_id, None, "Superman Returns", "comic", None, &[]).await;

        let rows = sqlx::query(BOOKS_SQL)
            .bind("%Batman%")
            .bind(None::<Uuid>)
            .bind(None::<&str>)
            .bind(20i64)
            .fetch_all(&pool)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get::<String, _>("title"), "Batman Year One");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn search_by_series_name(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "comics").await;
        let series_id = create_test_series(&pool, lib_id, "Dragon Ball").await;
        create_test_book(&pool, lib_id, Some(series_id), "Volume 1", "comic", None, &[]).await;
        create_test_book(&pool, lib_id, None, "Unrelated Book", "comic", None, &[]).await;

        let rows = sqlx::query(BOOKS_SQL)
            .bind("%Dragon%")
            .bind(None::<Uuid>)
            .bind(None::<&str>)
            .bind(20i64)
            .fetch_all(&pool)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get::<String, _>("title"), "Volume 1");
        assert_eq!(rows[0].get::<Option<String>, _>("series").unwrap(), "Dragon Ball");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn search_by_author(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "comics").await;
        create_test_book(&pool, lib_id, None, "The Sandman", "comic", Some("Neil Gaiman"), &["Neil Gaiman"]).await;
        create_test_book(&pool, lib_id, None, "Watchmen", "comic", Some("Alan Moore"), &["Alan Moore"]).await;

        let rows = sqlx::query(BOOKS_SQL)
            .bind("%Gaiman%")
            .bind(None::<Uuid>)
            .bind(None::<&str>)
            .bind(20i64)
            .fetch_all(&pool)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get::<String, _>("title"), "The Sandman");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn search_case_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "comics").await;
        create_test_book(&pool, lib_id, None, "Batman Year One", "comic", None, &[]).await;

        // Search with lowercase
        let rows = sqlx::query(BOOKS_SQL)
            .bind("%batman%")
            .bind(None::<Uuid>)
            .bind(None::<&str>)
            .bind(20i64)
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);

        // Search with uppercase
        let rows = sqlx::query(BOOKS_SQL)
            .bind("%BATMAN%")
            .bind(None::<Uuid>)
            .bind(None::<&str>)
            .bind(20i64)
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn search_library_scoped(pool: sqlx::PgPool) {
        let lib1 = create_test_library(&pool, "comics").await;
        let lib2 = create_test_library(&pool, "manga").await;
        create_test_book(&pool, lib1, None, "Batman Vol 1", "comic", None, &[]).await;
        create_test_book(&pool, lib2, None, "Batman Manga", "comic", None, &[]).await;

        // Search scoped to lib1 only
        let rows = sqlx::query(BOOKS_SQL)
            .bind("%Batman%")
            .bind(Some(lib1))
            .bind(None::<&str>)
            .bind(20i64)
            .fetch_all(&pool)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get::<String, _>("title"), "Batman Vol 1");
        assert_eq!(rows[0].get::<Uuid, _>("library_id"), lib1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn search_empty_query_returns_nothing(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "comics").await;
        create_test_book(&pool, lib_id, None, "Batman", "comic", None, &[]).await;

        // Empty ILIKE pattern "%%" matches everything — but the handler rejects empty q.
        // With a truly empty pattern (just whitespace wrapped in %), it matches all.
        // This test verifies the SQL behavior with a pattern that matches nothing.
        let rows = sqlx::query(BOOKS_SQL)
            .bind("%zzz_no_match_zzz%")
            .bind(None::<Uuid>)
            .bind(None::<&str>)
            .bind(20i64)
            .fetch_all(&pool)
            .await
            .unwrap();

        assert_eq!(rows.len(), 0);
    }
}
