use axum::extract::Extension;
use axum::{extract::{Path, Query, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{auth::AuthUser, books::BookItem, error::ApiError, state::AppState};

#[derive(Serialize, ToSchema)]
pub struct SeriesItem {
    pub name: String,
    pub book_count: i64,
    pub books_read_count: i64,
    #[schema(value_type = String)]
    pub first_book_id: Uuid,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub series_status: Option<String>,
    pub missing_count: Option<i64>,
    pub metadata_provider: Option<String>,
    pub anilist_id: Option<i32>,
    pub anilist_url: Option<String>,
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
        p += 1; format!("AND sc.name ILIKE ${p}")
    } else { String::new() };

    let count_rs_cond = if reading_statuses.is_some() {
        p += 1; format!("AND {series_status_expr} = ANY(${p})")
    } else { String::new() };

    let ss_cond = if query.series_status.is_some() {
        p += 1; format!("AND LOWER(sm.status) = ${p}")
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
            SELECT eml.series_name,
                COUNT(ebm.id) FILTER (WHERE ebm.book_id IS NULL) as missing_count
            FROM external_metadata_links eml
            JOIN external_book_metadata ebm ON ebm.link_id = eml.id
            WHERE eml.library_id = $1 AND eml.status = 'approved'
            GROUP BY eml.series_name
        )
        "#.to_string();

    let metadata_links_cte = r#"
        metadata_links AS (
            SELECT DISTINCT ON (eml.series_name, eml.library_id)
                eml.series_name, eml.library_id, eml.provider
            FROM external_metadata_links eml
            WHERE eml.status = 'approved'
            ORDER BY eml.series_name, eml.library_id, eml.created_at DESC
        )
    "#;

    let count_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT COALESCE(NULLIF(series, ''), 'unclassified') as name, id
            FROM books WHERE library_id = $1
        ),
        series_counts AS (
            SELECT sb.name,
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM sorted_books sb
            LEFT JOIN book_reading_progress brp ON brp.book_id = sb.id AND ${user_id_p}::uuid IS NOT NULL AND brp.user_id = ${user_id_p}
            GROUP BY sb.name
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT COUNT(*) FROM series_counts sc
        LEFT JOIN series_metadata sm ON sm.library_id = $1 AND sm.name = sc.name
        LEFT JOIN missing_counts mc ON mc.series_name = sc.name
        LEFT JOIN metadata_links ml ON ml.series_name = sc.name AND ml.library_id = $1
        WHERE TRUE {q_cond} {count_rs_cond} {ss_cond} {missing_cond} {metadata_provider_cond}
        "#
    );

    let data_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT
                COALESCE(NULLIF(series, ''), 'unclassified') as name,
                id,
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(series, ''), 'unclassified')
                    ORDER BY
                        volume NULLS LAST,
                        REGEXP_REPLACE(LOWER(title), '[0-9].*$', ''),
                        COALESCE((REGEXP_MATCH(LOWER(title), '\d+'))[1]::int, 0),
                        title ASC
                ) as rn
            FROM books
            WHERE library_id = $1
        ),
        series_counts AS (
            SELECT
                sb.name,
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM sorted_books sb
            LEFT JOIN book_reading_progress brp ON brp.book_id = sb.id AND ${user_id_p}::uuid IS NOT NULL AND brp.user_id = ${user_id_p}
            GROUP BY sb.name
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT
            sc.name,
            sc.book_count,
            sc.books_read_count,
            sb.id as first_book_id,
            sm.status as series_status,
            mc.missing_count,
            ml.provider as metadata_provider,
            asl.anilist_id,
            asl.anilist_url
        FROM series_counts sc
        JOIN sorted_books sb ON sb.name = sc.name AND sb.rn = 1
        LEFT JOIN series_metadata sm ON sm.library_id = $1 AND sm.name = sc.name
        LEFT JOIN missing_counts mc ON mc.series_name = sc.name
        LEFT JOIN metadata_links ml ON ml.series_name = sc.name AND ml.library_id = $1
        LEFT JOIN anilist_series_links asl ON asl.library_id = $1 AND asl.series_name = sc.name AND asl.provider = 'anilist'
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
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
            library_id,
            series_status: row.get("series_status"),
            missing_count: row.get("missing_count"),
            metadata_provider: row.get("metadata_provider"),
            anilist_id: row.get("anilist_id"),
            anilist_url: row.get("anilist_url"),
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
    /// Filter by author name (matches in series_metadata.authors or book-level authors)
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
        ("author" = Option<String>, Query, description = "Filter by author name (matches in series_metadata.authors or book-level authors)"),
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
        p += 1; format!("WHERE library_id = ${p}")
    } else {
        "WHERE TRUE".to_string()
    };

    let q_cond = if query.q.is_some() {
        p += 1; format!("AND sc.name ILIKE ${p}")
    } else { String::new() };

    let rs_cond = if reading_statuses.is_some() {
        p += 1; format!("AND {series_status_expr} = ANY(${p})")
    } else { String::new() };

    let ss_cond = if query.series_status.is_some() {
        p += 1; format!("AND LOWER(sm.status) = ${p}")
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
        p += 1; format!("AND (${p} = ANY(sm.authors) OR EXISTS (SELECT 1 FROM books bk WHERE bk.series = sc.name AND bk.library_id = sc.library_id AND ${p} = ANY(COALESCE(NULLIF(bk.authors, '{{}}'), CASE WHEN bk.author IS NOT NULL AND bk.author != '' THEN ARRAY[bk.author] ELSE ARRAY[]::text[] END))))")
    } else { String::new() };

    // Missing counts CTE — needs library_id filter when filtering by library
    let missing_cte = if query.library_id.is_some() {
        r#"
            missing_counts AS (
                SELECT eml.series_name, eml.library_id,
                    COUNT(ebm.id) FILTER (WHERE ebm.book_id IS NULL) as missing_count
                FROM external_metadata_links eml
                JOIN external_book_metadata ebm ON ebm.link_id = eml.id
                WHERE eml.library_id = $1 AND eml.status = 'approved'
                GROUP BY eml.series_name, eml.library_id
            )
            "#.to_string()
    } else {
        r#"
        missing_counts AS (
            SELECT eml.series_name, eml.library_id,
                COUNT(ebm.id) FILTER (WHERE ebm.book_id IS NULL) as missing_count
            FROM external_metadata_links eml
            JOIN external_book_metadata ebm ON ebm.link_id = eml.id
            WHERE eml.status = 'approved'
            GROUP BY eml.series_name, eml.library_id
        )
        "#.to_string()
    };

    let metadata_links_cte = r#"
        metadata_links AS (
            SELECT DISTINCT ON (eml.series_name, eml.library_id)
                eml.series_name, eml.library_id, eml.provider
            FROM external_metadata_links eml
            WHERE eml.status = 'approved'
            ORDER BY eml.series_name, eml.library_id, eml.created_at DESC
        )
    "#;

    let user_id_p = p + 1;
    let limit_p = p + 2;
    let offset_p = p + 3;

    let count_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT COALESCE(NULLIF(series, ''), 'unclassified') as name, id, library_id
            FROM books {lib_cond}
        ),
        series_counts AS (
            SELECT sb.name, sb.library_id,
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM sorted_books sb
            LEFT JOIN book_reading_progress brp ON brp.book_id = sb.id AND ${user_id_p}::uuid IS NOT NULL AND brp.user_id = ${user_id_p}
            GROUP BY sb.name, sb.library_id
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT COUNT(*) FROM series_counts sc
        LEFT JOIN series_metadata sm ON sm.library_id = sc.library_id AND sm.name = sc.name
        LEFT JOIN missing_counts mc ON mc.series_name = sc.name AND mc.library_id = sc.library_id
        LEFT JOIN metadata_links ml ON ml.series_name = sc.name AND ml.library_id = sc.library_id
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
                COALESCE(NULLIF(series, ''), 'unclassified') as name,
                id,
                library_id,
                created_at,
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(series, ''), 'unclassified')
                    ORDER BY
                        volume NULLS LAST,
                        REGEXP_REPLACE(LOWER(title), '[0-9].*$', ''),
                        COALESCE((REGEXP_MATCH(LOWER(title), '\d+'))[1]::int, 0),
                        title ASC
                ) as rn
            FROM books
            {lib_cond}
        ),
        series_counts AS (
            SELECT
                sb.name,
                sb.library_id,
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count,
                MAX(sb.created_at) as latest_created_at
            FROM sorted_books sb
            LEFT JOIN book_reading_progress brp ON brp.book_id = sb.id AND ${user_id_p}::uuid IS NOT NULL AND brp.user_id = ${user_id_p}
            GROUP BY sb.name, sb.library_id
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT
            sc.name,
            sc.book_count,
            sc.books_read_count,
            sb.id as first_book_id,
            sb.library_id,
            sm.status as series_status,
            mc.missing_count,
            ml.provider as metadata_provider,
            asl.anilist_id,
            asl.anilist_url
        FROM series_counts sc
        JOIN sorted_books sb ON sb.name = sc.name AND sb.rn = 1
        LEFT JOIN series_metadata sm ON sm.library_id = sc.library_id AND sm.name = sc.name
        LEFT JOIN missing_counts mc ON mc.series_name = sc.name AND mc.library_id = sc.library_id
        LEFT JOIN metadata_links ml ON ml.series_name = sc.name AND ml.library_id = sc.library_id
        LEFT JOIN anilist_series_links asl ON asl.library_id = sc.library_id AND asl.series_name = sc.name AND asl.provider = 'anilist'
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
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
            library_id: row.get("library_id"),
            series_status: row.get("series_status"),
            missing_count: row.get("missing_count"),
            metadata_provider: row.get("metadata_provider"),
            anilist_id: row.get("anilist_id"),
            anilist_url: row.get("anilist_url"),
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
            SELECT LOWER(status) AS s FROM series_metadata WHERE status IS NOT NULL
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
                COALESCE(NULLIF(b.series, ''), 'unclassified') AS name,
                COUNT(*) AS book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') AS books_read_count,
                MAX(brp.last_read_at) AS last_read_at
            FROM books b
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
            GROUP BY COALESCE(NULLIF(b.series, ''), 'unclassified')
            HAVING (
                COUNT(brp.book_id) FILTER (WHERE brp.status IN ('read', 'reading')) > 0
                AND COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') < COUNT(*)
            )
        ),
        first_books AS (
            SELECT
                COALESCE(NULLIF(series, ''), 'unclassified') AS name,
                id,
                library_id,
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(series, ''), 'unclassified')
                    ORDER BY
                        volume NULLS LAST,
                        REGEXP_REPLACE(LOWER(title), '[0-9].*$', ''),
                        COALESCE((REGEXP_MATCH(LOWER(title), '\d+'))[1]::int, 0),
                        title ASC
                ) AS rn
            FROM books
        )
        SELECT ss.name, ss.book_count, ss.books_read_count, fb.id AS first_book_id, fb.library_id
        FROM series_stats ss
        JOIN first_books fb ON fb.name = ss.name AND fb.rn = 1
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
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
            library_id: row.get("library_id"),
            series_status: None,
            missing_count: None,
            metadata_provider: None,
            anilist_id: None,
            anilist_url: None,
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
                COALESCE(NULLIF(b.series, ''), 'unclassified') AS name,
                MAX(brp.last_read_at) AS series_last_read_at
            FROM books b
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
            GROUP BY COALESCE(NULLIF(b.series, ''), 'unclassified')
            HAVING (
                COUNT(brp.book_id) FILTER (WHERE brp.status IN ('read', 'reading')) > 0
                AND COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') < COUNT(*)
            )
        ),
        next_books AS (
            SELECT
                b.id, b.library_id, b.kind, b.format, b.title, b.author, b.authors, b.series, b.volume,
                b.language, b.page_count, b.thumbnail_path, b.updated_at,
                COALESCE(brp.status, 'unread') AS reading_status,
                brp.current_page AS reading_current_page,
                brp.last_read_at AS reading_last_read_at,
                os.series_last_read_at,
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(b.series, ''), 'unclassified')
                    ORDER BY b.volume NULLS LAST, b.title
                ) AS rn
            FROM books b
            JOIN ongoing_series os ON COALESCE(NULLIF(b.series, ''), 'unclassified') = os.name
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

