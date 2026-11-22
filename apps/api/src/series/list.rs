use axum::extract::Extension;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use sqlx::Row;
use uuid::Uuid;

use super::catalog::{self, SeriesRequest};
use super::{helpers, ListAllSeriesQuery, ListSeriesQuery, SeriesItem, SeriesPage};
use crate::{auth::AuthUser, error::ApiError, state::AppState};

// ─── List series (per library) ───────────────────────────────────────────────

/// List all series in a library with pagination
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series",
    tag = "series",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("q" = Option<String>, Query, description = "Filter by series name (case- and accent-insensitive, partial match)"),
        ("reading_status" = Option<String>, Query, description = "Filter by reading status, comma-separated (e.g. 'unread,reading')"),
        ("metadata_provider" = Option<String>, Query, description = "Filter by metadata provider: a provider name (e.g. 'google_books'), 'linked' (any provider), or 'unlinked' (no provider)"),
        ("shelved" = Option<String>, Query, description = "Filter by shelf state for the current user: 'true' for series set aside, 'false' for the others"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
        ("sort" = Option<String>, Query, description = "Sort order: 'title' (default), 'latest' (most recently added first), or 'release_date' (series start year descending; undated series last by title)"),
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
        s.split(',')
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect()
    });

    let series_status_expr = r#"CASE
              WHEN sc.book_count = 0 THEN 'unread'
              WHEN sc.books_read_count = sc.book_count THEN 'read'
              WHEN sc.books_read_count = 0 THEN 'unread'
              ELSE 'reading'
           END"#;

    let has_missing = query.has_missing.as_deref() == Some("true");

    let mut p: usize = 1;

    let q_cond = if query.q.is_some() {
        p += 1;
        format!("AND norm_text(s.name) LIKE norm_text(${p})")
    } else {
        String::new()
    };

    let count_rs_cond = if reading_statuses.is_some() {
        p += 1;
        format!("AND {series_status_expr} = ANY(${p})")
    } else {
        String::new()
    };

    let ss_cond = if query.series_status.is_some() {
        p += 1;
        format!("AND LOWER(s.status) = ${p}")
    } else {
        String::new()
    };

    let missing_cond = if has_missing {
        "AND mc.missing_count > 0".to_string()
    } else {
        String::new()
    };

    let metadata_provider_cond = match query.metadata_provider.as_deref() {
        Some("unlinked") => "AND NOT EXISTS (SELECT 1 FROM external_metadata_links eml WHERE eml.series_id = sc.series_id AND eml.library_id = $1 AND eml.status = 'approved')".to_string(),
        Some("linked") => "AND EXISTS (SELECT 1 FROM external_metadata_links eml WHERE eml.series_id = sc.series_id AND eml.library_id = $1 AND eml.status = 'approved')".to_string(),
        Some(_) => {
            p += 1;
            format!("AND EXISTS (SELECT 1 FROM external_metadata_links eml WHERE eml.series_id = sc.series_id AND eml.library_id = $1 AND eml.status = 'approved' AND eml.provider = ${p})")
        }
        None => String::new(),
    };

    let has_books = query.has_books.as_deref() == Some("true");
    let has_books_cond = if has_books {
        "AND sc.book_count > 0".to_string()
    } else {
        String::new()
    };

    let shelved_cond = match query.shelved.as_deref() {
        Some("true") => "AND sus.created_at IS NOT NULL".to_string(),
        Some("false") => "AND sus.created_at IS NULL".to_string(),
        _ => String::new(),
    };

    let user_id_p = p + 1;
    let limit_p = p + 2;
    let offset_p = p + 3;

    let genre_restriction_cond = format!(
        "AND (${user_id_p}::uuid IS NULL OR NOT EXISTS (SELECT 1 FROM user_genre_restrictions ugr WHERE ugr.user_id = ${user_id_p} AND ugr.genre = ANY(s.genres)))"
    );

    let missing_cte = helpers::build_missing_counts_cte(Some("$1"));

    let title_order_clause = "lower(name) ASC";
    let series_order_clause = match query.sort.as_deref() {
        Some("release_date") => format!("start_year DESC NULLS LAST, {title_order_clause}"),
        _ => title_order_clause.to_string(),
    };

    let tome_counts = helpers::series_tome_counts("brp");

    let data_sql = format!(
        r#"
        WITH series_counts AS (
            SELECT
                s.id as series_id,
                s.name,
                {tome_counts}
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ${user_id_p}::uuid IS NOT NULL AND brp.user_id = ${user_id_p}
            WHERE s.library_id = $1
            GROUP BY s.id, s.name
        ),
        {missing_cte},
        filtered AS (
            SELECT
                sc.name,
                sc.series_id,
                sc.book_count,
                sc.books_read_count,
                sc.last_read_at,
                s.status as series_status,
                mc.missing_count,
                ml.provider as metadata_provider,
                asl.anilist_id,
                asl.anilist_url,
                s.cover_url, s.start_year, s.genres, s.authors, s.description,
                sus.created_at AS shelved_at
            FROM series_counts sc
            LEFT JOIN series s ON s.id = sc.series_id
            LEFT JOIN missing_counts mc ON mc.series_id = sc.series_id
            LEFT JOIN LATERAL (
                SELECT eml.provider FROM external_metadata_links eml
                WHERE eml.series_id = sc.series_id AND eml.library_id = $1 AND eml.status = 'approved'
                ORDER BY eml.is_primary DESC, eml.created_at DESC LIMIT 1
            ) ml ON TRUE
            LEFT JOIN anilist_series_links asl ON asl.series_id = sc.series_id AND asl.provider = 'anilist'
            LEFT JOIN series_user_shelves sus ON sus.series_id = sc.series_id AND sus.user_id = ${user_id_p}::uuid
            WHERE TRUE
              {q_cond}
              {count_rs_cond}
              {ss_cond}
              {missing_cond}
              {metadata_provider_cond}
              {has_books_cond}
              {shelved_cond}
              {genre_restriction_cond}
        ),
        total AS (SELECT COUNT(*) AS total_count FROM filtered),
        page AS (
            SELECT * FROM filtered
            ORDER BY {series_order_clause}
            LIMIT ${limit_p} OFFSET ${offset_p}
        )
        SELECT p.*, fb.id as first_book_id, fb.updated_at as first_book_updated_at, t.total_count
        FROM total t
        LEFT JOIN page p ON TRUE
        LEFT JOIN LATERAL (
            SELECT b.id, b.updated_at
            FROM books b
            WHERE b.library_id = $1 AND b.series_id = p.series_id
            ORDER BY
                CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END,
                b.volume NULLS LAST,
                b.title ASC
            LIMIT 1
        ) fb ON TRUE
        ORDER BY {series_order_clause}
        "#
    );

    let q_pattern = query.q.as_deref().map(|q| format!("%{}%", q));

    let mut data_builder = sqlx::query(sqlx::AssertSqlSafe(data_sql.as_str())).bind(library_id);

    if let Some(ref pat) = q_pattern {
        data_builder = data_builder.bind(pat);
    }
    if let Some(ref statuses) = reading_statuses {
        data_builder = data_builder.bind(statuses.clone());
    }
    if let Some(ref ss) = query.series_status {
        data_builder = data_builder.bind(ss);
    }
    if let Some(ref mp) = query.metadata_provider {
        if mp != "linked" && mp != "unlinked" {
            data_builder = data_builder.bind(mp);
        }
    }

    data_builder = data_builder.bind(user_id).bind(limit).bind(offset);

    let rows = data_builder.fetch_all(&state.pool).await?;
    let total: i64 = rows.first().map(|r| r.get("total_count")).unwrap_or(0);

    let items: Vec<SeriesItem> = rows
        .iter()
        .filter_map(|row| {
            let series_id: Option<Uuid> = row.get("series_id");
            let series_id = series_id?;
            Some(SeriesItem {
                name: row.get("name"),
                series_id,
                book_count: row.get("book_count"),
                books_read_count: row.get("books_read_count"),
                last_read_at: row.get("last_read_at"),
                first_book_id: row.get("first_book_id"),
                first_book_updated_at: row.get("first_book_updated_at"),
                library_id,
                series_status: row.get("series_status"),
                missing_count: row.get("missing_count"),
                metadata_provider: row.get("metadata_provider"),
                anilist_id: row.get("anilist_id"),
                anilist_url: row.get("anilist_url"),
                cover_url: row.get("cover_url"),
                start_year: row.get("start_year"),
                genres: row.get::<Vec<String>, _>("genres"),
                authors: row.get::<Vec<String>, _>("authors"),
                description: row.get("description"),
                user_rating: None,
                community_score: None,
                shelved_at: row.get("shelved_at"),
            })
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
        ("q" = Option<String>, Query, description = "Filter by series name (case- and accent-insensitive, partial match)"),
        ("library_id" = Option<String>, Query, description = "Filter by library ID"),
        ("reading_status" = Option<String>, Query, description = "Filter by reading status, comma-separated (e.g. 'unread,reading')"),
        ("metadata_provider" = Option<String>, Query, description = "Filter by metadata provider: a provider name (e.g. 'google_books'), 'linked' (any provider), or 'unlinked' (no provider)"),
        ("shelved" = Option<String>, Query, description = "Filter by shelf state for the current user: 'true' for series set aside, 'false' for the others"),
        ("gap" = Option<String>, Query, description = "Filter by metadata gap: 'no_description', 'no_genre', 'no_authors', 'no_publishers', 'no_year', 'no_cover', or 'no_community_score'"),
        ("author" = Option<String>, Query, description = "Filter by author name (matches in series.authors or book-level authors)"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
        ("sort" = Option<String>, Query, description = "Sort order: 'title' (default), 'latest' (most recently added first), 'release_date' (series start year descending; undated series last by title), or 'community_score' (best rated first). Unknown values are rejected with 400"),
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
    let request = SeriesRequest::from_list_all(query, user.map(|u| u.0.user_id))?;
    let page = catalog::query(&state.pool, request).await?;

    Ok(Json(SeriesPage {
        items: page.items,
        total: page.total,
        page: page.page,
        limit: page.limit,
    }))
}
