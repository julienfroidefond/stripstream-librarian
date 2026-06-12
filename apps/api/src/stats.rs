use axum::{
    extract::{Extension, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::{IntoParams, ToSchema};

use crate::{auth::AuthUser, error::ApiError, state::AppState};

#[derive(Deserialize, IntoParams)]
pub struct StatsQuery {
    /// Granularity: "day", "week" or "month" (default: "week")
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
    pub username: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct RecentlyReadItem {
    pub book_id: String,
    pub title: String,
    pub series: Option<String>,
    pub last_read_at: String,
    pub username: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct MonthlyReading {
    pub month: String,
    pub books_read: i64,
    pub pages_read: i64,
}

#[derive(Serialize, ToSchema)]
pub struct UserMonthlyReading {
    pub month: String,
    pub username: String,
    pub books_read: i64,
    pub pages_read: i64,
}

#[derive(Serialize, ToSchema)]
pub struct JobTimePoint {
    pub label: String,
    pub scan: i64,
    pub rebuild: i64,
    pub thumbnail: i64,
    pub metadata: i64,
    pub downloads: i64,
    pub reading: i64,
    pub conversion: i64,
}

#[derive(Serialize, ToSchema)]
pub struct DownloadStats {
    pub active_downloads: i64,
    pub imported_downloads: i64,
    pub error_downloads: i64,
    pub total_downloads: i64,
    pub available_series: i64,
    pub total_missing_volumes: i64,
    pub recent_downloads: Vec<RecentDownloadItem>,
}

#[derive(Serialize, ToSchema)]
pub struct RecentDownloadItem {
    pub id: String,
    pub series_name: String,
    pub status: String,
    pub expected_volumes: Vec<i32>,
    pub created_at: String,
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
    pub jobs_over_time: Vec<JobTimePoint>,
    pub metadata: MetadataStats,
    pub users_reading_over_time: Vec<UserMonthlyReading>,
    pub downloads: DownloadStats,
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
    user: Option<Extension<AuthUser>>,
) -> Result<Json<StatsResponse>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let period = query.period.as_deref().unwrap_or("week");
    // Overview + reading status in one query
    let overview_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total_books,
            COUNT(DISTINCT b.series_id) AS total_series,
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
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ($1::uuid IS NULL OR brp.user_id = $1)
        "#,
    )
    .bind(user_id)
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
            format: r
                .get::<Option<String>, _>("fmt")
                .unwrap_or_else(|| "unknown".to_string()),
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
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ($1::uuid IS NULL OR brp.user_id = $1)
        LEFT JOIN LATERAL (
            SELECT size_bytes FROM book_files WHERE book_id = b.id ORDER BY updated_at DESC LIMIT 1
        ) bf ON TRUE
        GROUP BY l.id, l.name
        ORDER BY book_count DESC
        "#,
    )
    .bind(user_id)
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
            s.name AS series,
            COUNT(*) AS book_count,
            COUNT(*) FILTER (WHERE brp.status = 'read') AS read_count,
            COALESCE(SUM(b.page_count), 0)::BIGINT AS total_pages
        FROM books b
        JOIN series s ON s.id = b.series_id
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ($1::uuid IS NULL OR brp.user_id = $1)
        WHERE b.series_id IS NOT NULL
        GROUP BY s.name
        ORDER BY book_count DESC
        LIMIT 10
        "#,
    )
    .bind(user_id)
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
            (SELECT COUNT(DISTINCT series_id) FROM books WHERE series_id IS NOT NULL) AS total_series,
            (SELECT COUNT(DISTINCT series_id) FROM external_metadata_links WHERE status = 'approved') AS series_linked,
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
        SELECT provider, COUNT(DISTINCT series_id) AS count
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
        SELECT b.id AS book_id, b.title, s.name AS series, brp.current_page, b.page_count, u.username
        FROM book_reading_progress brp
        JOIN books b ON b.id = brp.book_id
        LEFT JOIN series s ON s.id = b.series_id
        LEFT JOIN users u ON u.id = brp.user_id
        WHERE brp.status = 'reading' AND brp.current_page IS NOT NULL
          AND ($1::uuid IS NULL OR brp.user_id = $1)
        ORDER BY brp.updated_at DESC
        LIMIT 20
        "#,
    )
    .bind(user_id)
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
                username: r.get("username"),
            }
        })
        .collect();

    // Recently read books
    let recent_rows = sqlx::query(
        r#"
        SELECT b.id AS book_id, b.title, s.name AS series,
               TO_CHAR(brp.last_read_at, 'YYYY-MM-DD') AS last_read_at,
               u.username
        FROM book_reading_progress brp
        JOIN books b ON b.id = brp.book_id
        LEFT JOIN series s ON s.id = b.series_id
        LEFT JOIN users u ON u.id = brp.user_id
        WHERE brp.status = 'read' AND brp.last_read_at IS NOT NULL
          AND ($1::uuid IS NULL OR brp.user_id = $1)
        ORDER BY brp.last_read_at DESC
        LIMIT 10
        "#,
    )
    .bind(user_id)
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
                last_read_at: r
                    .get::<Option<String>, _>("last_read_at")
                    .unwrap_or_default(),
                username: r.get("username"),
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
                    COALESCE(cnt.books_read, 0) AS books_read,
                    COALESCE(cnt.pages_read, 0) AS pages_read
                FROM generate_series(CURRENT_DATE - INTERVAL '6 days', CURRENT_DATE, '1 day') AS d(dt)
                LEFT JOIN (
                    SELECT brp.last_read_at::date AS dt, COUNT(*) AS books_read,
                           COALESCE(SUM(b.page_count), 0)::BIGINT AS pages_read
                    FROM book_reading_progress brp
                    JOIN books b ON b.id = brp.book_id
                    WHERE brp.status = 'read'
                      AND brp.last_read_at >= CURRENT_DATE - INTERVAL '6 days'
                      AND ($1::uuid IS NULL OR brp.user_id = $1)
                    GROUP BY brp.last_read_at::date
                ) cnt ON cnt.dt = d.dt
                ORDER BY month ASC
                "#,
            )
            .bind(user_id)
            .fetch_all(&state.pool)
            .await?
        }
        "week" => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM-DD') AS month,
                    COALESCE(cnt.books_read, 0) AS books_read,
                    COALESCE(cnt.pages_read, 0) AS pages_read
                FROM generate_series(
                    DATE_TRUNC('week', NOW() - INTERVAL '2 months'),
                    DATE_TRUNC('week', NOW()),
                    '1 week'
                ) AS d(dt)
                LEFT JOIN (
                    SELECT DATE_TRUNC('week', brp.last_read_at) AS dt, COUNT(*) AS books_read,
                           COALESCE(SUM(b.page_count), 0)::BIGINT AS pages_read
                    FROM book_reading_progress brp
                    JOIN books b ON b.id = brp.book_id
                    WHERE brp.status = 'read'
                      AND brp.last_read_at >= DATE_TRUNC('week', NOW() - INTERVAL '2 months')
                      AND ($1::uuid IS NULL OR brp.user_id = $1)
                    GROUP BY DATE_TRUNC('week', brp.last_read_at)
                ) cnt ON cnt.dt = d.dt
                ORDER BY month ASC
                "#,
            )
            .bind(user_id)
            .fetch_all(&state.pool)
            .await?
        }
        _ => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM') AS month,
                    COALESCE(cnt.books_read, 0) AS books_read,
                    COALESCE(cnt.pages_read, 0) AS pages_read
                FROM generate_series(
                    DATE_TRUNC('month', NOW()) - INTERVAL '11 months',
                    DATE_TRUNC('month', NOW()),
                    '1 month'
                ) AS d(dt)
                LEFT JOIN (
                    SELECT DATE_TRUNC('month', brp.last_read_at) AS dt, COUNT(*) AS books_read,
                           COALESCE(SUM(b.page_count), 0)::BIGINT AS pages_read
                    FROM book_reading_progress brp
                    JOIN books b ON b.id = brp.book_id
                    WHERE brp.status = 'read'
                      AND brp.last_read_at >= DATE_TRUNC('month', NOW()) - INTERVAL '11 months'
                      AND ($1::uuid IS NULL OR brp.user_id = $1)
                    GROUP BY DATE_TRUNC('month', brp.last_read_at)
                ) cnt ON cnt.dt = d.dt
                ORDER BY month ASC
                "#,
            )
            .bind(user_id)
            .fetch_all(&state.pool)
            .await?
        }
    };

    let reading_over_time: Vec<MonthlyReading> = reading_time_rows
        .iter()
        .map(|r| MonthlyReading {
            month: r.get::<Option<String>, _>("month").unwrap_or_default(),
            books_read: r.get("books_read"),
            pages_read: r.get("pages_read"),
        })
        .collect();

    // Per-user reading over time (admin view — always all users, no user_id filter)
    let users_reading_time_rows = match period {
        "day" => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM-DD') AS month,
                    u.username,
                    COALESCE(cnt.books_read, 0) AS books_read,
                    COALESCE(cnt.pages_read, 0) AS pages_read
                FROM generate_series(CURRENT_DATE - INTERVAL '6 days', CURRENT_DATE, '1 day') AS d(dt)
                CROSS JOIN users u
                LEFT JOIN (
                    SELECT brp.last_read_at::date AS dt, brp.user_id, COUNT(*) AS books_read,
                           COALESCE(SUM(b.page_count), 0)::BIGINT AS pages_read
                    FROM book_reading_progress brp
                    JOIN books b ON b.id = brp.book_id
                    WHERE brp.status = 'read'
                      AND brp.last_read_at >= CURRENT_DATE - INTERVAL '6 days'
                    GROUP BY brp.last_read_at::date, brp.user_id
                ) cnt ON cnt.dt = d.dt AND cnt.user_id = u.id
                ORDER BY month ASC, u.username
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
                    u.username,
                    COALESCE(cnt.books_read, 0) AS books_read,
                    COALESCE(cnt.pages_read, 0) AS pages_read
                FROM generate_series(
                    DATE_TRUNC('week', NOW() - INTERVAL '2 months'),
                    DATE_TRUNC('week', NOW()),
                    '1 week'
                ) AS d(dt)
                CROSS JOIN users u
                LEFT JOIN (
                    SELECT DATE_TRUNC('week', brp.last_read_at) AS dt, brp.user_id, COUNT(*) AS books_read,
                           COALESCE(SUM(b.page_count), 0)::BIGINT AS pages_read
                    FROM book_reading_progress brp
                    JOIN books b ON b.id = brp.book_id
                    WHERE brp.status = 'read'
                      AND brp.last_read_at >= DATE_TRUNC('week', NOW() - INTERVAL '2 months')
                    GROUP BY DATE_TRUNC('week', brp.last_read_at), brp.user_id
                ) cnt ON cnt.dt = d.dt AND cnt.user_id = u.id
                ORDER BY month ASC, u.username
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
                    u.username,
                    COALESCE(cnt.books_read, 0) AS books_read,
                    COALESCE(cnt.pages_read, 0) AS pages_read
                FROM generate_series(
                    DATE_TRUNC('month', NOW()) - INTERVAL '11 months',
                    DATE_TRUNC('month', NOW()),
                    '1 month'
                ) AS d(dt)
                CROSS JOIN users u
                LEFT JOIN (
                    SELECT DATE_TRUNC('month', brp.last_read_at) AS dt, brp.user_id, COUNT(*) AS books_read,
                           COALESCE(SUM(b.page_count), 0)::BIGINT AS pages_read
                    FROM book_reading_progress brp
                    JOIN books b ON b.id = brp.book_id
                    WHERE brp.status = 'read'
                      AND brp.last_read_at >= DATE_TRUNC('month', NOW()) - INTERVAL '11 months'
                    GROUP BY DATE_TRUNC('month', brp.last_read_at), brp.user_id
                ) cnt ON cnt.dt = d.dt AND cnt.user_id = u.id
                ORDER BY month ASC, u.username
                "#,
            )
            .fetch_all(&state.pool)
            .await?
        }
    };

    let users_reading_over_time: Vec<UserMonthlyReading> = users_reading_time_rows
        .iter()
        .map(|r| UserMonthlyReading {
            month: r.get::<Option<String>, _>("month").unwrap_or_default(),
            username: r.get("username"),
            books_read: r.get("books_read"),
            pages_read: r.get("pages_read"),
        })
        .collect();

    // Jobs over time (with gap filling, grouped by type category)
    let jobs_rows = match period {
        "day" => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM-DD') AS label,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'scan'), 0)::BIGINT AS scan,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'rebuild'), 0)::BIGINT AS rebuild,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'thumbnail'), 0)::BIGINT AS thumbnail,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'metadata'), 0)::BIGINT AS metadata,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'downloads'), 0)::BIGINT AS downloads,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'reading'), 0)::BIGINT AS reading,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'conversion'), 0)::BIGINT AS conversion
                FROM generate_series(CURRENT_DATE - INTERVAL '6 days', CURRENT_DATE, '1 day') AS d(dt)
                LEFT JOIN (
                    SELECT
                        finished_at::date AS dt,
                        CASE
                            WHEN type = 'scan' THEN 'scan'
                            WHEN type IN ('rebuild', 'full_rebuild', 'rescan') THEN 'rebuild'
                            WHEN type IN ('thumbnail_rebuild', 'thumbnail_regenerate') THEN 'thumbnail'
                            WHEN type IN ('metadata_batch', 'metadata_batch_rematch', 'metadata_refresh') THEN 'metadata'
                            WHEN type = 'download_detection' THEN 'downloads'
                            WHEN type IN ('reading_status_match', 'reading_status_push') THEN 'reading'
                            WHEN type = 'cbr_to_cbz' THEN 'conversion'
                            ELSE 'metadata'
                        END AS cat,
                        COUNT(*) AS c
                    FROM index_jobs
                    WHERE status IN ('success', 'failed')
                      AND finished_at >= CURRENT_DATE - INTERVAL '6 days'
                    GROUP BY finished_at::date, cat
                ) cnt ON cnt.dt = d.dt
                GROUP BY d.dt
                ORDER BY label ASC
                "#,
            )
            .fetch_all(&state.pool)
            .await?
        }
        "week" => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM-DD') AS label,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'scan'), 0)::BIGINT AS scan,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'rebuild'), 0)::BIGINT AS rebuild,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'thumbnail'), 0)::BIGINT AS thumbnail,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'metadata'), 0)::BIGINT AS metadata,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'downloads'), 0)::BIGINT AS downloads,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'reading'), 0)::BIGINT AS reading,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'conversion'), 0)::BIGINT AS conversion
                FROM generate_series(
                    DATE_TRUNC('week', NOW() - INTERVAL '2 months'),
                    DATE_TRUNC('week', NOW()),
                    '1 week'
                ) AS d(dt)
                LEFT JOIN (
                    SELECT
                        DATE_TRUNC('week', finished_at) AS dt,
                        CASE
                            WHEN type = 'scan' THEN 'scan'
                            WHEN type IN ('rebuild', 'full_rebuild', 'rescan') THEN 'rebuild'
                            WHEN type IN ('thumbnail_rebuild', 'thumbnail_regenerate') THEN 'thumbnail'
                            WHEN type IN ('metadata_batch', 'metadata_batch_rematch', 'metadata_refresh') THEN 'metadata'
                            WHEN type = 'download_detection' THEN 'downloads'
                            WHEN type IN ('reading_status_match', 'reading_status_push') THEN 'reading'
                            WHEN type = 'cbr_to_cbz' THEN 'conversion'
                            ELSE 'metadata'
                        END AS cat,
                        COUNT(*) AS c
                    FROM index_jobs
                    WHERE status IN ('success', 'failed')
                      AND finished_at >= DATE_TRUNC('week', NOW() - INTERVAL '2 months')
                    GROUP BY DATE_TRUNC('week', finished_at), cat
                ) cnt ON cnt.dt = d.dt
                GROUP BY d.dt
                ORDER BY label ASC
                "#,
            )
            .fetch_all(&state.pool)
            .await?
        }
        _ => {
            sqlx::query(
                r#"
                SELECT
                    TO_CHAR(d.dt, 'YYYY-MM') AS label,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'scan'), 0)::BIGINT AS scan,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'rebuild'), 0)::BIGINT AS rebuild,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'thumbnail'), 0)::BIGINT AS thumbnail,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'metadata'), 0)::BIGINT AS metadata,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'downloads'), 0)::BIGINT AS downloads,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'reading'), 0)::BIGINT AS reading,
                    COALESCE(SUM(cnt.c) FILTER (WHERE cnt.cat = 'conversion'), 0)::BIGINT AS conversion
                FROM generate_series(
                    DATE_TRUNC('month', NOW()) - INTERVAL '11 months',
                    DATE_TRUNC('month', NOW()),
                    '1 month'
                ) AS d(dt)
                LEFT JOIN (
                    SELECT
                        DATE_TRUNC('month', finished_at) AS dt,
                        CASE
                            WHEN type = 'scan' THEN 'scan'
                            WHEN type IN ('rebuild', 'full_rebuild', 'rescan') THEN 'rebuild'
                            WHEN type IN ('thumbnail_rebuild', 'thumbnail_regenerate') THEN 'thumbnail'
                            WHEN type IN ('metadata_batch', 'metadata_batch_rematch', 'metadata_refresh') THEN 'metadata'
                            WHEN type = 'download_detection' THEN 'downloads'
                            WHEN type IN ('reading_status_match', 'reading_status_push') THEN 'reading'
                            WHEN type = 'cbr_to_cbz' THEN 'conversion'
                            ELSE 'metadata'
                        END AS cat,
                        COUNT(*) AS c
                    FROM index_jobs
                    WHERE status IN ('success', 'failed')
                      AND finished_at >= DATE_TRUNC('month', NOW()) - INTERVAL '11 months'
                    GROUP BY DATE_TRUNC('month', finished_at), cat
                ) cnt ON cnt.dt = d.dt
                GROUP BY d.dt
                ORDER BY label ASC
                "#,
            )
            .fetch_all(&state.pool)
            .await?
        }
    };

    let jobs_over_time: Vec<JobTimePoint> = jobs_rows
        .iter()
        .map(|r| JobTimePoint {
            label: r.get("label"),
            scan: r.get("scan"),
            rebuild: r.get("rebuild"),
            thumbnail: r.get("thumbnail"),
            metadata: r.get("metadata"),
            downloads: r.get("downloads"),
            reading: r.get("reading"),
            conversion: r.get("conversion"),
        })
        .collect();

    // Download stats
    let dl_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status IN ('downloading', 'importing')) AS active_downloads,
            COUNT(*) FILTER (WHERE status = 'imported') AS imported_downloads,
            COUNT(*) FILTER (WHERE status = 'error') AS error_downloads,
            COUNT(*) AS total_downloads
        FROM torrent_downloads
        "#,
    )
    .fetch_one(&state.pool)
    .await?;

    let avail_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS available_series,
            COALESCE(SUM(missing_count), 0)::BIGINT AS total_missing_volumes
        FROM available_downloads
        "#,
    )
    .fetch_one(&state.pool)
    .await?;

    let recent_dl_rows = sqlx::query(
        r#"
        SELECT id, series_name, status, expected_volumes,
               TO_CHAR(created_at, 'YYYY-MM-DD') AS created_at
        FROM torrent_downloads
        ORDER BY created_at DESC
        LIMIT 5
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let recent_downloads: Vec<RecentDownloadItem> = recent_dl_rows
        .iter()
        .map(|r| {
            let id: uuid::Uuid = r.get("id");
            RecentDownloadItem {
                id: id.to_string(),
                series_name: r.get("series_name"),
                status: r.get("status"),
                expected_volumes: r.get("expected_volumes"),
                created_at: r.get::<Option<String>, _>("created_at").unwrap_or_default(),
            }
        })
        .collect();

    let downloads = DownloadStats {
        active_downloads: dl_row.get("active_downloads"),
        imported_downloads: dl_row.get("imported_downloads"),
        error_downloads: dl_row.get("error_downloads"),
        total_downloads: dl_row.get("total_downloads"),
        available_series: avail_row.get("available_series"),
        total_missing_volumes: avail_row.get("total_missing_volumes"),
        recent_downloads,
    };

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
        jobs_over_time,
        metadata,
        users_reading_over_time,
        downloads,
    }))
}

