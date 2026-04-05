use axum::extract::Extension;
use axum::{extract::{Path, Query, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{auth::AuthUser, books::BookItem, error::ApiError, state::AppState};

// ─── Helper functions ────────────────────────────────────────────────────────

/// Get or create a series row, returning its UUID.
/// Also checks `original_name` to prevent duplicates after user renames.
pub(crate) async fn get_or_create_series(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    name: &str,
) -> Result<Uuid, ApiError> {
    // Try to find existing by current name OR original_name (prevents duplicates after rename)
    if let Some(id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM series WHERE library_id = $1 \
         AND (LOWER(unaccent(name)) = LOWER(unaccent($2)) \
              OR LOWER(unaccent(original_name)) = LOWER(unaccent($2)))"
    )
    .bind(library_id)
    .bind(name)
    .fetch_optional(pool)
    .await?
    {
        return Ok(id);
    }

    // Create new
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3) \
         ON CONFLICT (library_id, name) DO UPDATE SET name = EXCLUDED.name \
         RETURNING id"
    )
    .bind(id)
    .bind(library_id)
    .bind(name)
    .execute(pool)
    .await?;

    // Re-fetch in case of conflict (ON CONFLICT won't return the existing id via execute)
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM series WHERE library_id = $1 AND LOWER(unaccent(name)) = LOWER(unaccent($2))"
    )
    .bind(library_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

// ─── Lookup by name ──────────────────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct SeriesLookup {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub name: String,
}

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

/// Resolve library_id from a series UUID. Used by the new direct-access endpoints.
pub(crate) async fn resolve_library_id(
    pool: &sqlx::PgPool,
    series_id: Uuid,
) -> Result<Uuid, ApiError> {
    sqlx::query_scalar::<_, Uuid>("SELECT library_id FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("series not found"))
}

// ─── Structs ─────────────────────────────────────────────────────────────────

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

    let missing_cte = r#"
        missing_counts AS (
            SELECT s.id as series_id,
                GREATEST(COALESCE(s.total_volumes, 0) - COUNT(b.id), 0) as missing_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            WHERE s.library_id = $1
            GROUP BY s.id
        )
        "#.to_string();

    let metadata_links_cte = r#"
        metadata_links AS (
            SELECT DISTINCT ON (eml.series_id, eml.library_id)
                eml.series_id, eml.library_id, eml.provider
            FROM external_metadata_links eml
            WHERE eml.status = 'approved'
            ORDER BY eml.series_id, eml.library_id, eml.created_at DESC
        )
    "#;

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
        r#"
            missing_counts AS (
                SELECT s.id as series_id,
                    GREATEST(COALESCE(s.total_volumes, 0) - COUNT(b.id), 0) as missing_count
                FROM series s
                LEFT JOIN books b ON b.series_id = s.id
                WHERE s.library_id = $1
                GROUP BY s.id
            )
            "#.to_string()
    } else {
        r#"
        missing_counts AS (
            SELECT s.id as series_id,
                GREATEST(COALESCE(s.total_volumes, 0) - COUNT(b.id), 0) as missing_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            GROUP BY s.id
        )
        "#.to_string()
    };

    let metadata_links_cte = r#"
        metadata_links AS (
            SELECT DISTINCT ON (eml.series_id, eml.library_id)
                eml.series_id, eml.library_id, eml.provider
            FROM external_metadata_links eml
            WHERE eml.status = 'approved'
            ORDER BY eml.series_id, eml.library_id, eml.created_at DESC
        )
    "#;

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

#[derive(Deserialize, ToSchema)]
pub struct OngoingQuery {
    #[schema(value_type = Option<i64>, example = 10)]
    pub limit: Option<i64>,
}

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

/// `author` and `language` are wrapped in an extra Option so we can distinguish
/// "absent from JSON" (keep books unchanged) from "present as null" (clear the field).
#[derive(Deserialize, ToSchema)]
pub struct UpdateSeriesRequest {
    pub new_name: String,
    /// Series-level authors list (stored in series)
    #[serde(default)]
    pub authors: Vec<String>,
    /// Per-book author propagation: absent = keep books unchanged, present = overwrite all books
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Option<String>>,
    /// Per-book language propagation: absent = keep books unchanged, present = overwrite all books
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<Option<String>>,
    pub description: Option<String>,
    #[serde(default)]
    pub publishers: Vec<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    /// Series status: "ongoing", "ended", "hiatus", "cancelled", or null
    pub status: Option<String>,
    /// Fields locked from external metadata sync
    #[serde(default)]
    pub locked_fields: Option<serde_json::Value>,
}