/// Get metadata for a specific series
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series/{name}/metadata",
    tag = "series",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("name" = String, Path, description = "Series name"),
    ),
    responses(
        (status = 200, body = SeriesMetadata),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_metadata(
    State(state): State<AppState>,
    Path((library_id, name)): Path<(Uuid, String)>,
) -> Result<Json<SeriesMetadata>, ApiError> {
    // author/language from first book of series
    let books_row = if name == "unclassified" {
        sqlx::query("SELECT author, language FROM books WHERE library_id = $1 AND (series IS NULL OR series = '') LIMIT 1")
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await?
    } else {
        sqlx::query("SELECT author, language FROM books WHERE library_id = $1 AND series = $2 LIMIT 1")
            .bind(library_id)
            .bind(&name)
            .fetch_optional(&state.pool)
            .await?
    };

    let meta_row = sqlx::query(
        "SELECT authors, description, publishers, start_year, total_volumes, status, locked_fields FROM series_metadata WHERE library_id = $1 AND name = $2"
    )
    .bind(library_id)
    .bind(&name)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(SeriesMetadata {
        authors: meta_row.as_ref().map(|r| r.get::<Vec<String>, _>("authors")).unwrap_or_default(),
        description: meta_row.as_ref().and_then(|r| r.get("description")),
        publishers: meta_row.as_ref().map(|r| r.get::<Vec<String>, _>("publishers")).unwrap_or_default(),
        start_year: meta_row.as_ref().and_then(|r| r.get("start_year")),
        total_volumes: meta_row.as_ref().and_then(|r| r.get("total_volumes")),
        status: meta_row.as_ref().and_then(|r| r.get("status")),
        book_author: books_row.as_ref().and_then(|r| r.get("author")),
        book_language: books_row.as_ref().and_then(|r| r.get("language")),
        locked_fields: meta_row.as_ref().map(|r| r.get::<serde_json::Value, _>("locked_fields")).unwrap_or(serde_json::json!({})),
    }))
}

