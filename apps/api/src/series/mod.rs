mod helpers;
pub mod merge;
pub mod update;
#[cfg(test)]
mod tests;

pub(crate) use helpers::{get_or_create_series, resolve_library_id};
pub use merge::*;
pub use update::*;

use axum::extract::Extension;
use axum::{extract::{Path, Query, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{auth::AuthUser, books::BookItem, error::ApiError, state::AppState};

// ─── Structs ─────────────────────────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct SeriesLookup {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub struct SeriesItem {
    pub name: String,
    #[schema(value_type = String)]
    pub series_id: Uuid,
    pub book_count: i64,
    pub books_read_count: i64,
    #[schema(value_type = Option<String>)]
    pub first_book_id: Option<Uuid>,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub series_status: Option<String>,
    pub missing_count: Option<i64>,
    pub metadata_provider: Option<String>,
    pub anilist_id: Option<i32>,
    pub anilist_url: Option<String>,
    pub cover_url: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct SeriesPage {
    pub items: Vec<SeriesItem>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct ListSeriesQuery {
    #[schema(value_type = Option<String>, example = "dragon")]
    pub q: Option<String>,
    #[schema(value_type = Option<String>, example = "unread,reading")]
    pub reading_status: Option<String>,
    /// Filter by series status (e.g. "ongoing", "ended")
    #[schema(value_type = Option<String>, example = "ongoing")]
    pub series_status: Option<String>,
    /// Filter series with missing books: "true" to show only series with missing books
    #[schema(value_type = Option<String>, example = "true")]
    pub has_missing: Option<String>,
    /// Filter by metadata provider: a provider name (e.g. "google_books"), "linked" (any provider), or "unlinked" (no provider)
    #[schema(value_type = Option<String>, example = "google_books")]
    pub metadata_provider: Option<String>,
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 50)]
    pub limit: Option<i64>,
}

#[derive(Deserialize, ToSchema)]
pub struct ListAllSeriesQuery {
    #[schema(value_type = Option<String>, example = "dragon")]
    pub q: Option<String>,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    #[schema(value_type = Option<String>, example = "unread,reading")]
    pub reading_status: Option<String>,
    /// Filter by series status (e.g. "ongoing", "ended")
    #[schema(value_type = Option<String>, example = "ongoing")]
    pub series_status: Option<String>,
    /// Filter series with missing books: "true" to show only series with missing books
    #[schema(value_type = Option<String>, example = "true")]
    pub has_missing: Option<String>,
    /// Filter by metadata provider: a provider name (e.g. "google_books"), "linked" (any provider), or "unlinked" (no provider)
    #[schema(value_type = Option<String>, example = "google_books")]
    pub metadata_provider: Option<String>,
    /// Filter by author name (matches in series.authors or book-level authors)
    #[schema(value_type = Option<String>, example = "Toriyama")]
    pub author: Option<String>,
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 50)]
    pub limit: Option<i64>,
    /// Sort order: "title" (default) or "latest" (most recently added first)
    #[schema(value_type = Option<String>, example = "latest")]
    pub sort: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct OngoingQuery {
    #[schema(value_type = Option<i64>, example = 10)]
    pub limit: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct SeriesMetadata {
    /// Name of the series
    pub series_name: String,
    /// Authors of the series (series-level metadata, distinct from per-book author field)
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub publishers: Vec<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    /// Series status: "ongoing", "ended", "hiatus", "cancelled", or null
    pub status: Option<String>,
    /// Convenience: author from first book (for pre-filling the per-book apply section)
    pub book_author: Option<String>,
    pub book_language: Option<String>,
    /// Fields locked from external metadata sync, e.g. {"authors": true, "description": true}
    pub locked_fields: serde_json::Value,
}

// ─── Lookup by name ──────────────────────────────────────────────────────────

/// Look up a series by name within a library. Returns its UUID and name.
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series/by-name/{name}",
    tag = "series",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("name" = String, Path, description = "Series name (URL-encoded)"),
    ),
    responses(
        (status = 200, body = SeriesLookup),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_by_name(
    State(state): State<AppState>,
    Path((library_id, name)): Path<(Uuid, String)>,
) -> Result<Json<SeriesLookup>, ApiError> {
    let row = sqlx::query(
        "SELECT id, library_id, name FROM series WHERE library_id = $1 AND LOWER(name) = LOWER($2)"
    )
    .bind(library_id)
    .bind(&name)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found(format!("series '{}' not found", name)))?;

    Ok(Json(SeriesLookup {
        id: row.get("id"),
        library_id: row.get("library_id"),
        name: row.get("name"),
    }))
}

