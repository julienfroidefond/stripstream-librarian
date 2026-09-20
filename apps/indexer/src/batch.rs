use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

// Batched update data structures
pub struct BookUpdate {
    pub book_id: Uuid,
    pub title: String,
    pub kind: String,
    pub format: String,
    pub series_id: Option<Uuid>,
    pub volume: Option<i32>,
    pub volume_type: String,
    pub page_count: Option<i32>,
    pub clear_thumbnail: bool,
}

pub struct FileUpdate {
    pub file_id: Uuid,
    pub format: String,
    pub size_bytes: i64,
    pub mtime: DateTime<Utc>,
    pub fingerprint: String,
}

pub struct BookInsert {
    pub book_id: Uuid,
    pub library_id: Uuid,
    pub kind: String,
    pub format: String,
    pub title: String,
    pub series_id: Option<Uuid>,
    pub volume: Option<i32>,
    pub volume_type: String,
    pub page_count: Option<i32>,
    pub thumbnail_path: Option<String>,
}

pub struct FileInsert {
    pub file_id: Uuid,
    pub book_id: Uuid,
    pub format: String,
    pub abs_path: String,
    pub size_bytes: i64,
    pub mtime: DateTime<Utc>,
    pub fingerprint: String,
    pub parse_status: String,
    pub parse_error: Option<String>,
}

pub struct ErrorInsert {
    pub job_id: Uuid,
    pub file_path: String,
    pub error_message: String,
}

pub struct EventInsert {
    pub job_id: Uuid,
    pub event_type: String,
    pub level: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub entity_name: Option<String>,
    pub message: Option<String>,
    pub detail: Option<serde_json::Value>,
}

pub async fn flush_events(pool: &PgPool, events: &mut Vec<EventInsert>) -> Result<()> {
    if events.is_empty() {
        return Ok(());
    }

    let job_ids: Vec<Uuid> = events.iter().map(|e| e.job_id).collect();
    let event_types: Vec<String> = events.iter().map(|e| e.event_type.clone()).collect();
    let levels: Vec<String> = events.iter().map(|e| e.level.clone()).collect();
    let entity_types: Vec<Option<String>> = events.iter().map(|e| e.entity_type.clone()).collect();
    let entity_ids: Vec<Option<Uuid>> = events.iter().map(|e| e.entity_id).collect();
    let entity_names: Vec<Option<String>> = events.iter().map(|e| e.entity_name.clone()).collect();
    let messages: Vec<Option<String>> = events.iter().map(|e| e.message.clone()).collect();
    let details: Vec<Option<serde_json::Value>> = events.iter().map(|e| e.detail.clone()).collect();

    sqlx::query(
        r#"
        INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_id, entity_name, message, detail)
        SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::uuid[], $6::text[], $7::text[], $8::jsonb[])
        AS t(job_id, event_type, level, entity_type, entity_id, entity_name, message, detail)
        "#,
    )
    .bind(&job_ids)
    .bind(&event_types)
    .bind(&levels)
    .bind(&entity_types)
    .bind(&entity_ids)
    .bind(&entity_names)
    .bind(&messages)
    .bind(&details)
    .execute(pool)
    .await?;

    events.clear();
    Ok(())
}

/// Set `books.page_count` for many books in one statement.
pub async fn flush_page_counts(pool: &PgPool, updates: &mut Vec<(Uuid, i32)>) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    let book_ids: Vec<Uuid> = updates.iter().map(|(id, _)| *id).collect();
    let page_counts: Vec<i32> = updates.iter().map(|(_, pc)| *pc).collect();

    sqlx::query(
        r#"
        UPDATE books SET page_count = data.page_count, updated_at = NOW()
        FROM (
            SELECT * FROM UNNEST($1::uuid[], $2::int[]) AS t(book_id, page_count)
        ) AS data
        WHERE books.id = data.book_id
        "#,
    )
    .bind(&book_ids)
    .bind(&page_counts)
    .execute(pool)
    .await?;

    updates.clear();
    Ok(())
}

