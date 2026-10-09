use axum::extract::Extension;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use sqlx::Row;
use uuid::Uuid;

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

    let data_sql = format!(
        r#"
        WITH series_counts AS (
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
        filtered AS (
            SELECT
                sc.name,
                sc.series_id,
                sc.book_count,
                sc.books_read_count,
                s.status as series_status,
                mc.missing_count,
                ml.provider as metadata_provider,
                asl.anilist_id,
                asl.anilist_url,
                s.cover_url, s.start_year, s.genres, s.authors, s.description
            FROM series_counts sc
            LEFT JOIN series s ON s.id = sc.series_id
            LEFT JOIN missing_counts mc ON mc.series_id = sc.series_id
            LEFT JOIN LATERAL (
                SELECT eml.provider FROM external_metadata_links eml
                WHERE eml.series_id = sc.series_id AND eml.library_id = $1 AND eml.status = 'approved'
                ORDER BY eml.is_primary DESC, eml.created_at DESC LIMIT 1
            ) ml ON TRUE
            LEFT JOIN anilist_series_links asl ON asl.series_id = sc.series_id AND asl.provider = 'anilist'
            WHERE TRUE
              {q_cond}
              {count_rs_cond}
              {ss_cond}
              {missing_cond}
              {metadata_provider_cond}
              {has_books_cond}
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
        ("gap" = Option<String>, Query, description = "Filter by metadata gap: 'no_description', 'no_genre', 'no_authors', 'no_publishers', 'no_year', 'no_cover', or 'no_community_score'"),
        ("author" = Option<String>, Query, description = "Filter by author name (matches in series.authors or book-level authors)"),
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

    let mut p: usize = 0;

    let lib_cond = if query.library_id.is_some() {
        p += 1;
        format!("WHERE s.library_id = ${p}")
    } else {
        "WHERE TRUE".to_string()
    };

    let q_cond = if query.q.is_some() {
        p += 1;
        format!("AND norm_text(s.name) LIKE norm_text(${p})")
    } else {
        String::new()
    };

    let rs_cond = if reading_statuses.is_some() {
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
        Some("unlinked") => "AND NOT EXISTS (SELECT 1 FROM external_metadata_links eml WHERE eml.series_id = sc.series_id AND eml.library_id = sc.library_id AND eml.status = 'approved')".to_string(),
        Some("linked") => "AND EXISTS (SELECT 1 FROM external_metadata_links eml WHERE eml.series_id = sc.series_id AND eml.library_id = sc.library_id AND eml.status = 'approved')".to_string(),
        Some(_) => {
            p += 1;
            format!("AND EXISTS (SELECT 1 FROM external_metadata_links eml WHERE eml.series_id = sc.series_id AND eml.library_id = sc.library_id AND eml.status = 'approved' AND eml.provider = ${p})")
        }
        None => String::new(),
    };

    let gap_cond = match query.gap.as_deref() {
        Some("no_description") => "AND (s.description IS NULL OR s.description = '')".to_string(),
        Some("no_genre") => "AND COALESCE(cardinality(s.genres), 0) = 0".to_string(),
        Some("no_authors") => "AND COALESCE(cardinality(s.authors), 0) = 0".to_string(),
        Some("no_publishers") => "AND COALESCE(cardinality(s.publishers), 0) = 0".to_string(),
        Some("no_year") => "AND s.start_year IS NULL".to_string(),
        Some("no_cover") => "AND (s.cover_url IS NULL OR s.cover_url = '')".to_string(),
        // No approved provider link carries a usable rating for this series.
        // Must mirror the `community_score_lateral` predicate exactly.
        Some("no_community_score") => "AND cs.community_score IS NULL".to_string(),
        _ => String::new(),
    };

    let author_cond = if query.author.is_some() {
        p += 1;
        format!("AND (${p} = ANY(s.authors) OR EXISTS (SELECT 1 FROM books bk WHERE bk.series_id = s.id AND ${p} = ANY(COALESCE(NULLIF(bk.authors, '{{}}'), CASE WHEN bk.author IS NOT NULL AND bk.author != '' THEN ARRAY[bk.author] ELSE ARRAY[]::text[] END))))")
    } else {
        String::new()
    };

    let has_books = query.has_books.as_deref() == Some("true");
    let has_books_cond = if has_books {
        "AND sc.book_count > 0".to_string()
    } else {
        String::new()
    };

    let no_books = query.no_books.as_deref() == Some("true");
    let no_books_cond = if no_books {
        "AND sc.book_count = 0".to_string()
    } else {
        String::new()
    };

    let genre_cond = if query.genre.is_some() {
        p += 1;
        format!("AND ${p} = ANY(s.genres)")
    } else {
        String::new()
    };

    let rated_only_cond = match query.rated_only.as_deref() {
        Some("true") => "AND sur.rating IS NOT NULL".to_string(),
        Some("unrated") => "AND sur.rating IS NULL".to_string(),
        _ => String::new(),
    };

    let oneshot_cond = if let Some(vt) = query.volume_type.as_deref() {
        let safe_vt = match vt {
            "regular" | "oneshot" | "hs" | "integral" => vt,
            _ => "",
        };
        if safe_vt.is_empty() {
            String::new()
        } else {
            format!("AND EXISTS (SELECT 1 FROM books bvt WHERE bvt.series_id = s.id AND bvt.volume_type = '{safe_vt}')")
        }
    } else {
        String::new()
    };

    // Missing counts CTE
    let missing_cte = if query.library_id.is_some() {
        helpers::build_missing_counts_cte(Some("$1"))
    } else {
        helpers::build_missing_counts_cte(None)
    };

    // Used in SELECT and ORDER BY — index on (series_id, status) makes this fast per row
    let community_score_lateral = r#"LEFT JOIN LATERAL (
        SELECT (AVG(provider_rating / COALESCE(NULLIF(provider_rating_scale, 0), 10.0) * 5.0))::real AS community_score
        FROM external_metadata_links
        WHERE series_id = sc.series_id
          AND status = 'approved'
          AND provider_rating IS NOT NULL
          AND provider_rating > 0
    ) cs ON TRUE"#;

    let user_id_p = p + 1;
    let limit_p = p + 2;
    let offset_p = p + 3;

    let genre_restriction_cond = format!(
        "AND (${user_id_p}::uuid IS NULL OR NOT EXISTS (SELECT 1 FROM user_genre_restrictions ugr WHERE ugr.user_id = ${user_id_p} AND ugr.genre = ANY(s.genres)))"
    );

    let title_order_clause = "lower(name) ASC";
    let series_order_clause = match query.sort.as_deref() {
        Some("latest") => {
            // For series without books, latest_created_at falls back to s.created_at
            // (see series_counts CTE), so the value is never NULL.
            "latest_created_at DESC".to_string()
        }
        Some("release_date") => format!("start_year DESC NULLS LAST, {title_order_clause}"),
        Some("community_score") => {
            format!("community_score DESC NULLS LAST, {title_order_clause}")
        }
        _ => title_order_clause.to_string(),
    };

    let data_sql = format!(
        r#"
        WITH series_counts AS (
            SELECT
                s.id as series_id,
                s.name,
                s.library_id,
                COUNT(b.id) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count,
                COALESCE(MAX(b.created_at), s.created_at) as latest_created_at
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ${user_id_p}::uuid IS NOT NULL AND brp.user_id = ${user_id_p}
            {lib_cond}
            GROUP BY s.id, s.name, s.library_id, s.created_at
        ),
        {missing_cte},
        filtered AS (
            SELECT
                sc.name,
                sc.series_id,
                sc.book_count,
                sc.books_read_count,
                sc.library_id,
                s.status as series_status,
                mc.missing_count,
                ml.provider as metadata_provider,
                asl.anilist_id,
                asl.anilist_url,
                s.cover_url, s.start_year, s.genres, s.authors, s.description,
                cs.community_score,
                sur.rating as user_rating,
                sc.latest_created_at
            FROM series_counts sc
            LEFT JOIN series s ON s.id = sc.series_id
            LEFT JOIN missing_counts mc ON mc.series_id = sc.series_id
            LEFT JOIN LATERAL (
                SELECT eml.provider FROM external_metadata_links eml
                WHERE eml.series_id = sc.series_id AND eml.library_id = sc.library_id AND eml.status = 'approved'
                ORDER BY eml.is_primary DESC, eml.created_at DESC LIMIT 1
            ) ml ON TRUE
            LEFT JOIN anilist_series_links asl ON asl.series_id = sc.series_id AND asl.provider = 'anilist'
            {community_score_lateral}
            LEFT JOIN series_user_ratings sur ON sur.series_id = sc.series_id AND sur.user_id = ${user_id_p}::uuid
            WHERE TRUE
              {q_cond}
              {rs_cond}
              {ss_cond}
              {missing_cond}
              {metadata_provider_cond}
              {gap_cond}
              {author_cond}
              {has_books_cond}
              {no_books_cond}
              {genre_cond}
              {oneshot_cond}
              {rated_only_cond}
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
            WHERE b.series_id = p.series_id
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

    let mut data_builder = sqlx::query(sqlx::AssertSqlSafe(data_sql.as_str()));

    if let Some(lib_id) = query.library_id {
        data_builder = data_builder.bind(lib_id);
    }
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
    if let Some(ref author) = query.author {
        data_builder = data_builder.bind(author.clone());
    }
    if let Some(ref genre) = query.genre {
        data_builder = data_builder.bind(genre.clone());
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
                first_book_id: row.get("first_book_id"),
                first_book_updated_at: row.get("first_book_updated_at"),
                library_id: row.get("library_id"),
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
                user_rating: row.get("user_rating"),
                community_score: row.get("community_score"),
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