// ─── List series (per library) ───────────────────────────────────────────────

/// List all series in a library with pagination
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series",
    tag = "series",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("q" = Option<String>, Query, description = "Filter by series name (case-insensitive, partial match)"),
        ("reading_status" = Option<String>, Query, description = "Filter by reading status, comma-separated (e.g. 'unread,reading')"),
        ("metadata_provider" = Option<String>, Query, description = "Filter by metadata provider: a provider name (e.g. 'google_books'), 'linked' (any provider), or 'unlinked' (no provider)"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
    ),
    responses(
        (status = 200, body = SeriesPage),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_series(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(library_id): Path<Uuid>,
    Query(query): Query<ListSeriesQuery>,
) -> Result<Json<SeriesPage>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let reading_statuses: Option<Vec<String>> = query.reading_status.as_deref().map(|s| {
        s.split(',').map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect()
    });

    let series_status_expr = r#"CASE
              WHEN sc.books_read_count = sc.book_count THEN 'read'
              WHEN sc.books_read_count = 0 THEN 'unread'
              ELSE 'reading'
           END"#;

    let has_missing = query.has_missing.as_deref() == Some("true");

    // Paramètres dynamiques — $1 = library_id fixe, puis optionnels dans l'ordre
    let mut p: usize = 1;

    let q_cond = if query.q.is_some() {
        p += 1; format!("AND s.name ILIKE ${p}")
    } else { String::new() };

    let count_rs_cond = if reading_statuses.is_some() {
        p += 1; format!("AND {series_status_expr} = ANY(${p})")
    } else { String::new() };

    let ss_cond = if query.series_status.is_some() {
        p += 1; format!("AND LOWER(s.status) = ${p}")
    } else { String::new() };

    let missing_cond = if has_missing {
        "AND mc.missing_count > 0".to_string()
    } else { String::new() };

    let metadata_provider_cond = match query.metadata_provider.as_deref() {
        Some("unlinked") => "AND ml.provider IS NULL".to_string(),
        Some("linked") => "AND ml.provider IS NOT NULL".to_string(),
        Some(_) => { p += 1; format!("AND ml.provider = ${p}") },
        None => String::new(),
    };

    let user_id_p = p + 1;
    let limit_p = p + 2;
    let offset_p = p + 3;

    let missing_cte = helpers::build_missing_counts_cte(Some("$1"));
    let metadata_links_cte = helpers::METADATA_LINKS_CTE;

    let count_sql = format!(
        r#"
        WITH series_counts AS (
            SELECT s.id as series_id, s.name,
                COUNT(b.id) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ${user_id_p}::uuid IS NOT NULL AND brp.user_id = ${user_id_p}
            WHERE s.library_id = $1
            GROUP BY s.id, s.name
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT COUNT(*) FROM series_counts sc
        LEFT JOIN series s ON s.id = sc.series_id
        LEFT JOIN missing_counts mc ON mc.series_id = sc.series_id
        LEFT JOIN metadata_links ml ON ml.series_id = sc.series_id AND ml.library_id = $1
        WHERE TRUE {q_cond} {count_rs_cond} {ss_cond} {missing_cond} {metadata_provider_cond}
        "#
    );

    let data_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT
                b.series_id,
                b.id,
                ROW_NUMBER() OVER (
                    PARTITION BY b.series_id
                    ORDER BY
                        b.volume NULLS LAST,
                        REGEXP_REPLACE(LOWER(b.title), '[0-9].*$', ''),
                        COALESCE((REGEXP_MATCH(LOWER(b.title), '\d+'))[1]::int, 0),
                        b.title ASC
                ) as rn
            FROM books b
            WHERE b.library_id = $1
        ),
        series_counts AS (
            SELECT
                s.id as series_id,
                s.name,
                COUNT(b.id) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ${user_id_p}::uuid IS NOT NULL AND brp.user_id = ${user_id_p}
            WHERE s.library_id = $1
            GROUP BY s.id, s.name
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT
            sc.name,
            sc.series_id,
            sc.book_count,
            sc.books_read_count,
            sb.id as first_book_id,
            s.status as series_status,
            mc.missing_count,
            ml.provider as metadata_provider,
            asl.anilist_id,
            asl.anilist_url,
            s.cover_url
        FROM series_counts sc
        JOIN sorted_books sb ON sb.series_id = sc.series_id AND sb.rn = 1
        LEFT JOIN series s ON s.id = sc.series_id
        LEFT JOIN missing_counts mc ON mc.series_id = sc.series_id
        LEFT JOIN metadata_links ml ON ml.series_id = sc.series_id AND ml.library_id = $1
        LEFT JOIN anilist_series_links asl ON asl.series_id = sc.series_id AND asl.provider = 'anilist'
        WHERE TRUE
          {q_cond}
          {count_rs_cond}
          {ss_cond}
          {missing_cond}
          {metadata_provider_cond}
        ORDER BY
            REGEXP_REPLACE(LOWER(sc.name), '[0-9].*$', ''),
            COALESCE(
                (REGEXP_MATCH(LOWER(sc.name), '\d+'))[1]::int,
                0
            ),
            sc.name ASC
        LIMIT ${limit_p} OFFSET ${offset_p}
        "#
    );

    let q_pattern = query.q.as_deref().map(|q| format!("%{}%", q));

    let mut count_builder = sqlx::query(&count_sql).bind(library_id);
    let mut data_builder = sqlx::query(&data_sql).bind(library_id);

    if let Some(ref pat) = q_pattern {
        count_builder = count_builder.bind(pat);
        data_builder = data_builder.bind(pat);
    }
    if let Some(ref statuses) = reading_statuses {
        count_builder = count_builder.bind(statuses.clone());
        data_builder = data_builder.bind(statuses.clone());
    }
    if let Some(ref ss) = query.series_status {
        count_builder = count_builder.bind(ss);
        data_builder = data_builder.bind(ss);
    }
    if let Some(ref mp) = query.metadata_provider {
        if mp != "linked" && mp != "unlinked" {
            count_builder = count_builder.bind(mp);
            data_builder = data_builder.bind(mp);
        }
    }

    count_builder = count_builder.bind(user_id);
    data_builder = data_builder.bind(user_id).bind(limit).bind(offset);

    let (count_row, rows) = tokio::try_join!(
        count_builder.fetch_one(&state.pool),
        data_builder.fetch_all(&state.pool),
    )?;
    let total: i64 = count_row.get(0);

    let items: Vec<SeriesItem> = rows
        .iter()
        .map(|row| SeriesItem {
            name: row.get("name"),
            series_id: row.get("series_id"),
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
            library_id,
            series_status: row.get("series_status"),
            missing_count: row.get("missing_count"),
            metadata_provider: row.get("metadata_provider"),
            anilist_id: row.get("anilist_id"),
            anilist_url: row.get("anilist_url"),
            cover_url: row.get("cover_url"),
        })
        .collect();

    Ok(Json(SeriesPage {
        items,
        total,
        page,
        limit,
    }))
}