/// Set `books.page_count` and `books.thumbnail_path` for many books in one statement.
pub async fn flush_thumbnails(pool: &PgPool, updates: &mut Vec<(Uuid, i32, String)>) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    let book_ids: Vec<Uuid> = updates.iter().map(|(id, _, _)| *id).collect();
    let page_counts: Vec<i32> = updates.iter().map(|(_, pc, _)| *pc).collect();
    let thumb_paths: Vec<String> = updates.iter().map(|(_, _, p)| p.clone()).collect();

    sqlx::query(
        r#"
        UPDATE books SET page_count = data.page_count, thumbnail_path = data.thumbnail_path, updated_at = NOW()
        FROM (
            SELECT * FROM UNNEST($1::uuid[], $2::int[], $3::text[]) AS t(book_id, page_count, thumbnail_path)
        ) AS data
        WHERE books.id = data.book_id
        "#,
    )
    .bind(&book_ids)
    .bind(&page_counts)
    .bind(&thumb_paths)
    .execute(pool)
    .await?;

    updates.clear();
    Ok(())
}

/// Mark many book files as parse errors in one statement.
pub async fn flush_parse_errors(pool: &PgPool, updates: &mut Vec<(Uuid, String)>) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    let book_ids: Vec<Uuid> = updates.iter().map(|(id, _)| *id).collect();
    let errors: Vec<String> = updates.iter().map(|(_, e)| e.clone()).collect();

    sqlx::query(
        r#"
        UPDATE book_files SET parse_status = 'error', parse_error_opt = data.parse_error, updated_at = NOW()
        FROM (
            SELECT * FROM UNNEST($1::uuid[], $2::text[]) AS t(book_id, parse_error)
        ) AS data
        WHERE book_files.book_id = data.book_id
        "#,
    )
    .bind(&book_ids)
    .bind(&errors)
    .execute(pool)
    .await?;

    updates.clear();
    Ok(())
}

