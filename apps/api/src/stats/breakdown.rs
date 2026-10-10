use axum::{
    extract::{Extension, State},
    Json,
};
use sqlx::Row;

use crate::{auth::AuthUser, error::ApiError, state::AppState};

use super::types::*;

pub async fn get_stats_breakdown(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<StatsBreakdownResponse>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let pool = &state.pool;

    let (
        lib_rows,
        series_rows,
        dl_row,
        avail_row,
        recent_dl_rows,
    ) = tokio::try_join!(
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

    Ok(Json(StatsBreakdownResponse {
        by_library: lib_rows
            .iter()
            .map(|r| LibraryStats {
                library_name: r.get("library_name"),
                book_count: r.get("book_count"),
                size_bytes: r.get("size_bytes"),
                read_count: r.get("read_count"),
                reading_count: r.get("reading_count"),
                unread_count: r.get("unread_count"),
            })
            .collect(),
        top_series: series_rows
            .iter()
            .map(|r| TopSeries {
                series: r.get("series"),
                book_count: r.get("book_count"),
                read_count: r.get("read_count"),
                total_pages: r.get("total_pages"),
            })
            .collect(),
        downloads: DownloadStats {
            active_downloads: dl_row.get("active_downloads"),
            imported_downloads: dl_row.get("imported_downloads"),
            error_downloads: dl_row.get("error_downloads"),
            total_downloads: dl_row.get("total_downloads"),
            available_series: avail_row.get("available_series"),
            total_missing_volumes: avail_row.get("total_missing_volumes"),
            recent_downloads: recent_dl_rows
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
                .collect(),
        },
    }))
}