/// `author` and `language` are wrapped in an extra Option so we can distinguish
/// "absent from JSON" (keep books unchanged) from "present as null" (clear the field).
#[derive(Deserialize, ToSchema)]
pub struct UpdateSeriesRequest {
    pub new_name: String,
    /// Series-level authors list (stored in series_metadata)
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

/// Update metadata for all books in a series
#[utoipa::path(
    patch,
    path = "/libraries/{library_id}/series/{name}",
    tag = "series",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("name" = String, Path, description = "Series name (use 'unclassified' for books without series)"),
    ),
    request_body = UpdateSeriesRequest,
    responses(
        (status = 200, body = UpdateSeriesResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_series(
    State(state): State<AppState>,
    Path((library_id, name)): Path<(Uuid, String)>,
    Json(body): Json<UpdateSeriesRequest>,
) -> Result<Json<UpdateSeriesResponse>, ApiError> {
    let new_name = body.new_name.trim().to_string();
    if new_name.is_empty() {
        return Err(ApiError::bad_request("series name cannot be empty"));
    }
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
    let new_series_value: Option<String> = if new_name == "unclassified" { None } else { Some(new_name.clone()) };

    // 1. Update books: always update series name; author/language only if opted-in
    // $1=library_id, $2=new_series_value, $3=apply_author, $4=author_value,
    // $5=apply_language, $6=language_value, [$7=old_name]
    let result = if name == "unclassified" {
        sqlx::query(
            "UPDATE books \
             SET series = $2, \
                 author = CASE WHEN $3 THEN $4 ELSE author END, \
                 language = CASE WHEN $5 THEN $6 ELSE language END, \
                 updated_at = NOW() \
             WHERE library_id = $1 AND (series IS NULL OR series = '')"
        )
        .bind(library_id)
        .bind(&new_series_value)
        .bind(apply_author)
        .bind(&author_value)
        .bind(apply_language)
        .bind(&language_value)
        .execute(&state.pool)
        .await?
    } else {
        sqlx::query(
            "UPDATE books \
             SET series = $2, \
                 author = CASE WHEN $3 THEN $4 ELSE author END, \
                 language = CASE WHEN $5 THEN $6 ELSE language END, \
                 updated_at = NOW() \
             WHERE library_id = $1 AND series = $7"
        )
        .bind(library_id)
        .bind(&new_series_value)
        .bind(apply_author)
        .bind(&author_value)
        .bind(apply_language)
        .bind(&language_value)
        .bind(&name)
        .execute(&state.pool)
        .await?
    };

    // 2. Upsert series_metadata (keyed by new_name)
    let meta_name = new_series_value.as_deref().unwrap_or("unclassified");
    let authors: Vec<String> = body.authors.iter()
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect();
    let locked_fields = body.locked_fields.clone().unwrap_or(serde_json::json!({}));

    // When renaming, preserve the filesystem-derived original name so the scanner
    // can map files back to the renamed series instead of recreating the old one.
    let is_rename = name != "unclassified" && new_name != name;
    let original_name: Option<String> = if is_rename {
        // Check if the old metadata already has an original_name (chained renames: A→B→C)
        let existing_original: Option<Option<String>> = sqlx::query_scalar(
            "SELECT original_name FROM series_metadata WHERE library_id = $1 AND name = $2"
        )
        .bind(library_id)
        .bind(&name)
        .fetch_optional(&state.pool)
        .await?;
        // Use existing original_name if set, otherwise use the old name itself
        Some(existing_original.flatten().unwrap_or_else(|| name.clone()))
    } else {
        None
    };

    sqlx::query(
        r#"
        INSERT INTO series_metadata (library_id, name, authors, description, publishers, start_year, total_volumes, status, locked_fields, original_name, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())
        ON CONFLICT (library_id, name) DO UPDATE
          SET authors     = EXCLUDED.authors,
              description = EXCLUDED.description,
              publishers  = EXCLUDED.publishers,
              start_year  = EXCLUDED.start_year,
              total_volumes = EXCLUDED.total_volumes,
              status      = EXCLUDED.status,
              locked_fields = EXCLUDED.locked_fields,
              original_name = COALESCE(EXCLUDED.original_name, series_metadata.original_name),
              updated_at  = NOW()
        "#
    )
    .bind(library_id)
    .bind(meta_name)
    .bind(&authors)
    .bind(&description)
    .bind(&publishers)
    .bind(body.start_year)
    .bind(body.total_volumes)
    .bind(&body.status)
    .bind(&locked_fields)
    .bind(&original_name)
    .execute(&state.pool)
    .await?;

    // 3. If renamed, delete the old series_metadata entry
    if is_rename {
        sqlx::query(
            "DELETE FROM series_metadata WHERE library_id = $1 AND name = $2"
        )
        .bind(library_id)
        .bind(&name)
        .execute(&state.pool)
        .await?;
    }

    Ok(Json(UpdateSeriesResponse { updated: result.rows_affected() }))
}