pub async fn flush_all_batches(
    pool: &PgPool,
    books_update: &mut Vec<BookUpdate>,
    files_update: &mut Vec<FileUpdate>,
    books_insert: &mut Vec<BookInsert>,
    files_insert: &mut Vec<FileInsert>,
    errors_insert: &mut Vec<ErrorInsert>,
    events_insert: &mut Vec<EventInsert>,
) -> Result<()> {
    if books_update.is_empty()
        && files_update.is_empty()
        && books_insert.is_empty()
        && files_insert.is_empty()
        && errors_insert.is_empty()
        && events_insert.is_empty()
    {
        return Ok(());
    }

    let start = std::time::Instant::now();
    let mut tx = pool.begin().await?;

    // Batch update books using UNNEST
    if !books_update.is_empty() {
        let book_ids: Vec<Uuid> = books_update.iter().map(|b| b.book_id).collect();
        let titles: Vec<String> = books_update.iter().map(|b| b.title.clone()).collect();
        let kinds: Vec<String> = books_update.iter().map(|b| b.kind.clone()).collect();
        let formats: Vec<String> = books_update.iter().map(|b| b.format.clone()).collect();
        let series_ids: Vec<Option<Uuid>> = books_update.iter().map(|b| b.series_id).collect();
        let volumes: Vec<Option<i32>> = books_update.iter().map(|b| b.volume).collect();
        let volume_types: Vec<String> =
            books_update.iter().map(|b| b.volume_type.clone()).collect();
        let page_counts: Vec<Option<i32>> = books_update.iter().map(|b| b.page_count).collect();
        let clear_thumbnails: Vec<bool> = books_update.iter().map(|b| b.clear_thumbnail).collect();

        sqlx::query(
            r#"
            UPDATE books SET
                title = data.title,
                kind = data.kind,
                format = data.format,
                series_id = data.series_id,
                volume = data.volume,
                volume_type = data.volume_type,
                page_count = data.page_count,
                thumbnail_path = CASE WHEN data.clear_thumbnail THEN NULL ELSE books.thumbnail_path END,
                updated_at = NOW()
            FROM (
                SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::uuid[], $6::int[], $7::text[], $8::int[], $9::bool[])
                AS t(book_id, title, kind, format, series_id, volume, volume_type, page_count, clear_thumbnail)
            ) AS data
            WHERE books.id = data.book_id
            "#
        )
        .bind(&book_ids)
        .bind(&titles)
        .bind(&kinds)
        .bind(&formats)
        .bind(&series_ids)
        .bind(&volumes)
        .bind(&volume_types)
        .bind(&page_counts)
        .bind(&clear_thumbnails)
        .execute(&mut *tx)
        .await?;

        books_update.clear();
    }

    // Batch update files using UNNEST
    if !files_update.is_empty() {
        let file_ids: Vec<Uuid> = files_update.iter().map(|f| f.file_id).collect();
        let formats: Vec<String> = files_update.iter().map(|f| f.format.clone()).collect();
        let sizes: Vec<i64> = files_update.iter().map(|f| f.size_bytes).collect();
        let mtimes: Vec<DateTime<Utc>> = files_update.iter().map(|f| f.mtime).collect();
        let fingerprints: Vec<String> =
            files_update.iter().map(|f| f.fingerprint.clone()).collect();

        sqlx::query(
            r#"
            UPDATE book_files SET 
                format = data.format,
                size_bytes = data.size,
                mtime = data.mtime,
                fingerprint = data.fp,
                parse_status = 'ok',
                parse_error_opt = NULL,
                updated_at = NOW()
            FROM (
                SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::bigint[], $4::timestamptz[], $5::text[])
                AS t(file_id, format, size, mtime, fp)
            ) AS data
            WHERE book_files.id = data.file_id
            "#
        )
        .bind(&file_ids)
        .bind(&formats)
        .bind(&sizes)
        .bind(&mtimes)
        .bind(&fingerprints)
        .execute(&mut *tx)
        .await?;

        files_update.clear();
    }

    // Batch insert books using UNNEST
    if !books_insert.is_empty() {
        let book_ids: Vec<Uuid> = books_insert.iter().map(|b| b.book_id).collect();
        let library_ids: Vec<Uuid> = books_insert.iter().map(|b| b.library_id).collect();
        let kinds: Vec<String> = books_insert.iter().map(|b| b.kind.clone()).collect();
        let formats: Vec<String> = books_insert.iter().map(|b| b.format.clone()).collect();
        let titles: Vec<String> = books_insert.iter().map(|b| b.title.clone()).collect();
        let series_ids: Vec<Option<Uuid>> = books_insert.iter().map(|b| b.series_id).collect();
        let volumes: Vec<Option<i32>> = books_insert.iter().map(|b| b.volume).collect();
        let page_counts: Vec<Option<i32>> = books_insert.iter().map(|b| b.page_count).collect();
        let volume_types: Vec<String> =
            books_insert.iter().map(|b| b.volume_type.clone()).collect();
        let thumbnail_paths: Vec<Option<String>> = books_insert
            .iter()
            .map(|b| b.thumbnail_path.clone())
            .collect();

        sqlx::query(
            r#"
            INSERT INTO books (id, library_id, kind, format, title, series_id, volume, page_count, volume_type, thumbnail_path)
            SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::text[], $4::text[], $5::text[], $6::uuid[], $7::int[], $8::int[], $9::text[], $10::text[])
            AS t(id, library_id, kind, format, title, series_id, volume, page_count, volume_type, thumbnail_path)
            "#
        )
        .bind(&book_ids)
        .bind(&library_ids)
        .bind(&kinds)
        .bind(&formats)
        .bind(&titles)
        .bind(&series_ids)
        .bind(&volumes)
        .bind(&page_counts)
        .bind(&volume_types)
        .bind(&thumbnail_paths)
        .execute(&mut *tx)
        .await?;

        books_insert.clear();
    }

    // Batch insert files using UNNEST
    if !files_insert.is_empty() {
        let file_ids: Vec<Uuid> = files_insert.iter().map(|f| f.file_id).collect();
        let book_ids: Vec<Uuid> = files_insert.iter().map(|f| f.book_id).collect();
        let formats: Vec<String> = files_insert.iter().map(|f| f.format.clone()).collect();
        let abs_paths: Vec<String> = files_insert.iter().map(|f| f.abs_path.clone()).collect();
        let sizes: Vec<i64> = files_insert.iter().map(|f| f.size_bytes).collect();
        let mtimes: Vec<DateTime<Utc>> = files_insert.iter().map(|f| f.mtime).collect();
        let fingerprints: Vec<String> =
            files_insert.iter().map(|f| f.fingerprint.clone()).collect();
        let statuses: Vec<String> = files_insert
            .iter()
            .map(|f| f.parse_status.clone())
            .collect();
        let errors: Vec<Option<String>> =
            files_insert.iter().map(|f| f.parse_error.clone()).collect();

        sqlx::query(
            r#"
            INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status, parse_error_opt)
            SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::text[], $4::text[], $5::bigint[], $6::timestamptz[], $7::text[], $8::text[], $9::text[])
            AS t(id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status, parse_error_opt)
            "#
        )
        .bind(&file_ids)
        .bind(&book_ids)
        .bind(&formats)
        .bind(&abs_paths)
        .bind(&sizes)
        .bind(&mtimes)
        .bind(&fingerprints)
        .bind(&statuses)
        .bind(&errors)
        .execute(&mut *tx)
        .await?;

        files_insert.clear();
    }

    // Errors are now tracked via index_job_events (no longer written to index_job_errors)
    errors_insert.clear();

    // Batch insert events using UNNEST
    if !events_insert.is_empty() {
        let job_ids: Vec<Uuid> = events_insert.iter().map(|e| e.job_id).collect();
        let event_types: Vec<String> = events_insert.iter().map(|e| e.event_type.clone()).collect();
        let levels: Vec<String> = events_insert.iter().map(|e| e.level.clone()).collect();
        let entity_types: Vec<Option<String>> = events_insert
            .iter()
            .map(|e| e.entity_type.clone())
            .collect();
        let entity_ids: Vec<Option<Uuid>> = events_insert.iter().map(|e| e.entity_id).collect();
        let entity_names: Vec<Option<String>> = events_insert
            .iter()
            .map(|e| e.entity_name.clone())
            .collect();
        let messages: Vec<Option<String>> =
            events_insert.iter().map(|e| e.message.clone()).collect();
        let details: Vec<Option<serde_json::Value>> =
            events_insert.iter().map(|e| e.detail.clone()).collect();

        sqlx::query(
            r#"
            INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_id, entity_name, message, detail)
            SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::uuid[], $6::text[], $7::text[], $8::jsonb[])
            AS t(job_id, event_type, level, entity_type, entity_id, entity_name, message, detail)
            "#,
        )
        .bind(&job_ids)
        .bind(&event_types)
        .bind(&levels)
        .bind(&entity_types)
        .bind(&entity_ids)
        .bind(&entity_names)
        .bind(&messages)
        .bind(&details)
        .execute(&mut *tx)
        .await?;

        events_insert.clear();
    }

    tx.commit().await?;
    tracing::info!("[BATCH] Flushed all batches in {:?}", start.elapsed());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    async fn create_test_job(pool: &PgPool) -> Uuid {
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, NULL, 'scan', 'running')",
        )
        .bind(job_id)
        .execute(pool)
        .await
        .unwrap();
        job_id
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn flush_events_inserts_all_events(pool: PgPool) {
        let job_id = create_test_job(&pool).await;
        let entity_id = Uuid::new_v4();

        let mut events = vec![
            EventInsert {
                job_id,
                event_type: "book_added".to_string(),
                level: "info".to_string(),
                entity_type: Some("book".to_string()),
                entity_id: Some(entity_id),
                entity_name: Some("My Comic".to_string()),
                message: Some("New book discovered".to_string()),
                detail: Some(serde_json::json!({"pages": 42})),
            },
            EventInsert {
                job_id,
                event_type: "parse_error".to_string(),
                level: "error".to_string(),
                entity_type: Some("book".to_string()),
                entity_id: None,
                entity_name: Some("Bad File".to_string()),
                message: Some("corrupt archive".to_string()),
                detail: None,
            },
            EventInsert {
                job_id,
                event_type: "series_created".to_string(),
                level: "info".to_string(),
                entity_type: Some("series".to_string()),
                entity_id: Some(Uuid::new_v4()),
                entity_name: Some("My Series".to_string()),
                message: None,
                detail: None,
            },
        ];

        flush_events(&pool, &mut events).await.unwrap();

        // Vec should be cleared after flush
        assert!(events.is_empty());

        // Verify rows in DB
        let rows: Vec<(String, String)> = sqlx::query(
            "SELECT event_type, level FROM index_job_events WHERE job_id = $1 ORDER BY created_at ASC",
        )
        .bind(job_id)
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|r| (r.get("event_type"), r.get("level")))
        .collect();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], ("book_added".to_string(), "info".to_string()));
        assert_eq!(rows[1], ("parse_error".to_string(), "error".to_string()));
        assert_eq!(rows[2], ("series_created".to_string(), "info".to_string()));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn flush_events_with_empty_vec_is_noop(pool: PgPool) {
        let mut events: Vec<EventInsert> = vec![];
        // Should not error even without a valid job
        flush_events(&pool, &mut events).await.unwrap();
        assert!(events.is_empty());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn flush_events_with_various_entity_types(pool: PgPool) {
        let job_id = create_test_job(&pool).await;

        let mut events = vec![
            EventInsert {
                job_id,
                event_type: "book_added".to_string(),
                level: "info".to_string(),
                entity_type: Some("book".to_string()),
                entity_id: Some(Uuid::new_v4()),
                entity_name: Some("Comic 1".to_string()),
                message: None,
                detail: None,
            },
            EventInsert {
                job_id,
                event_type: "series_created".to_string(),
                level: "info".to_string(),
                entity_type: Some("series".to_string()),
                entity_id: Some(Uuid::new_v4()),
                entity_name: Some("My Series".to_string()),
                message: None,
                detail: None,
            },
            EventInsert {
                job_id,
                event_type: "scan_started".to_string(),
                level: "info".to_string(),
                entity_type: None,
                entity_id: None,
                entity_name: None,
                message: Some("Scan started".to_string()),
                detail: None,
            },
        ];

        flush_events(&pool, &mut events).await.unwrap();
        assert!(events.is_empty());

        // Verify entity_type values
        let entity_types: Vec<Option<String>> = sqlx::query(
            "SELECT entity_type FROM index_job_events WHERE job_id = $1 ORDER BY created_at ASC",
        )
        .bind(job_id)
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.get("entity_type"))
        .collect();

        assert_eq!(entity_types.len(), 3);
        assert_eq!(entity_types[0], Some("book".to_string()));
        assert_eq!(entity_types[1], Some("series".to_string()));
        assert_eq!(entity_types[2], None);
    }

    async fn create_test_library(pool: &PgPool) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind("Test Library")
            .bind(format!("/tmp/{}", library_id))
            .execute(pool)
            .await
            .unwrap();
        library_id
    }

    async fn create_test_book(pool: &PgPool, library_id: Uuid) -> Uuid {
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, kind, title, format) VALUES ($1, $2, 'comic', 'Test', 'cbz')",
        )
        .bind(book_id)
        .bind(library_id)
        .execute(pool)
        .await
        .unwrap();
        book_id
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn flush_page_counts_updates_all_books(pool: PgPool) {
        let library_id = create_test_library(&pool).await;
        let book_a = create_test_book(&pool, library_id).await;
        let book_b = create_test_book(&pool, library_id).await;

        let mut updates = vec![(book_a, 42), (book_b, 7)];
        flush_page_counts(&pool, &mut updates).await.unwrap();
        assert!(updates.is_empty());

        let counts: Vec<(Uuid, Option<i32>)> =
            sqlx::query_as("SELECT id, page_count FROM books WHERE id = ANY($1) ORDER BY id")
                .bind(&[book_a, book_b][..])
                .fetch_all(&pool)
                .await
                .unwrap();

        assert_eq!(counts.len(), 2);
        let map: std::collections::HashMap<Uuid, Option<i32>> = counts.into_iter().collect();
        assert_eq!(map[&book_a], Some(42));
        assert_eq!(map[&book_b], Some(7));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn flush_page_counts_with_empty_vec_is_noop(pool: PgPool) {
        let mut updates: Vec<(Uuid, i32)> = vec![];
        flush_page_counts(&pool, &mut updates).await.unwrap();
        assert!(updates.is_empty());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn flush_thumbnails_updates_page_count_and_path(pool: PgPool) {
        let library_id = create_test_library(&pool).await;
        let book_a = create_test_book(&pool, library_id).await;
        let book_b = create_test_book(&pool, library_id).await;

        let mut updates = vec![
            (book_a, 10, "/data/thumbnails/a.webp".to_string()),
            (book_b, 20, "/data/thumbnails/b.webp".to_string()),
        ];
        flush_thumbnails(&pool, &mut updates).await.unwrap();
        assert!(updates.is_empty());

        let rows: Vec<(Uuid, Option<i32>, Option<String>)> = sqlx::query_as(
            "SELECT id, page_count, thumbnail_path FROM books WHERE id = ANY($1) ORDER BY id",
        )
        .bind(&[book_a, book_b][..])
        .fetch_all(&pool)
        .await
        .unwrap();

        let map: std::collections::HashMap<Uuid, (Option<i32>, Option<String>)> = rows
            .into_iter()
            .map(|(id, pc, tp)| (id, (pc, tp)))
            .collect();
        assert_eq!(map[&book_a].0, Some(10));
        assert_eq!(map[&book_a].1.as_deref(), Some("/data/thumbnails/a.webp"));
        assert_eq!(map[&book_b].0, Some(20));
        assert_eq!(map[&book_b].1.as_deref(), Some("/data/thumbnails/b.webp"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn flush_parse_errors_marks_files(pool: PgPool) {
        let library_id = create_test_library(&pool).await;
        let book_a = create_test_book(&pool, library_id).await;
        let book_b = create_test_book(&pool, library_id).await;

        for (i, book_id) in [book_a, book_b].iter().enumerate() {
            sqlx::query(
                "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint)
                 VALUES ($1, $2, 'cbz', $3, 100, NOW(), 'fp')",
            )
            .bind(Uuid::new_v4())
            .bind(book_id)
            .bind(format!("/tmp/{}/file{}.cbz", library_id, i))
            .execute(&pool)
            .await
            .unwrap();
        }

        let mut updates = vec![
            (book_a, "corrupt archive".to_string()),
            (book_b, "timed out".to_string()),
        ];
        flush_parse_errors(&pool, &mut updates).await.unwrap();
        assert!(updates.is_empty());

        let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
            "SELECT book_id, parse_status, parse_error_opt FROM book_files WHERE book_id = ANY($1) ORDER BY book_id",
        )
        .bind(&[book_a, book_b][..])
        .fetch_all(&pool)
        .await
        .unwrap();

        let map: std::collections::HashMap<Uuid, (String, Option<String>)> =
            rows.into_iter().map(|(id, s, e)| (id, (s, e))).collect();
        assert_eq!(map[&book_a].0, "error");
        assert_eq!(map[&book_a].1.as_deref(), Some("corrupt archive"));
        assert_eq!(map[&book_b].0, "error");
        assert_eq!(map[&book_b].1.as_deref(), Some("timed out"));
    }
}
