use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::{IntoParams, ToSchema};

use crate::{error::ApiError, state::AppState};

#[derive(Deserialize, IntoParams)]
pub struct StatsQuery {
    /// Granularity: "day", "week" or "month" (default: "month")
    pub period: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct StatsOverview {
    pub total_books: i64,
    pub total_series: i64,
    pub total_libraries: i64,
    pub total_pages: i64,
    pub total_size_bytes: i64,
    pub total_authors: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ReadingStatusStats {
    pub unread: i64,
    pub reading: i64,
    pub read: i64,
}

#[derive(Serialize, ToSchema)]
pub struct FormatCount {
    pub format: String,
    pub count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct LanguageCount {
    pub language: Option<String>,
    pub count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct LibraryStats {
    pub library_name: String,
    pub book_count: i64,
    pub size_bytes: i64,
    pub read_count: i64,
    pub reading_count: i64,
    pub unread_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct TopSeries {
    pub series: String,
    pub book_count: i64,
    pub read_count: i64,
    pub total_pages: i64,
}

#[derive(Serialize, ToSchema)]
pub struct MonthlyAdditions {
    pub month: String,
    pub books_added: i64,
}

#[derive(Serialize, ToSchema)]
pub struct MetadataStats {
    pub total_series: i64,
    pub series_linked: i64,
    pub series_unlinked: i64,
    pub books_with_summary: i64,
    pub books_with_isbn: i64,
    pub by_provider: Vec<ProviderCount>,
}

#[derive(Serialize, ToSchema)]
pub struct ProviderCount {
    pub provider: String,
    pub count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct CurrentlyReadingItem {
    pub book_id: String,
    pub title: String,
    pub series: Option<String>,
    pub current_page: i32,
    pub page_count: i32,
}

#[derive(Serialize, ToSchema)]
pub struct RecentlyReadItem {
    pub book_id: String,
    pub title: String,
    pub series: Option<String>,
    pub last_read_at: String,
}

#[derive(Serialize, ToSchema)]
pub struct MonthlyReading {
    pub month: String,
    pub books_read: i64,
}

#[derive(Serialize, ToSchema)]
pub struct StatsResponse {
    pub overview: StatsOverview,
    pub reading_status: ReadingStatusStats,
    pub currently_reading: Vec<CurrentlyReadingItem>,
    pub recently_read: Vec<RecentlyReadItem>,
    pub reading_over_time: Vec<MonthlyReading>,
    pub by_format: Vec<FormatCount>,
    pub by_language: Vec<LanguageCount>,
    pub by_library: Vec<LibraryStats>,
    pub top_series: Vec<TopSeries>,
    pub additions_over_time: Vec<MonthlyAdditions>,
    pub metadata: MetadataStats,
}

/// Get collection statistics for the dashboard
#[utoipa::path(
    get,
    path = "/stats",
    tag = "stats",
    params(StatsQuery),
    responses(
        (status = 200, body = StatsResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_stats(
    State(state): State<AppState>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<StatsResponse>, ApiError> {
    let period = query.period.as_deref().unwrap_or("month");
    // Overview + reading status in one query
    let overview_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total_books,
            COUNT(DISTINCT NULLIF(series, '')) AS total_series,
            COUNT(DISTINCT library_id) AS total_libraries,
            COALESCE(SUM(page_count), 0)::BIGINT AS total_pages,
            (SELECT COUNT(DISTINCT a) FROM (
                SELECT DISTINCT UNNEST(authors) AS a FROM books WHERE authors != '{}'
                UNION
                SELECT DISTINCT author FROM books WHERE author IS NOT NULL AND author != ''
            ) sub) AS total_authors,
            COUNT(*) FILTER (WHERE COALESCE(brp.status, 'unread') = 'unread') AS unread,
            COUNT(*) FILTER (WHERE brp.status = 'reading') AS reading,
            COUNT(*) FILTER (WHERE brp.status = 'read') AS read
        FROM books b
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id
        "#,
    )
    .fetch_one(&state.pool)
    .await?;

    // Total size from book_files
    let size_row = sqlx::query(
        r#"
        SELECT COALESCE(SUM(bf.size_bytes), 0)::BIGINT AS total_size_bytes
        FROM (
            SELECT DISTINCT ON (book_id) size_bytes
            FROM book_files
            ORDER BY book_id, updated_at DESC
        ) bf
        "#,
    )
    .fetch_one(&state.pool)
    .await?;

    let overview = StatsOverview {
        total_books: overview_row.get("total_books"),
        total_series: overview_row.get("total_series"),
        total_libraries: overview_row.get("total_libraries"),
        total_pages: overview_row.get("total_pages"),
        total_size_bytes: size_row.get("total_size_bytes"),
        total_authors: overview_row.get("total_authors"),
    };

    let reading_status = ReadingStatusStats {
        unread: overview_row.get("unread"),
        reading: overview_row.get("reading"),
        read: overview_row.get("read"),
    };

    // By format
    let format_rows = sqlx::query(
        r#"
        SELECT COALESCE(bf.format, b.kind) AS fmt, COUNT(*) AS count
        FROM books b
        LEFT JOIN LATERAL (
            SELECT format FROM book_files WHERE book_id = b.id ORDER BY updated_at DESC LIMIT 1
        ) bf ON TRUE
        GROUP BY fmt
        ORDER BY count DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let by_format: Vec<FormatCount> = format_rows
        .iter()
        .map(|r| FormatCount {
            format: r.get::<Option<String>, _>("fmt").unwrap_or_else(|| "unknown".to_string()),
            count: r.get("count"),
        })
        .collect();

    // By language
    let lang_rows = sqlx::query(
        r#"
        SELECT language, COUNT(*) AS count
        FROM books
        GROUP BY language
        ORDER BY count DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let by_language: Vec<LanguageCount> = lang_rows
        .iter()
        .map(|r| LanguageCount {
            language: r.get("language"),
            count: r.get("count"),
        })
        .collect();

    // By library
    let lib_rows = sqlx::query(
        r#"
        SELECT
            l.name AS library_name,
            COUNT(b.id) AS book_count,
            COALESCE(SUM(bf.size_bytes), 0)::BIGINT AS size_bytes,
            COUNT(*) FILTER (WHERE brp.status = 'read') AS read_count,
            COUNT(*) FILTER (WHERE brp.status = 'reading') AS reading_count,
            COUNT(*) FILTER (WHERE COALESCE(brp.status, 'unread') = 'unread') AS unread_count
        FROM libraries l
        LEFT JOIN books b ON b.library_id = l.id
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id
        LEFT JOIN LATERAL (
            SELECT size_bytes FROM book_files WHERE book_id = b.id ORDER BY updated_at DESC LIMIT 1
        ) bf ON TRUE
        GROUP BY l.id, l.name
        ORDER BY book_count DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let by_library: Vec<LibraryStats> = lib_rows
        .iter()
        .map(|r| LibraryStats {
            library_name: r.get("library_name"),
            book_count: r.get("book_count"),
            size_bytes: r.get("size_bytes"),
            read_count: r.get("read_count"),
            reading_count: r.get("reading_count"),
            unread_count: r.get("unread_count"),
        })
        .collect();

    // Top series (by book count)
    let series_rows = sqlx::query(
        r#"
        SELECT
            b.series,
            COUNT(*) AS book_count,
            COUNT(*) FILTER (WHERE brp.status = 'read') AS read_count,
            COALESCE(SUM(b.page_count), 0)::BIGINT AS total_pages
        FROM books b
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id
        WHERE b.series IS NOT NULL AND b.series != ''
        GROUP BY b.series
        ORDER BY book_count DESC
        LIMIT 10
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let top_series: Vec<TopSeries> = series_rows
        .iter()
        .map(|r| TopSeries {
            series: r.get("series"),
            book_count: r.get("book_count"),
            read_count: r.get("read_count"),
            total_pages: r.get("total_pages"),
        })
        .collect();

    // Additions over time (with gap filling)
    let additions_rows = match period {
        "day" => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM-DD') AS month,
                    COALESCE(cnt.books_added, 0) AS books_added
                FROM generate_series(CURRENT_DATE - INTERVAL '6 days', CURRENT_DATE, '1 day') AS d(dt)
                LEFT JOIN (
                    SELECT created_at::date AS dt, COUNT(*) AS books_added
                    FROM books
                    WHERE created_at >= CURRENT_DATE - INTERVAL '6 days'
                    GROUP BY created_at::date
                ) cnt ON cnt.dt = d.dt
                ORDER BY month ASC
                "#,
            )
            .fetch_all(&state.pool)
            .await?
        }
        "week" => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM-DD') AS month,
                    COALESCE(cnt.books_added, 0) AS books_added
                FROM generate_series(
                    DATE_TRUNC('week', NOW() - INTERVAL '2 months'),
                    DATE_TRUNC('week', NOW()),
                    '1 week'
                ) AS d(dt)
                LEFT JOIN (
                    SELECT DATE_TRUNC('week', created_at) AS dt, COUNT(*) AS books_added
                    FROM books
                    WHERE created_at >= DATE_TRUNC('week', NOW() - INTERVAL '2 months')
                    GROUP BY DATE_TRUNC('week', created_at)
                ) cnt ON cnt.dt = d.dt
                ORDER BY month ASC
                "#,
            )
            .fetch_all(&state.pool)
            .await?
        }
        _ => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM') AS month,
                    COALESCE(cnt.books_added, 0) AS books_added
                FROM generate_series(
                    DATE_TRUNC('month', NOW()) - INTERVAL '11 months',
                    DATE_TRUNC('month', NOW()),
                    '1 month'
                ) AS d(dt)
                LEFT JOIN (
                    SELECT DATE_TRUNC('month', created_at) AS dt, COUNT(*) AS books_added
                    FROM books
                    WHERE created_at >= DATE_TRUNC('month', NOW()) - INTERVAL '11 months'
                    GROUP BY DATE_TRUNC('month', created_at)
                ) cnt ON cnt.dt = d.dt
                ORDER BY month ASC
                "#,
            )
            .fetch_all(&state.pool)
            .await?
        }
    };

    let additions_over_time: Vec<MonthlyAdditions> = additions_rows
        .iter()
        .map(|r| MonthlyAdditions {
            month: r.get("month"),
            books_added: r.get("books_added"),
        })
        .collect();

    // Metadata stats
    let meta_row = sqlx::query(
        r#"
        SELECT
            (SELECT COUNT(DISTINCT NULLIF(series, '')) FROM books) AS total_series,
            (SELECT COUNT(DISTINCT series_name) FROM external_metadata_links WHERE status = 'approved') AS series_linked,
            (SELECT COUNT(*) FROM books WHERE summary IS NOT NULL AND summary != '') AS books_with_summary,
            (SELECT COUNT(*) FROM books WHERE isbn IS NOT NULL AND isbn != '') AS books_with_isbn
        "#,
    )
    .fetch_one(&state.pool)
    .await?;

    let meta_total_series: i64 = meta_row.get("total_series");
    let meta_series_linked: i64 = meta_row.get("series_linked");

    let provider_rows = sqlx::query(
        r#"
        SELECT provider, COUNT(DISTINCT series_name) AS count
        FROM external_metadata_links
        WHERE status = 'approved'
        GROUP BY provider
        ORDER BY count DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let by_provider: Vec<ProviderCount> = provider_rows
        .iter()
        .map(|r| ProviderCount {
            provider: r.get("provider"),
            count: r.get("count"),
        })
        .collect();

    let metadata = MetadataStats {
        total_series: meta_total_series,
        series_linked: meta_series_linked,
        series_unlinked: meta_total_series - meta_series_linked,
        books_with_summary: meta_row.get("books_with_summary"),
        books_with_isbn: meta_row.get("books_with_isbn"),
        by_provider,
    };

    // Currently reading books
    let reading_rows = sqlx::query(
        r#"
        SELECT b.id AS book_id, b.title, b.series, brp.current_page, b.page_count
        FROM book_reading_progress brp
        JOIN books b ON b.id = brp.book_id
        WHERE brp.status = 'reading' AND brp.current_page IS NOT NULL
        ORDER BY brp.updated_at DESC
        LIMIT 20
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let currently_reading: Vec<CurrentlyReadingItem> = reading_rows
        .iter()
        .map(|r| {
            let id: uuid::Uuid = r.get("book_id");
            CurrentlyReadingItem {
                book_id: id.to_string(),
                title: r.get("title"),
                series: r.get("series"),
                current_page: r.get::<Option<i32>, _>("current_page").unwrap_or(0),
                page_count: r.get::<Option<i32>, _>("page_count").unwrap_or(0),
            }
        })
        .collect();

    // Recently read books
    let recent_rows = sqlx::query(
        r#"
        SELECT b.id AS book_id, b.title, b.series,
               TO_CHAR(brp.last_read_at, 'YYYY-MM-DD') AS last_read_at
        FROM book_reading_progress brp
        JOIN books b ON b.id = brp.book_id
        WHERE brp.status = 'read' AND brp.last_read_at IS NOT NULL
        ORDER BY brp.last_read_at DESC
        LIMIT 10
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let recently_read: Vec<RecentlyReadItem> = recent_rows
        .iter()
        .map(|r| {
            let id: uuid::Uuid = r.get("book_id");
            RecentlyReadItem {
                book_id: id.to_string(),
                title: r.get("title"),
                series: r.get("series"),
                last_read_at: r.get::<Option<String>, _>("last_read_at").unwrap_or_default(),
            }
        })
        .collect();

    // Reading activity over time (with gap filling)
    let reading_time_rows = match period {
        "day" => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM-DD') AS month,
                    COALESCE(cnt.books_read, 0) AS books_read
                FROM generate_series(CURRENT_DATE - INTERVAL '6 days', CURRENT_DATE, '1 day') AS d(dt)
                LEFT JOIN (
                    SELECT brp.last_read_at::date AS dt, COUNT(*) AS books_read
                    FROM book_reading_progress brp
                    WHERE brp.status = 'read'
                      AND brp.last_read_at >= CURRENT_DATE - INTERVAL '6 days'
                    GROUP BY brp.last_read_at::date
                ) cnt ON cnt.dt = d.dt
                ORDER BY month ASC
                "#,
            )
            .fetch_all(&state.pool)
            .await?
        }
        "week" => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM-DD') AS month,
                    COALESCE(cnt.books_read, 0) AS books_read
                FROM generate_series(
                    DATE_TRUNC('week', NOW() - INTERVAL '2 months'),
                    DATE_TRUNC('week', NOW()),
                    '1 week'
                ) AS d(dt)
                LEFT JOIN (
                    SELECT DATE_TRUNC('week', brp.last_read_at) AS dt, COUNT(*) AS books_read
                    FROM book_reading_progress brp
                    WHERE brp.status = 'read'
                      AND brp.last_read_at >= DATE_TRUNC('week', NOW() - INTERVAL '2 months')
                    GROUP BY DATE_TRUNC('week', brp.last_read_at)
                ) cnt ON cnt.dt = d.dt
                ORDER BY month ASC
                "#,
            )
            .fetch_all(&state.pool)
            .await?
        }
        _ => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM') AS month,
                    COALESCE(cnt.books_read, 0) AS books_read
                FROM generate_series(
                    DATE_TRUNC('month', NOW()) - INTERVAL '11 months',
                    DATE_TRUNC('month', NOW()),
                    '1 month'
                ) AS d(dt)
                LEFT JOIN (
                    SELECT DATE_TRUNC('month', brp.last_read_at) AS dt, COUNT(*) AS books_read
                    FROM book_reading_progress brp
                    WHERE brp.status = 'read'
                      AND brp.last_read_at >= DATE_TRUNC('month', NOW()) - INTERVAL '11 months'
                    GROUP BY DATE_TRUNC('month', brp.last_read_at)
                ) cnt ON cnt.dt = d.dt
                ORDER BY month ASC
                "#,
            )
            .fetch_all(&state.pool)
            .await?
        }
    };

    let reading_over_time: Vec<MonthlyReading> = reading_time_rows
        .iter()
        .map(|r| MonthlyReading {
            month: r.get::<Option<String>, _>("month").unwrap_or_default(),
            books_read: r.get("books_read"),
        })
        .collect();

    Ok(Json(StatsResponse {
        overview,
        reading_status,
        currently_reading,
        recently_read,
        reading_over_time,
        by_format,
        by_language,
        by_library,
        top_series,
        additions_over_time,
        metadata,
    }))
}
