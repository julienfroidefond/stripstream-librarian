use sqlx::{postgres::PgRow, PgPool};

pub(super) async fn additions_over_time(
    pool: &PgPool,
    period: &str,
) -> Result<Vec<PgRow>, sqlx::Error> {
    match period {
            "day" => sqlx::query(
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
            .fetch_all(pool)
            .await,
            "week" => sqlx::query(
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
            .fetch_all(pool)
            .await,
            _ => sqlx::query(
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
            .fetch_all(pool)
            .await,
        }
}

pub(super) async fn reading_over_time(
    pool: &PgPool,
    period: &str,
    user_id: Option<uuid::Uuid>,
) -> Result<Vec<PgRow>, sqlx::Error> {
    match period {
            "day" => sqlx::query(
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
            .fetch_all(pool)
            .await,
            "week" => sqlx::query(
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
            .fetch_all(pool)
            .await,
            _ => sqlx::query(
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
            .fetch_all(pool)
            .await,
        }
}

pub(super) async fn users_reading_over_time(
    pool: &PgPool,
    period: &str,
    user_id: Option<uuid::Uuid>,
) -> Result<Vec<PgRow>, sqlx::Error> {
    match period {
            "day" => sqlx::query(
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
                WHERE ($1::uuid IS NULL OR u.id = $1)
                ORDER BY month ASC, u.username
                "#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await,
            "week" => sqlx::query(
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
                WHERE ($1::uuid IS NULL OR u.id = $1)
                ORDER BY month ASC, u.username
                "#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await,
            _ => sqlx::query(
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
                WHERE ($1::uuid IS NULL OR u.id = $1)
                ORDER BY month ASC, u.username
                "#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await,
        }
}

pub(super) async fn jobs_over_time(pool: &PgPool, period: &str) -> Result<Vec<PgRow>, sqlx::Error> {
    match period {
            "day" => sqlx::query(
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
                            WHEN type IN ('metadata_batch', 'metadata_batch_rematch', 'metadata_refresh', 'metadata_refresh_all') THEN 'metadata'
                            WHEN type IN ('download_detection', 'prowlarr_rss') THEN 'downloads'
                            WHEN type IN ('reading_status_match', 'reading_status_push') THEN 'reading'
                            WHEN type = 'cbr_to_cbz' THEN 'conversion'
                            ELSE 'other'
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
            .fetch_all(pool)
            .await,
            "week" => sqlx::query(
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
                            WHEN type IN ('metadata_batch', 'metadata_batch_rematch', 'metadata_refresh', 'metadata_refresh_all') THEN 'metadata'
                            WHEN type IN ('download_detection', 'prowlarr_rss') THEN 'downloads'
                            WHEN type IN ('reading_status_match', 'reading_status_push') THEN 'reading'
                            WHEN type = 'cbr_to_cbz' THEN 'conversion'
                            ELSE 'other'
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
            .fetch_all(pool)
            .await,
            _ => sqlx::query(
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
                            WHEN type IN ('metadata_batch', 'metadata_batch_rematch', 'metadata_refresh', 'metadata_refresh_all') THEN 'metadata'
                            WHEN type IN ('download_detection', 'prowlarr_rss') THEN 'downloads'
                            WHEN type IN ('reading_status_match', 'reading_status_push') THEN 'reading'
                            WHEN type = 'cbr_to_cbz' THEN 'conversion'
                            ELSE 'other'
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
            .fetch_all(pool)
            .await,
        }
}