#[derive(Serialize, ToSchema)]
pub struct UpdateSeriesResponse {
    pub updated: u64,
}

/// Update metadata for all books in a series (deprecated: use PATCH /series/{series_id})
#[deprecated]
#[utoipa::path(
    patch,
    path = "/libraries/{library_id}/series/{series_id}",
    tag = "series (deprecated)",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    request_body = UpdateSeriesRequest,
    responses(
        (status = 200, body = UpdateSeriesResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_series(
    State(state): State<AppState>,
    Path((library_id, series_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateSeriesRequest>,
) -> Result<Json<UpdateSeriesResponse>, ApiError> {
    let new_name = body.new_name.trim().to_string();
    if new_name.is_empty() {
        return Err(ApiError::bad_request("series name cannot be empty"));
    }

    // Verify the series exists
    let old_row = sqlx::query("SELECT name, original_name FROM series WHERE id = $1 AND library_id = $2")
        .bind(series_id)
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("series not found"))?;
    let old_name: String = old_row.get("name");

    // author/language: None = absent (keep books unchanged), Some(v) = apply to all books
    let apply_author = body.author.is_some();
    let author_value = body.author.flatten().as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let apply_language = body.language.is_some();
    let language_value = body.language.flatten().as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let description = body.description.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let publishers: Vec<String> = body.publishers.iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    let authors: Vec<String> = body.authors.iter()
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect();
    let locked_fields = body.locked_fields.clone().unwrap_or(serde_json::json!({}));

    // 1. Update books: author/language only if opted-in
    let result = sqlx::query(
        "UPDATE books \
         SET author = CASE WHEN $2 THEN $3 ELSE author END, \
             language = CASE WHEN $4 THEN $5 ELSE language END, \
             updated_at = NOW() \
         WHERE series_id = $1"
    )
    .bind(series_id)
    .bind(apply_author)
    .bind(&author_value)
    .bind(apply_language)
    .bind(&language_value)
    .execute(&state.pool)
    .await?;

    // 2. Update the series row (name, metadata, original_name tracking)
    let is_rename = new_name != old_name;
    let original_name: Option<String> = if is_rename {
        // Use existing original_name if set (chained renames: A->B->C), otherwise use old name
        let existing_original: Option<String> = old_row.get("original_name");
        Some(existing_original.unwrap_or_else(|| old_name.clone()))
    } else {
        None
    };

    sqlx::query(
        r#"
        UPDATE series
        SET name = $2,
            authors = $3,
            description = $4,
            publishers = $5,
            start_year = $6,
            total_volumes = $7,
            status = $8,
            locked_fields = $9,
            book_author = CASE WHEN $10 THEN $11 ELSE book_author END,
            book_language = CASE WHEN $12 THEN $13 ELSE book_language END,
            original_name = COALESCE($14, original_name),
            updated_at = NOW()
        WHERE id = $1
        "#
    )
    .bind(series_id)
    .bind(&new_name)
    .bind(&authors)
    .bind(&description)
    .bind(&publishers)
    .bind(body.start_year)
    .bind(body.total_volumes)
    .bind(&body.status)
    .bind(&locked_fields)
    .bind(apply_author)
    .bind(&author_value)
    .bind(apply_language)
    .bind(&language_value)
    .bind(&original_name)
    .execute(&state.pool)
    .await?;

    Ok(Json(UpdateSeriesResponse { updated: result.rows_affected() }))
}

// ─── Merge series ──────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct MergeSeriesRequest {
    /// The series to absorb (will be deleted after merge)
    pub source_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub struct MergeSeriesResponse {
    pub books_moved: u64,
    pub metadata_moved: u64,
    pub downloads_moved: u64,
}

/// Merge source series into target series.
/// Moves books, metadata links, and available downloads from source to target,
/// then deletes the source series.
#[utoipa::path(
    post,
    path = "/series/{target_id}/merge",
    tag = "series",
    params(
        ("target_id" = String, Path, description = "Target series UUID (kept)"),
    ),
    request_body = MergeSeriesRequest,
    responses(
        (status = 200, body = MergeSeriesResponse),
        (status = 400, description = "Cannot merge a series into itself"),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn merge_series(
    State(state): State<AppState>,
    _user: Option<Extension<AuthUser>>,
    Path(target_id): Path<Uuid>,
    Json(body): Json<MergeSeriesRequest>,
) -> Result<Json<MergeSeriesResponse>, ApiError> {
    let source_id = body.source_id;

    if target_id == source_id {
        return Err(ApiError::bad_request("Cannot merge a series into itself"));
    }

    // Verify both series exist and are in the same library
    let target_lib: Uuid = resolve_library_id(&state.pool, target_id).await?;
    let source_lib: Uuid = resolve_library_id(&state.pool, source_id).await?;
    if target_lib != source_lib {
        return Err(ApiError::bad_request("Cannot merge series from different libraries"));
    }

    let mut tx = state.pool.begin().await?;

    // 1. Move books from source to target
    let books_result = sqlx::query("UPDATE books SET series_id = $1 WHERE series_id = $2")
        .bind(target_id)
        .bind(source_id)
        .execute(&mut *tx)
        .await?;
    let books_moved = books_result.rows_affected();

    // 2. Move metadata links that target doesn't already have (by provider)
    let metadata_result = sqlx::query(
        "UPDATE external_metadata_links SET series_id = $1 \
         WHERE series_id = $2 \
         AND provider NOT IN (SELECT provider FROM external_metadata_links WHERE series_id = $1)",
    )
    .bind(target_id)
    .bind(source_id)
    .execute(&mut *tx)
    .await?;
    let metadata_moved = metadata_result.rows_affected();

    // Delete remaining source metadata links (duplicates of target's providers)
    sqlx::query("DELETE FROM external_metadata_links WHERE series_id = $1")
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

    // 3. Move available downloads that target doesn't already have
    let downloads_result = sqlx::query(
        "UPDATE available_downloads SET series_id = $1 \
         WHERE series_id = $2 \
         AND id NOT IN ( \
             SELECT ad2.id FROM available_downloads ad2 \
             WHERE ad2.series_id = $1 \
         )",
    )
    .bind(target_id)
    .bind(source_id)
    .execute(&mut *tx)
    .await?;
    let downloads_moved = downloads_result.rows_affected();

    // Delete remaining source downloads
    sqlx::query("DELETE FROM available_downloads WHERE series_id = $1")
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

    // 4. Move anilist links if target doesn't have one
    sqlx::query(
        "UPDATE anilist_series_links SET series_id = $1 \
         WHERE series_id = $2 \
         AND NOT EXISTS (SELECT 1 FROM anilist_series_links WHERE series_id = $1)",
    )
    .bind(target_id)
    .bind(source_id)
    .execute(&mut *tx)
    .await?;

    // Delete remaining source anilist links
    sqlx::query("DELETE FROM anilist_series_links WHERE series_id = $1")
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

    // 5. Delete the source series
    sqlx::query("DELETE FROM series WHERE id = $1")
        .bind(source_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Json(MergeSeriesResponse {
        books_moved,
        metadata_moved,
        downloads_moved,
    }))
}

/// Delete an entire series (deprecated: use DELETE /series/{series_id})
#[deprecated]
#[utoipa::path(
    delete,
    path = "/libraries/{library_id}/series/{series_id}",
    tag = "series (deprecated)",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, description = "Series deleted"),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_series(
    State(state): State<AppState>,
    _user: Option<Extension<AuthUser>>,
    Path((library_id, series_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<crate::responses::DeletedResponse>, ApiError> {
    use stripstream_core::paths::remap_libraries_path;

    // Verify the series exists
    let series_row = sqlx::query("SELECT name FROM series WHERE id = $1 AND library_id = $2")
        .bind(series_id)
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("series not found"))?;
    let series_name: String = series_row.get("name");

    // Find all books in this series
    let book_rows = sqlx::query(
        "SELECT b.id, b.thumbnail_path, bf.abs_path \
         FROM books b \
         LEFT JOIN book_files bf ON bf.book_id = b.id \
         WHERE b.series_id = $1",
    )
    .bind(series_id)
    .fetch_all(&state.pool)
    .await?;

    if book_rows.is_empty() {
        // Series exists but has no books — still delete the series row
    }

    // Collect the series directory from the first book's path
    let mut series_dir: Option<String> = None;

    // Delete each book's physical file and thumbnail
    for row in &book_rows {
        let abs_path: Option<String> = row.get("abs_path");
        let thumbnail_path: Option<String> = row.get("thumbnail_path");

        if let Some(ref path) = abs_path {
            let physical = remap_libraries_path(path);
            if series_dir.is_none() {
                if let Some(parent) = std::path::Path::new(&physical).parent() {
                    series_dir = Some(parent.to_string_lossy().into_owned());
                }
            }
            match std::fs::remove_file(&physical) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    tracing::warn!("[SERIES] Failed to delete file {}: {}", physical, e);
                }
            }
        }

        if let Some(ref path) = thumbnail_path {
            let _ = std::fs::remove_file(path);
        }
    }

    // Delete the series directory if it's now empty (or only has non-book files)
    if let Some(ref dir) = series_dir {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => tracing::info!("[SERIES] Deleted series directory: {}", dir),
            Err(e) => tracing::warn!("[SERIES] Failed to delete series directory {}: {}", dir, e),
        }
    }

    // Delete all books from DB (cascades to book_files, reading_progress, etc.)
    let book_ids: Vec<Uuid> = book_rows.iter().map(|r| r.get("id")).collect();
    if !book_ids.is_empty() {
        sqlx::query("DELETE FROM books WHERE id = ANY($1)")
            .bind(&book_ids)
            .execute(&state.pool)
            .await?;
    }

    // Delete the series row (cascades to external_metadata_links, anilist_series_links, available_downloads via FK)
    sqlx::query("DELETE FROM series WHERE id = $1")
        .bind(series_id)
        .execute(&state.pool)
        .await?;

    // Queue a scan job for consistency
    let scan_job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
    )
    .bind(scan_job_id)
    .bind(library_id)
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "[SERIES] Deleted series '{}' ({}) ({} books) from library {}, scan job {} queued",
        series_name, series_id, book_ids.len(), library_id, scan_job_id
    );

    Ok(Json(crate::responses::DeletedResponse::new(library_id)))
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
            SELECT eml.series_id, COUNT(ebm.id) FILTER (WHERE ebm.book_id IS NULL) as missing_count
            FROM external_metadata_links eml
            JOIN external_book_metadata ebm ON ebm.link_id = eml.id
            WHERE eml.series_id = $1 AND eml.status = 'approved'
            GROUP BY eml.series_id
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
    let library_id = resolve_library_id(&state.pool, series_id).await?;
    get_series_metadata(state, Path((library_id, series_id))).await
}