// ─── Reading Overview (per-user) ─────────────────────────────────────────────

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserReadingOverviewItem {
    pub book_id: String,
    pub title: String,
    pub series: Option<String>,
    pub series_id: Option<String>,
    pub current_page: i32,
    pub page_count: i32,
    pub last_read_at: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserReadingOverviewSeriesBook {
    pub book_id: String,
    pub title: String,
    pub volume: Option<i32>,
    pub volume_type: String,
    pub status: String,
    pub current_page: i32,
    pub page_count: i32,
    pub last_read_at: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserReadingOverviewSeries {
    pub series_id: Option<String>,
    pub series_name: String,
    pub books_total: i64,
    pub books_read: i64,
    pub books_reading: i64,
    pub books_unread: i64,
    pub last_read_at: Option<String>,
    pub books: Vec<UserReadingOverviewSeriesBook>,
}

#[derive(Serialize, ToSchema)]
pub struct UserReadingOverview {
    #[schema(value_type = String)]
    pub user_id: uuid::Uuid,
    pub username: String,
    pub books_read: i64,
    pub books_reading: i64,
    pub series_in_progress: i64,
    pub last_read_at: Option<String>,
    pub currently_reading: Vec<UserReadingOverviewItem>,
    pub recently_read: Vec<UserReadingOverviewItem>,
    pub series_progress: Vec<UserReadingOverviewSeries>,
}

/// Get reading overview for all users (admin)
#[utoipa::path(
    get,
    path = "/admin/reading-overview",
    tag = "stats",
    responses(
        (status = 200, body = Vec<UserReadingOverview>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_reading_overview(
    State(state): State<AppState>,
) -> Result<Json<Vec<UserReadingOverview>>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT
            u.id AS user_id,
            u.username,
            COUNT(*) FILTER (WHERE brp.status = 'read') AS books_read,
            COUNT(*) FILTER (WHERE brp.status = 'reading') AS books_reading,
            COUNT(DISTINCT CASE WHEN brp.status = 'reading' THEN b.series_id END) AS series_in_progress,
            TO_CHAR(MAX(brp.last_read_at), 'YYYY-MM-DD') AS last_read_at,
            COALESCE(
                json_agg(
                    json_build_object(
                        'book_id', b.id::text,
                        'title', b.title,
                        'series', s.name,
                        'series_id', b.series_id::text,
                        'current_page', COALESCE(brp.current_page, 0),
                        'page_count', COALESCE(b.page_count, 0),
                        'last_read_at', TO_CHAR(brp.last_read_at, 'YYYY-MM-DD')
                    ) ORDER BY brp.updated_at DESC
                ) FILTER (WHERE brp.status = 'reading'),
                '[]'::json
            ) AS currently_reading,
            (
                SELECT COALESCE(json_agg(rr), '[]'::json)
                FROM (
                    SELECT json_build_object(
                        'book_id', b2.id::text,
                        'title', b2.title,
                        'series', s2.name,
                        'series_id', b2.series_id::text,
                        'current_page', 0,
                        'page_count', COALESCE(b2.page_count, 0),
                        'last_read_at', TO_CHAR(brp2.last_read_at, 'YYYY-MM-DD')
                    ) AS rr
                    FROM book_reading_progress brp2
                    JOIN books b2 ON b2.id = brp2.book_id
                    LEFT JOIN series s2 ON s2.id = b2.series_id
                    WHERE brp2.user_id = u.id AND brp2.status = 'read'
                    ORDER BY brp2.last_read_at DESC NULLS LAST
                    LIMIT 30
                ) sub
            ) AS recently_read,
            (
                SELECT COALESCE(
                    json_agg(series_row ORDER BY sort_last_read_at DESC NULLS LAST, series_name ASC),
                    '[]'::json
                )
                FROM (
                    SELECT
                        COALESCE(s3.name, 'Sans série') AS series_name,
                        MAX(brp3.last_read_at) AS sort_last_read_at,
                        json_build_object(
                            'series_id', s3.id::text,
                            'series_name', COALESCE(s3.name, 'Sans série'),
                            'books_total', COUNT(*),
                            'books_read', COUNT(*) FILTER (WHERE COALESCE(brp3.status, 'unread') = 'read'),
                            'books_reading', COUNT(*) FILTER (WHERE COALESCE(brp3.status, 'unread') = 'reading'),
                            'books_unread', COUNT(*) FILTER (WHERE COALESCE(brp3.status, 'unread') = 'unread'),
                            'last_read_at', TO_CHAR(MAX(brp3.last_read_at), 'YYYY-MM-DD'),
                            'books', (
                                SELECT COALESCE(
                                    json_agg(
                                        json_build_object(
                                            'book_id', b4.id::text,
                                            'title', b4.title,
                                            'volume', b4.volume,
                                            'volume_type', b4.volume_type,
                                            'status', COALESCE(brp4.status, 'unread'),
                                            'current_page', COALESCE(brp4.current_page, 0),
                                            'page_count', COALESCE(b4.page_count, 0),
                                            'last_read_at', TO_CHAR(brp4.last_read_at, 'YYYY-MM-DD')
                                        )
                                        ORDER BY
                                            CASE WHEN b4.volume_type = 'regular' THEN 0 ELSE 1 END,
                                            b4.volume NULLS LAST,
                                            b4.title
                                    ),
                                    '[]'::json
                                )
                                FROM books b4
                                LEFT JOIN book_reading_progress brp4
                                    ON brp4.book_id = b4.id AND brp4.user_id = u.id
                                WHERE b4.series_id IS NOT DISTINCT FROM b3.series_id
                            )
                        ) AS series_row
                    FROM books b3
                    LEFT JOIN series s3 ON s3.id = b3.series_id
                    LEFT JOIN book_reading_progress brp3
                        ON brp3.book_id = b3.id AND brp3.user_id = u.id
                    GROUP BY b3.series_id, s3.id, s3.name
                ) series_rows
            ) AS series_progress
        FROM users u
        LEFT JOIN book_reading_progress brp ON brp.user_id = u.id
        LEFT JOIN books b ON b.id = brp.book_id
        LEFT JOIN series s ON s.id = b.series_id
        GROUP BY u.id, u.username
        ORDER BY MAX(brp.last_read_at) DESC NULLS LAST, u.username ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let overview = rows
        .into_iter()
        .map(|r| {
            let currently_json: serde_json::Value = r
                .try_get("currently_reading")
                .unwrap_or(serde_json::Value::Array(vec![]));
            let currently_reading: Vec<UserReadingOverviewItem> =
                serde_json::from_value(currently_json).unwrap_or_default();
            let recently_json: serde_json::Value = r
                .try_get("recently_read")
                .unwrap_or(serde_json::Value::Array(vec![]));
            let recently_read: Vec<UserReadingOverviewItem> =
                serde_json::from_value(recently_json).unwrap_or_default();
            let series_json: serde_json::Value = r
                .try_get("series_progress")
                .unwrap_or(serde_json::Value::Array(vec![]));
            let series_progress: Vec<UserReadingOverviewSeries> =
                serde_json::from_value(series_json).unwrap_or_default();
            UserReadingOverview {
                user_id: r.get("user_id"),
                username: r.get("username"),
                books_read: r.get("books_read"),
                books_reading: r.get("books_reading"),
                series_in_progress: r.get("series_in_progress"),
                last_read_at: r.get("last_read_at"),
                currently_reading,
                recently_read,
                series_progress,
            }
        })
        .collect();

    Ok(Json(overview))
}