// ─── List all series (cross-library) ─────────────────────────────────────────

/// List all series across libraries with optional filtering and pagination
#[utoipa::path(
    get,
    path = "/series",
    tag = "series",
    params(
        ("q" = Option<String>, Query, description = "Filter by series name (case-insensitive, partial match)"),
        ("library_id" = Option<String>, Query, description = "Filter by library ID"),
        ("reading_status" = Option<String>, Query, description = "Filter by reading status, comma-separated (e.g. 'unread,reading')"),
        ("metadata_provider" = Option<String>, Query, description = "Filter by metadata provider: a provider name (e.g. 'google_books'), 'linked' (any provider), or 'unlinked' (no provider)"),
        ("author" = Option<String>, Query, description = "Filter by author name (matches in series.authors or book-level authors)"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
        ("sort" = Option<String>, Query, description = "Sort order: 'title' (default) or 'latest' (most recently added first)"),
    ),
    responses(
        (status = 200, body = SeriesPage),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_all_series(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Query(query): Query<ListAllSeriesQuery>,
) -> Result<Json<SeriesPage>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let reading_statuses: Option<Vec<String>> = query.reading_status.as_deref().map(|s| {
        s.split(',').map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect()
    });

    let series_status_expr = r#"CASE
              WHEN sc.books_read_count = sc.book_count THEN 'read'
              WHEN sc.books_read_count = 0 THEN 'unread'
              ELSE 'reading'
           END"#;

    let has_missing = query.has_missing.as_deref() == Some("true");

    let mut p: usize = 0;

    let lib_cond = if query.library_id.is_some() {
        p += 1; format!("WHERE s.library_id = ${p}")
    } else {
        "WHERE TRUE".to_string()
    };

    let q_cond = if query.q.is_some() {
        p += 1; format!("AND s.name ILIKE ${p}")
    } else { String::new() };

    let rs_cond = if reading_statuses.is_some() {
        p += 1; format!("AND {series_status_expr} = ANY(${p})")
    } else { String::new() };

    let ss_cond = if query.series_status.is_some() {
        p += 1; format!("AND LOWER(s.status) = ${p}")
    } else { String::new() };

    let missing_cond = if has_missing {
        "AND mc.missing_count > 0".to_string()
    } else { String::new() };

    let metadata_provider_cond = match query.metadata_provider.as_deref() {
        Some("unlinked") => "AND ml.provider IS NULL".to_string(),
        Some("linked") => "AND ml.provider IS NOT NULL".to_string(),
        Some(_) => { p += 1; format!("AND ml.provider = ${p}") },
        None => String::new(),
    };

    let author_cond = if query.author.is_some() {
        p += 1; format!("AND (${p} = ANY(s.authors) OR EXISTS (SELECT 1 FROM books bk WHERE bk.series_id = s.id AND ${p} = ANY(COALESCE(NULLIF(bk.authors, '{{}}'), CASE WHEN bk.author IS NOT NULL AND bk.author != '' THEN ARRAY[bk.author] ELSE ARRAY[]::text[] END))))")
    } else { String::new() };

    // Missing counts CTE — based on series.total_volumes - book_count
    let missing_cte = if query.library_id.is_some() {
        helpers::build_missing_counts_cte(Some("$1"))
    } else {
        helpers::build_missing_counts_cte(None)
    };

    let metadata_links_cte = helpers::METADATA_LINKS_CTE;

    let user_id_p = p + 1;
    let limit_p = p + 2;
    let offset_p = p + 3;

    let count_sql = format!(
        r#"
        WITH series_counts AS (
            SELECT s.id as series_id, s.name, s.library_id,
                COUNT(b.id) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ${user_id_p}::uuid IS NOT NULL AND brp.user_id = ${user_id_p}
            {lib_cond}
            GROUP BY s.id, s.name, s.library_id
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT COUNT(*) FROM series_counts sc
        LEFT JOIN series s ON s.id = sc.series_id
        LEFT JOIN missing_counts mc ON mc.series_id = sc.series_id
        LEFT JOIN metadata_links ml ON ml.series_id = sc.series_id AND ml.library_id = sc.library_id
        WHERE TRUE {q_cond} {rs_cond} {ss_cond} {missing_cond} {metadata_provider_cond} {author_cond}
        "#
    );

    let series_order_clause = if query.sort.as_deref() == Some("latest") {
        "sc.latest_created_at DESC".to_string()
    } else {
        "REGEXP_REPLACE(LOWER(sc.name), '[0-9].*$', ''), COALESCE((REGEXP_MATCH(LOWER(sc.name), '\\d+'))[1]::int, 0), sc.name ASC".to_string()
    };

    let data_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT
                b.series_id,
                b.id,
                b.library_id,
                b.created_at,
                ROW_NUMBER() OVER (
                    PARTITION BY b.series_id
                    ORDER BY
                        b.volume NULLS LAST,
                        REGEXP_REPLACE(LOWER(b.title), '[0-9].*$', ''),
                        COALESCE((REGEXP_MATCH(LOWER(b.title), '\d+'))[1]::int, 0),
                        b.title ASC
                ) as rn
            FROM books b
            JOIN series s ON s.id = b.series_id
            {lib_cond}
        ),
        series_counts AS (
            SELECT
                s.id as series_id,
                s.name,
                s.library_id,
                COUNT(b.id) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count,
                MAX(b.created_at) as latest_created_at
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ${user_id_p}::uuid IS NOT NULL AND brp.user_id = ${user_id_p}
            {lib_cond}
            GROUP BY s.id, s.name, s.library_id
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT
            sc.name,
            sc.series_id,
            sc.book_count,
            sc.books_read_count,
            sb.id as first_book_id,
            sc.library_id,
            s.status as series_status,
            mc.missing_count,
            ml.provider as metadata_provider,
            asl.anilist_id,
            asl.anilist_url,
            s.cover_url
        FROM series_counts sc
        LEFT JOIN sorted_books sb ON sb.series_id = sc.series_id AND sb.rn = 1
        LEFT JOIN series s ON s.id = sc.series_id
        LEFT JOIN missing_counts mc ON mc.series_id = sc.series_id
        LEFT JOIN metadata_links ml ON ml.series_id = sc.series_id AND ml.library_id = sc.library_id
        LEFT JOIN anilist_series_links asl ON asl.series_id = sc.series_id AND asl.provider = 'anilist'
        WHERE TRUE
          {q_cond}
          {rs_cond}
          {ss_cond}
          {missing_cond}
          {metadata_provider_cond}
          {author_cond}
        ORDER BY {series_order_clause}
        LIMIT ${limit_p} OFFSET ${offset_p}
        "#
    );

    let q_pattern = query.q.as_deref().map(|q| format!("%{}%", q));

    let mut count_builder = sqlx::query(&count_sql);
    let mut data_builder = sqlx::query(&data_sql);

    if let Some(lib_id) = query.library_id {
        count_builder = count_builder.bind(lib_id);
        data_builder = data_builder.bind(lib_id);
    }
    if let Some(ref pat) = q_pattern {
        count_builder = count_builder.bind(pat);
        data_builder = data_builder.bind(pat);
    }
    if let Some(ref statuses) = reading_statuses {
        count_builder = count_builder.bind(statuses.clone());
        data_builder = data_builder.bind(statuses.clone());
    }
    if let Some(ref ss) = query.series_status {
        count_builder = count_builder.bind(ss);
        data_builder = data_builder.bind(ss);
    }
    if let Some(ref mp) = query.metadata_provider {
        if mp != "linked" && mp != "unlinked" {
            count_builder = count_builder.bind(mp);
            data_builder = data_builder.bind(mp);
        }
    }
    if let Some(ref author) = query.author {
        count_builder = count_builder.bind(author.clone());
        data_builder = data_builder.bind(author.clone());
    }

    count_builder = count_builder.bind(user_id);
    data_builder = data_builder.bind(user_id).bind(limit).bind(offset);

    let (count_row, rows) = tokio::try_join!(
        count_builder.fetch_one(&state.pool),
        data_builder.fetch_all(&state.pool),
    )?;
    let total: i64 = count_row.get(0);

    let items: Vec<SeriesItem> = rows
        .iter()
        .map(|row| SeriesItem {
            name: row.get("name"),
            series_id: row.get("series_id"),
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
            library_id: row.get("library_id"),
            series_status: row.get("series_status"),
            missing_count: row.get("missing_count"),
            metadata_provider: row.get("metadata_provider"),
            anilist_id: row.get("anilist_id"),
            anilist_url: row.get("anilist_url"),
            cover_url: row.get("cover_url"),
        })
        .collect();

    Ok(Json(SeriesPage {
        items,
        total,
        page,
        limit,
    }))
}

// ─── Series statuses ─────────────────────────────────────────────────────────

/// List all distinct series status values present in the database
#[utoipa::path(
    get,
    path = "/series/statuses",
    tag = "series",
    responses(
        (status = 200, body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn series_statuses(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"SELECT DISTINCT s FROM (
            SELECT LOWER(status) AS s FROM series WHERE status IS NOT NULL
            UNION
            SELECT mapped_status AS s FROM status_mappings WHERE mapped_status IS NOT NULL
        ) t ORDER BY s"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

/// List distinct raw provider statuses from external metadata links
#[utoipa::path(
    get,
    path = "/series/provider-statuses",
    tag = "series",
    responses(
        (status = 200, body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn provider_statuses(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"SELECT DISTINCT lower(metadata_json->>'status') AS s
           FROM external_metadata_links
           WHERE metadata_json->>'status' IS NOT NULL
             AND metadata_json->>'status' != ''
           ORDER BY s"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

// ─── Ongoing series / books ──────────────────────────────────────────────────

/// List ongoing series (partially read, sorted by most recent activity)
#[utoipa::path(
    get,
    path = "/series/ongoing",
    tag = "series",
    params(
        ("limit" = Option<i64>, Query, description = "Max items to return (default 10, max 50)"),
    ),
    responses(
        (status = 200, body = Vec<SeriesItem>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn ongoing_series(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Query(query): Query<OngoingQuery>,
) -> Result<Json<Vec<SeriesItem>>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let limit = query.limit.unwrap_or(10).clamp(1, 50);

    let rows = sqlx::query(
        r#"
        WITH series_stats AS (
            SELECT
                s.id AS series_id,
                s.name,
                s.library_id,
                COUNT(*) AS book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') AS books_read_count,
                MAX(brp.last_read_at) AS last_read_at
            FROM series s
            JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
            GROUP BY s.id, s.name, s.library_id
            HAVING (
                COUNT(brp.book_id) FILTER (WHERE brp.status IN ('read', 'reading')) > 0
                AND COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') < COUNT(*)
            )
        ),
        first_books AS (
            SELECT
                b.series_id,
                b.id,
                b.library_id,
                ROW_NUMBER() OVER (
                    PARTITION BY b.series_id
                    ORDER BY
                        b.volume NULLS LAST,
                        REGEXP_REPLACE(LOWER(b.title), '[0-9].*$', ''),
                        COALESCE((REGEXP_MATCH(LOWER(b.title), '\d+'))[1]::int, 0),
                        b.title ASC
                ) AS rn
            FROM books b
        )
        SELECT ss.name, ss.series_id, ss.book_count, ss.books_read_count, fb.id AS first_book_id, fb.library_id
        FROM series_stats ss
        JOIN first_books fb ON fb.series_id = ss.series_id AND fb.rn = 1
        ORDER BY ss.last_read_at DESC NULLS LAST
        LIMIT $1
        "#,
    )
    .bind(limit)
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    let items: Vec<SeriesItem> = rows
        .iter()
        .map(|row| SeriesItem {
            name: row.get("name"),
            series_id: row.get("series_id"),
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
            library_id: row.get("library_id"),
            series_status: None,
            missing_count: None,
            metadata_provider: None,
            anilist_id: None,
            anilist_url: None,
            cover_url: None,
        })
        .collect();

    Ok(Json(items))
}

/// List next unread book for each ongoing series (sorted by most recent activity)
#[utoipa::path(
    get,
    path = "/books/ongoing",
    tag = "series",
    params(
        ("limit" = Option<i64>, Query, description = "Max items to return (default 10, max 50)"),
    ),
    responses(
        (status = 200, body = Vec<BookItem>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn ongoing_books(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Query(query): Query<OngoingQuery>,
) -> Result<Json<Vec<BookItem>>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let limit = query.limit.unwrap_or(10).clamp(1, 50);

    let rows = sqlx::query(
        r#"
        WITH ongoing_series AS (
            SELECT
                s.id AS series_id,
                MAX(brp.last_read_at) AS series_last_read_at
            FROM series s
            JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
            GROUP BY s.id
            HAVING (
                COUNT(brp.book_id) FILTER (WHERE brp.status IN ('read', 'reading')) > 0
                AND COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') < COUNT(*)
            )
        ),
        next_books AS (
            SELECT
                b.id, b.library_id, b.kind, b.format, b.title, b.author, b.authors, s.name AS series, b.volume,
                b.language, b.page_count, b.thumbnail_path, b.updated_at,
                COALESCE(brp.status, 'unread') AS reading_status,
                brp.current_page AS reading_current_page,
                brp.last_read_at AS reading_last_read_at,
                os.series_last_read_at,
                ROW_NUMBER() OVER (
                    PARTITION BY b.series_id
                    ORDER BY b.volume NULLS LAST, b.title
                ) AS rn
            FROM books b
            JOIN ongoing_series os ON b.series_id = os.series_id
            JOIN series s ON s.id = b.series_id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
            WHERE COALESCE(brp.status, 'unread') != 'read'
        )
        SELECT id, library_id, kind, format, title, author, authors, series, volume, language, page_count,
               thumbnail_path, updated_at, reading_status, reading_current_page, reading_last_read_at
        FROM next_books
        WHERE rn = 1
        ORDER BY series_last_read_at DESC NULLS LAST
        LIMIT $1
        "#,
    )
    .bind(limit)
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    let items: Vec<BookItem> = rows
        .iter()
        .map(|row| {
            let thumbnail_path: Option<String> = row.get("thumbnail_path");
            BookItem {
                id: row.get("id"),
                library_id: row.get("library_id"),
                kind: row.get("kind"),
                format: row.get("format"),
                title: row.get("title"),
                author: row.get("author"),
                authors: row.get::<Vec<String>, _>("authors"),
                series: row.get("series"),
                volume: row.get("volume"),
                language: row.get("language"),
                page_count: row.get("page_count"),
                thumbnail_url: thumbnail_path.map(|_| format!("/books/{}/thumbnail", row.get::<Uuid, _>("id"))),
                updated_at: row.get("updated_at"),
                reading_status: row.get("reading_status"),
                reading_current_page: row.get("reading_current_page"),
                reading_last_read_at: row.get("reading_last_read_at"),
            }
        })
        .collect();

    Ok(Json(items))
}

// ─── Series metadata ────────────────────────────────────────────────────────

/// Get metadata for a specific series (deprecated: use GET /series/{series_id}/metadata)
#[deprecated]
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series/{series_id}/metadata",
    tag = "series (deprecated)",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, body = SeriesMetadata),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_metadata(
    State(state): State<AppState>,
    Path((library_id, series_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SeriesMetadata>, ApiError> {
    // Fetch series row (contains metadata directly)
    let series_row = sqlx::query(
        "SELECT name, authors, description, publishers, start_year, total_volumes, status, locked_fields, book_author, book_language \
         FROM series WHERE id = $1 AND library_id = $2"
    )
    .bind(series_id)
    .bind(library_id)
    .fetch_optional(&state.pool)
    .await?;

    // Fallback: get book_author/book_language from first book if not on series row
    let books_row = sqlx::query("SELECT author, language FROM books WHERE series_id = $1 LIMIT 1")
        .bind(series_id)
        .fetch_optional(&state.pool)
        .await?;

    Ok(Json(SeriesMetadata {
        series_name: series_row.as_ref().map(|r| r.get::<String, _>("name")).unwrap_or_default(),
        authors: series_row.as_ref().map(|r| r.get::<Vec<String>, _>("authors")).unwrap_or_default(),
        description: series_row.as_ref().and_then(|r| r.get("description")),
        publishers: series_row.as_ref().map(|r| r.get::<Vec<String>, _>("publishers")).unwrap_or_default(),
        start_year: series_row.as_ref().and_then(|r| r.get("start_year")),
        total_volumes: series_row.as_ref().and_then(|r| r.get("total_volumes")),
        status: series_row.as_ref().and_then(|r| r.get("status")),
        book_author: series_row.as_ref().and_then(|r| r.get::<Option<String>, _>("book_author"))
            .or_else(|| books_row.as_ref().and_then(|r| r.get("author"))),
        book_language: series_row.as_ref().and_then(|r| r.get::<Option<String>, _>("book_language"))
            .or_else(|| books_row.as_ref().and_then(|r| r.get("language"))),
        locked_fields: series_row.as_ref().map(|r| r.get::<serde_json::Value, _>("locked_fields")).unwrap_or(serde_json::json!({})),
    }))
}

// ─── Direct series-by-ID endpoints (no library_id in path) ──────────────────

/// Get a series by its UUID (resolves library_id internally)
#[utoipa::path(
    get,
    path = "/series/{series_id}/details",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, body = SeriesItem),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_by_id(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesItem>, ApiError> {
    let user_id: Option<Uuid> = user.map(|u| u.0.user_id);

    let row = sqlx::query(
        r#"
        WITH series_counts AS (
            SELECT
                s.id as series_id, s.name, s.library_id, s.status as series_status,
                COUNT(b.id) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
            WHERE s.id = $1
            GROUP BY s.id, s.name, s.library_id, s.status
        ),
        first_book AS (
            SELECT b.id, b.series_id
            FROM books b WHERE b.series_id = $1
            ORDER BY b.volume NULLS LAST, b.title ASC
            LIMIT 1
        )
        SELECT sc.name, sc.series_id, sc.book_count, sc.books_read_count,
               COALESCE(fb.id, '00000000-0000-0000-0000-000000000000'::uuid) as first_book_id,
               sc.library_id, sc.series_status,
               mc.missing_count,
               ml.provider as metadata_provider,
               asl.anilist_id, asl.anilist_url,
               s.cover_url
        FROM series_counts sc
        LEFT JOIN series s ON s.id = sc.series_id
        LEFT JOIN first_book fb ON fb.series_id = sc.series_id
        LEFT JOIN (
            SELECT s2.id as series_id,
                GREATEST(COALESCE(s2.total_volumes, 0) - COUNT(b2.id), 0) as missing_count
            FROM series s2
            LEFT JOIN books b2 ON b2.series_id = s2.id
            WHERE s2.id = $1
            GROUP BY s2.id
        ) mc ON mc.series_id = sc.series_id
        LEFT JOIN (
            SELECT DISTINCT ON (eml.series_id) eml.series_id, eml.provider
            FROM external_metadata_links eml
            WHERE eml.series_id = $1 AND eml.status = 'approved'
            ORDER BY eml.series_id, eml.created_at DESC
        ) ml ON ml.series_id = sc.series_id
        LEFT JOIN anilist_series_links asl ON asl.series_id = sc.series_id AND asl.provider = 'anilist'
        "#
    )
    .bind(series_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("series not found"))?;

    Ok(Json(SeriesItem {
        name: row.get("name"),
        series_id: row.get("series_id"),
        book_count: row.get("book_count"),
        books_read_count: row.get("books_read_count"),
        first_book_id: row.get("first_book_id"),
        library_id: row.get("library_id"),
        series_status: row.get("series_status"),
        missing_count: row.get("missing_count"),
        metadata_provider: row.get("metadata_provider"),
        anilist_id: row.get("anilist_id"),
        anilist_url: row.get("anilist_url"),
        cover_url: row.get("cover_url"),
    }))
}

/// Get metadata for a series by its UUID (resolves library_id internally)
#[utoipa::path(
    get,
    path = "/series/{series_id}/metadata",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, body = SeriesMetadata),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
#[allow(deprecated)]
pub async fn get_series_metadata_by_id(
    state: State<AppState>,
    _user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesMetadata>, ApiError> {
    let library_id = helpers::resolve_library_id(&state.pool, series_id).await?;
    get_series_metadata(state, Path((library_id, series_id))).await
}