/// Update a series by its UUID (resolves library_id internally)
#[utoipa::path(
    patch,
    path = "/series/{series_id}",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    request_body = UpdateSeriesRequest,
    responses(
        (status = 200, body = UpdateSeriesResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
#[allow(deprecated)]
pub async fn update_series_by_id(
    state: State<AppState>,
    Path(series_id): Path<Uuid>,
    body: Json<UpdateSeriesRequest>,
) -> Result<Json<UpdateSeriesResponse>, ApiError> {
    let library_id = resolve_library_id(&state.pool, series_id).await?;
    update_series(state, Path((library_id, series_id)), body).await
}

/// Delete a series by its UUID (resolves library_id internally)
#[utoipa::path(
    delete,
    path = "/series/{series_id}",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, description = "Series deleted"),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
#[allow(deprecated)]
pub async fn delete_series_by_id(
    state: State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<crate::responses::DeletedResponse>, ApiError> {
    let library_id = resolve_library_id(&state.pool, series_id).await?;
    delete_series(state, user, Path((library_id, series_id))).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_item_has_series_id() {
        let item = SeriesItem {
            name: "Dragon Ball".to_string(),
            series_id: Uuid::new_v4(),
            book_count: 42,
            books_read_count: 10,
            first_book_id: Some(Uuid::new_v4()),
            library_id: Uuid::new_v4(),
            series_status: Some("ended".to_string()),
            missing_count: Some(0),
            metadata_provider: None,
            anilist_id: None,
            anilist_url: None,
            cover_url: None,
        };
        let json = serde_json::to_value(&item).unwrap();
        assert!(json["series_id"].is_string());
        assert_eq!(json["name"], "Dragon Ball");
        assert_eq!(json["book_count"], 42);
    }

    #[test]
    fn series_metadata_serializes() {
        let meta = SeriesMetadata {
            series_name: "Naruto".to_string(),
            description: Some("A ninja story".to_string()),
            authors: vec!["Kishimoto".to_string()],
            publishers: vec![],
            book_author: None,
            book_language: None,
            start_year: Some(1999),
            total_volumes: Some(72),
            status: Some("ended".to_string()),
            locked_fields: serde_json::json!({}),
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["total_volumes"], 72);
        assert_eq!(json["authors"][0], "Kishimoto");
        assert_eq!(json["status"], "ended");
    }

    #[test]
    fn update_series_response_serializes() {
        let resp = UpdateSeriesResponse { updated: 5 };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["updated"], 5);
    }

    #[test]
    fn series_item_includes_library_id() {
        let lib_id = Uuid::new_v4();
        let item = SeriesItem {
            name: "One Piece".to_string(),
            series_id: Uuid::new_v4(),
            book_count: 100,
            books_read_count: 50,
            first_book_id: Some(Uuid::new_v4()),
            library_id: lib_id,
            series_status: Some("ongoing".to_string()),
            missing_count: Some(5),
            metadata_provider: Some("google_books".to_string()),
            anilist_id: Some(12345),
            anilist_url: Some("https://anilist.co/manga/12345".to_string()),
            cover_url: None,
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["library_id"], lib_id.to_string());
        assert_eq!(json["series_status"], "ongoing");
        assert_eq!(json["missing_count"], 5);
        assert_eq!(json["metadata_provider"], "google_books");
        assert_eq!(json["anilist_id"], 12345);
    }

    // ─── Integration tests (require PostgreSQL) ─────────────────────────

    /// Helper to create a test library in the DB.
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

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_new(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let id = get_or_create_series(&pool, lib_id, "Dragon Ball").await.unwrap();
        assert_ne!(id, Uuid::nil());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_idempotent(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let id1 = get_or_create_series(&pool, lib_id, "Dragon Ball").await.unwrap();
        let id2 = get_or_create_series(&pool, lib_id, "Dragon Ball").await.unwrap();
        assert_eq!(id1, id2);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_case_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let id1 = get_or_create_series(&pool, lib_id, "Dragon Ball").await.unwrap();
        let id2 = get_or_create_series(&pool, lib_id, "dragon ball").await.unwrap();
        assert_eq!(id1, id2, "same series with different casing should return same id");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_accent_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let id1 = get_or_create_series(&pool, lib_id, "Astérix").await.unwrap();
        let id2 = get_or_create_series(&pool, lib_id, "Asterix").await.unwrap();
        assert_eq!(id1, id2, "accented and unaccented names should match");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_different_libraries(pool: sqlx::PgPool) {
        let lib1 = create_test_library(&pool, "lib1").await;
        let lib2 = create_test_library(&pool, "lib2").await;
        let id1 = get_or_create_series(&pool, lib1, "Naruto").await.unwrap();
        let id2 = get_or_create_series(&pool, lib2, "Naruto").await.unwrap();
        assert_ne!(id1, id2, "same name in different libraries should be different series");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_finds_by_original_name(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "rename_test").await;

        // Create series "Dragon Ball" then rename to "Dragon Ball Z" (simulating user rename)
        let id1 = get_or_create_series(&pool, lib_id, "Dragon Ball").await.unwrap();
        sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
            .bind("Dragon Ball Z")
            .bind("Dragon Ball")
            .bind(id1)
            .execute(&pool)
            .await
            .unwrap();

        // Looking up old name should find the renamed series
        let id2 = get_or_create_series(&pool, lib_id, "Dragon Ball").await.unwrap();
        assert_eq!(id1, id2, "lookup by original_name should return the renamed series");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_original_name_case_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "rename_case_test").await;

        let id1 = get_or_create_series(&pool, lib_id, "LES MYTHICS").await.unwrap();
        sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
            .bind("Mythics")
            .bind("LES MYTHICS")
            .bind(id1)
            .execute(&pool)
            .await
            .unwrap();

        // Case-insensitive lookup by original_name
        let id2 = get_or_create_series(&pool, lib_id, "les mythics").await.unwrap();
        assert_eq!(id1, id2, "original_name lookup should be case-insensitive");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_chained_rename(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "chained_rename").await;

        // A → B → C : original_name stays "A"
        let id1 = get_or_create_series(&pool, lib_id, "Series A").await.unwrap();
        sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
            .bind("Series C")
            .bind("Series A")
            .bind(id1)
            .execute(&pool)
            .await
            .unwrap();

        // Lookup by original name "Series A" should still find it
        let id2 = get_or_create_series(&pool, lib_id, "Series A").await.unwrap();
        assert_eq!(id1, id2, "chained rename: original_name should still match");

        // Lookup by current name "Series C" should also work
        let id3 = get_or_create_series(&pool, lib_id, "Series C").await.unwrap();
        assert_eq!(id1, id3, "current name should also match");
    }

    // ─── Merge series tests ────────────────────────────────────────────

    /// Helper to create a series directly in DB.
    async fn create_series(pool: &sqlx::PgPool, lib_id: Uuid, name: &str) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO series (id, library_id, name) VALUES (gen_random_uuid(), $1, $2) RETURNING id",
        )
        .bind(lib_id)
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// Helper to create a book assigned to a series.
    async fn create_book(pool: &sqlx::PgPool, lib_id: Uuid, series_id: Uuid, title: &str) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO books (id, library_id, title, kind, format, series_id) \
             VALUES (gen_random_uuid(), $1, $2, 'comic', 'cbz', $3) RETURNING id",
        )
        .bind(lib_id)
        .bind(title)
        .bind(series_id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn merge_moves_books_to_target(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "merge_books").await;
        let target = create_series(&pool, lib_id, "Target").await;
        let source = create_series(&pool, lib_id, "Source").await;

        create_book(&pool, lib_id, target, "Book A").await;
        create_book(&pool, lib_id, source, "Book B").await;
        create_book(&pool, lib_id, source, "Book C").await;

        // Merge source into target
        let moved = sqlx::query("UPDATE books SET series_id = $1 WHERE series_id = $2")
            .bind(target)
            .bind(source)
            .execute(&pool)
            .await
            .unwrap()
            .rows_affected();
        assert_eq!(moved, 2);

        // Target now has 3 books
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = $1")
            .bind(target)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 3);

        // Source has 0 books
        let source_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = $1")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(source_count, 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn merge_moves_metadata_links(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "merge_meta").await;
        let target = create_series(&pool, lib_id, "Target").await;
        let source = create_series(&pool, lib_id, "Source").await;

        // Source has a senscritique link, target has none
        sqlx::query(
            "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id) \
             VALUES ($1, $2, 'senscritique', '123')",
        )
        .bind(lib_id)
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();

        // Move metadata links where target doesn't already have one for that provider
        let moved = sqlx::query(
            "UPDATE external_metadata_links SET series_id = $1 \
             WHERE series_id = $2 \
             AND provider NOT IN (SELECT provider FROM external_metadata_links WHERE series_id = $1)",
        )
        .bind(target)
        .bind(source)
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected();
        assert_eq!(moved, 1);

        // Target now has the link
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1",
        )
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn merge_keeps_target_metadata_on_conflict(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "merge_meta_conflict").await;
        let target = create_series(&pool, lib_id, "Target").await;
        let source = create_series(&pool, lib_id, "Source").await;

        // Both have a senscritique link
        for (sid, ext_id) in [(target, "target_123"), (source, "source_456")] {
            sqlx::query(
                "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id) \
                 VALUES ($1, $2, 'senscritique', $3)",
            )
            .bind(lib_id)
            .bind(sid)
            .bind(ext_id)
            .execute(&pool)
            .await
            .unwrap();
        }

        // Move only non-conflicting providers
        let moved = sqlx::query(
            "UPDATE external_metadata_links SET series_id = $1 \
             WHERE series_id = $2 \
             AND provider NOT IN (SELECT provider FROM external_metadata_links WHERE series_id = $1)",
        )
        .bind(target)
        .bind(source)
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected();
        assert_eq!(moved, 0, "source link should NOT be moved (target already has senscritique)");

        // Delete remaining source links
        sqlx::query("DELETE FROM external_metadata_links WHERE series_id = $1")
            .bind(source)
            .execute(&pool)
            .await
            .unwrap();

        // Target still has its original link
        let ext_id: String = sqlx::query_scalar(
            "SELECT external_id FROM external_metadata_links WHERE series_id = $1 AND provider = 'senscritique'",
        )
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(ext_id, "target_123", "target's original link should be preserved");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn merge_deletes_source_series(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "merge_delete").await;
        let target = create_series(&pool, lib_id, "Target").await;
        let source = create_series(&pool, lib_id, "Source").await;

        // Move books (none in this case)
        sqlx::query("UPDATE books SET series_id = $1 WHERE series_id = $2")
            .bind(target)
            .bind(source)
            .execute(&pool)
            .await
            .unwrap();

        // Delete source
        sqlx::query("DELETE FROM series WHERE id = $1")
            .bind(source)
            .execute(&pool)
            .await
            .unwrap();

        // Source no longer exists
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(!exists, "source series should be deleted");

        // Target still exists
        let target_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
            .bind(target)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(target_exists, "target series should still exist");
    }

    // ─── Missing count tests ───────────────────────────────────────────

    /// Helper: run the missing_counts CTE for a single series and return missing_count.
    async fn query_missing_count(pool: &sqlx::PgPool, lib_id: Uuid, series_id: Uuid) -> i64 {
        sqlx::query_scalar::<_, i64>(
            r#"
            WITH missing_counts AS (
                SELECT s.id as series_id,
                    GREATEST(COALESCE(s.total_volumes, 0) - COUNT(b.id), 0) as missing_count
                FROM series s
                LEFT JOIN books b ON b.series_id = s.id
                WHERE s.library_id = $1
                GROUP BY s.id
            )
            SELECT COALESCE(mc.missing_count, 0) FROM missing_counts mc WHERE mc.series_id = $2
            "#,
        )
        .bind(lib_id)
        .bind(series_id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn missing_count_with_total_volumes_and_books(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "missing_test").await;
        let sid = create_series(&pool, lib_id, "Naruto").await;

        // Set total_volumes = 10
        sqlx::query("UPDATE series SET total_volumes = 10 WHERE id = $1")
            .bind(sid).execute(&pool).await.unwrap();

        // Add 7 books
        for i in 1..=7 {
            create_book(&pool, lib_id, sid, &format!("Vol {i}")).await;
        }

        let missing = query_missing_count(&pool, lib_id, sid).await;
        assert_eq!(missing, 3, "10 total - 7 books = 3 missing");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn missing_count_zero_when_no_total_volumes(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "missing_null").await;
        let sid = create_series(&pool, lib_id, "Unknown").await;

        // total_volumes is NULL, 5 books
        for i in 1..=5 {
            create_book(&pool, lib_id, sid, &format!("Vol {i}")).await;
        }

        let missing = query_missing_count(&pool, lib_id, sid).await;
        assert_eq!(missing, 0, "NULL total_volumes => 0 missing");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn missing_count_zero_when_complete(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "missing_complete").await;
        let sid = create_series(&pool, lib_id, "Complete").await;

        sqlx::query("UPDATE series SET total_volumes = 3 WHERE id = $1")
            .bind(sid).execute(&pool).await.unwrap();

        for i in 1..=3 {
            create_book(&pool, lib_id, sid, &format!("Vol {i}")).await;
        }

        let missing = query_missing_count(&pool, lib_id, sid).await;
        assert_eq!(missing, 0, "3 total - 3 books = 0 missing");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn missing_count_zero_when_more_books_than_total(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "missing_over").await;
        let sid = create_series(&pool, lib_id, "Overflow").await;

        sqlx::query("UPDATE series SET total_volumes = 2 WHERE id = $1")
            .bind(sid).execute(&pool).await.unwrap();

        for i in 1..=5 {
            create_book(&pool, lib_id, sid, &format!("Vol {i}")).await;
        }

        let missing = query_missing_count(&pool, lib_id, sid).await;
        assert_eq!(missing, 0, "GREATEST(2 - 5, 0) = 0, not negative");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn missing_count_updates_after_manual_edit(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "missing_edit").await;
        let sid = create_series(&pool, lib_id, "Edited").await;

        sqlx::query("UPDATE series SET total_volumes = 5 WHERE id = $1")
            .bind(sid).execute(&pool).await.unwrap();

        create_book(&pool, lib_id, sid, "Vol 1").await;
        create_book(&pool, lib_id, sid, "Vol 2").await;

        assert_eq!(query_missing_count(&pool, lib_id, sid).await, 3, "5 - 2 = 3");

        // User manually changes total_volumes to 10
        sqlx::query("UPDATE series SET total_volumes = 10 WHERE id = $1")
            .bind(sid).execute(&pool).await.unwrap();

        assert_eq!(query_missing_count(&pool, lib_id, sid).await, 8, "10 - 2 = 8");
    }
}
