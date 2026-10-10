use axum::{
    extract::{Extension, State},
    Json,
};
use sqlx::Row;

use crate::{auth::AuthUser, error::ApiError, state::AppState};

use super::types::*;

pub async fn get_stats_overview(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<StatsOverviewResponse>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let pool = &state.pool;

    let (
        overview_row,
        size_row,
        format_rows,
        lang_rows,
        meta_row,
        provider_rows,
        reading_rows,
        recent_rows,
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
    )?;

    let meta_total_series: i64 = meta_row.get("total_series");
    let meta_series_linked: i64 = meta_row.get("series_linked");

    Ok(Json(StatsOverviewResponse {
        overview: StatsOverview {
            total_books: overview_row.get("total_books"),
            total_series: overview_row.get("total_series"),
            total_libraries: overview_row.get("total_libraries"),
            total_pages: overview_row.get("total_pages"),
            total_size_bytes: size_row.get("total_size_bytes"),
            total_authors: overview_row.get("total_authors"),
        },
        reading_status: ReadingStatusStats {
            unread: overview_row.get("unread"),
            reading: overview_row.get("reading"),
            read: overview_row.get("read"),
        },
        by_format: format_rows
            .iter()
            .map(|r| FormatCount {
                format: r
                    .get::<Option<String>, _>("fmt")
                    .unwrap_or_else(|| "unknown".to_string()),
                count: r.get("count"),
            })
            .collect(),
        by_language: lang_rows
            .iter()
            .map(|r| LanguageCount {
                language: r.get("language"),
                count: r.get("count"),
            })
            .collect(),
        metadata: MetadataStats {
            total_series: meta_total_series,
            series_linked: meta_series_linked,
            series_unlinked: meta_total_series - meta_series_linked,
            books_with_summary: meta_row.get("books_with_summary"),
            books_with_isbn: meta_row.get("books_with_isbn"),
            by_provider: provider_rows
                .iter()
                .map(|r| ProviderCount {
                    provider: r.get("provider"),
                    count: r.get("count"),
                })
                .collect(),
        },
        currently_reading: reading_rows
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
            .collect(),
        recently_read: recent_rows
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
            .collect(),
    }))
}
