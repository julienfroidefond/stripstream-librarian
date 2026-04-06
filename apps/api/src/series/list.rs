use axum::extract::Extension;
use axum::{extract::{Path, Query, State}, Json};
use sqlx::Row;
use uuid::Uuid;

use crate::{auth::AuthUser, error::ApiError, state::AppState};
use super::{helpers, ListSeriesQuery, ListAllSeriesQuery, SeriesItem, SeriesPage};

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

    // Missing counts CTE
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
