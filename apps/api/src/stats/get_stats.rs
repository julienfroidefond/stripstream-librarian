use axum::{
    extract::{Extension, Query, State},
    Json,
};
use sqlx::Row;

use crate::{auth::AuthUser, error::ApiError, state::AppState};

use super::period;
use super::types::*;

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
    if !matches!(period, "day" | "week" | "month") {
        return Err(ApiError::bad_request(
            "period must be one of: day, week, month",
        ));
    }
    let pool = &state.pool;

    // Build period-dependent futures upfront so they can run concurrently
    let additions_fut = period::additions_over_time(pool, period);
    let reading_time_fut = period::reading_over_time(pool, period, user_id);
    let users_reading_time_fut = period::users_reading_over_time(pool, period, user_id);
    let jobs_fut = period::jobs_over_time(pool, period);

    // Execute all 17 independent queries concurrently
    let (
        overview_row,
        size_row,
        format_rows,
        lang_rows,
        lib_rows,
        series_rows,
        additions_rows,
        meta_row,
        provider_rows,
        reading_rows,
        recent_rows,
        reading_time_rows,
        users_reading_time_rows,
        jobs_rows,
        dl_row,
        avail_row,
        recent_dl_rows,
    ) = tokio::try_join!(
        sqlx::query(
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
        .fetch_one(pool),
        sqlx::query(
            r#"
            SELECT COALESCE(SUM(bf.size_bytes), 0)::BIGINT AS total_size_bytes
            FROM (
                SELECT DISTINCT ON (book_id) size_bytes
                FROM book_files
                ORDER BY book_id, updated_at DESC
            ) bf
            "#,
        )
        .fetch_one(pool),
        sqlx::query(
            r#"
            SELECT COALESCE(bf.format, 'unknown') AS fmt, COUNT(*) AS count
            FROM books b
            LEFT JOIN LATERAL (
                SELECT format FROM book_files WHERE book_id = b.id ORDER BY updated_at DESC LIMIT 1
            ) bf ON TRUE
            GROUP BY fmt
            ORDER BY count DESC
            "#,
        )
        .fetch_all(pool),
        sqlx::query(
            r#"
            SELECT language, COUNT(*) AS count
            FROM books
            GROUP BY language
            ORDER BY count DESC
            "#,
        )
        .fetch_all(pool),
        sqlx::query(
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
        .fetch_all(pool),
        sqlx::query(
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
        .fetch_all(pool),
        additions_fut,
        sqlx::query(
            r#"
            SELECT
                (SELECT COUNT(DISTINCT series_id) FROM books WHERE series_id IS NOT NULL) AS total_series,
                (SELECT COUNT(DISTINCT series_id) FROM external_metadata_links WHERE status = 'approved') AS series_linked,
                (SELECT COUNT(*) FROM books WHERE summary IS NOT NULL AND summary != '') AS books_with_summary,
                (SELECT COUNT(*) FROM books WHERE isbn IS NOT NULL AND isbn != '') AS books_with_isbn
            "#,
        )
        .fetch_one(pool),
        sqlx::query(
            r#"
            SELECT provider, COUNT(DISTINCT series_id) AS count
            FROM external_metadata_links
            WHERE status = 'approved'
            GROUP BY provider
            ORDER BY count DESC
            "#,
        )
        .fetch_all(pool),
        sqlx::query(
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
        .fetch_all(pool),
        sqlx::query(
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
        .fetch_all(pool),
        reading_time_fut,
        users_reading_time_fut,
        jobs_fut,
        sqlx::query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status IN ('downloading', 'importing')) AS active_downloads,
                COUNT(*) FILTER (WHERE status = 'imported') AS imported_downloads,
                COUNT(*) FILTER (WHERE status = 'error') AS error_downloads,
                COUNT(*) AS total_downloads
            FROM torrent_downloads
            "#,
        )
        .fetch_one(pool),
        sqlx::query(
            r#"
            SELECT
                COUNT(*) AS available_series,
                COALESCE(SUM(missing_count), 0)::BIGINT AS total_missing_volumes
            FROM available_downloads
            "#,
        )
        .fetch_one(pool),
        sqlx::query(
            r#"
            SELECT id, series_name, status, expected_volumes,
                   TO_CHAR(created_at, 'YYYY-MM-DD') AS created_at
            FROM torrent_downloads
            ORDER BY created_at DESC
            LIMIT 5
            "#,
        )
        .fetch_all(pool),
    )?;

    // Build response from rows
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

    let by_format: Vec<FormatCount> = format_rows
        .iter()
        .map(|r| FormatCount {
            format: r
                .get::<Option<String>, _>("fmt")
                .unwrap_or_else(|| "unknown".to_string()),
            count: r.get("count"),
        })
        .collect();

    let by_language: Vec<LanguageCount> = lang_rows
        .iter()
        .map(|r| LanguageCount {
            language: r.get("language"),
            count: r.get("count"),
        })
        .collect();

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

    let top_series: Vec<TopSeries> = series_rows
        .iter()
        .map(|r| TopSeries {
            series: r.get("series"),
            book_count: r.get("book_count"),
            read_count: r.get("read_count"),
            total_pages: r.get("total_pages"),
        })
        .collect();

    let additions_over_time: Vec<MonthlyAdditions> = additions_rows
        .iter()
        .map(|r| MonthlyAdditions {
            month: r.get("month"),
            books_added: r.get("books_added"),
        })
        .collect();

    let meta_total_series: i64 = meta_row.get("total_series");
    let meta_series_linked: i64 = meta_row.get("series_linked");

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

    let reading_over_time: Vec<MonthlyReading> = reading_time_rows
        .iter()
        .map(|r| MonthlyReading {
            month: r.get::<Option<String>, _>("month").unwrap_or_default(),
            books_read: r.get("books_read"),
            pages_read: r.get("pages_read"),
        })
        .collect();

    let users_reading_over_time: Vec<UserMonthlyReading> = users_reading_time_rows
        .iter()
        .map(|r| UserMonthlyReading {
            month: r.get::<Option<String>, _>("month").unwrap_or_default(),
            username: r.get("username"),
            books_read: r.get("books_read"),
            pages_read: r.get("pages_read"),
        })
        .collect();

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
