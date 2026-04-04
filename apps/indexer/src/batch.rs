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
    pub page_count: Option<i32>,
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

pub async fn flush_all_batches(
    pool: &PgPool,
    books_update: &mut Vec<BookUpdate>,
    files_update: &mut Vec<FileUpdate>,
    books_insert: &mut Vec<BookInsert>,
    files_insert: &mut Vec<FileInsert>,
    errors_insert: &mut Vec<ErrorInsert>,
    events_insert: &mut Vec<EventInsert>,
) -> Result<()> {
    if books_update.is_empty() && files_update.is_empty() && books_insert.is_empty() && files_insert.is_empty() && errors_insert.is_empty() && events_insert.is_empty() {
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
        let page_counts: Vec<Option<i32>> = books_update.iter().map(|b| b.page_count).collect();

        sqlx::query(
            r#"
            UPDATE books SET
                title = data.title,
                kind = data.kind,
                format = data.format,
                series_id = data.series_id,
                volume = data.volume,
                page_count = data.page_count,
                updated_at = NOW()
            FROM (
                SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::uuid[], $6::int[], $7::int[])
                AS t(book_id, title, kind, format, series_id, volume, page_count)
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
        .bind(&page_counts)
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
        let fingerprints: Vec<String> = files_update.iter().map(|f| f.fingerprint.clone()).collect();
        
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
        let thumbnail_paths: Vec<Option<String>> = books_insert.iter().map(|b| b.thumbnail_path.clone()).collect();

        sqlx::query(
            r#"
            INSERT INTO books (id, library_id, kind, format, title, series_id, volume, page_count, thumbnail_path)
            SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::text[], $4::text[], $5::text[], $6::uuid[], $7::int[], $8::int[], $9::text[])
            AS t(id, library_id, kind, format, title, series_id, volume, page_count, thumbnail_path)
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
        let fingerprints: Vec<String> = files_insert.iter().map(|f| f.fingerprint.clone()).collect();
        let statuses: Vec<String> = files_insert.iter().map(|f| f.parse_status.clone()).collect();
        let errors: Vec<Option<String>> = files_insert.iter().map(|f| f.parse_error.clone()).collect();
        
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
    
    // Batch insert errors using UNNEST
    if !errors_insert.is_empty() {
        let job_ids: Vec<Uuid> = errors_insert.iter().map(|e| e.job_id).collect();
        let file_paths: Vec<String> = errors_insert.iter().map(|e| e.file_path.clone()).collect();
        let messages: Vec<String> = errors_insert.iter().map(|e| e.error_message.clone()).collect();
        
        sqlx::query(
            r#"
            INSERT INTO index_job_errors (job_id, file_path, error_message)
            SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[])
            AS t(job_id, file_path, error_message)
            "#
        )
        .bind(&job_ids)
        .bind(&file_paths)
        .bind(&messages)
        .execute(&mut *tx)
        .await?;
        
        errors_insert.clear();
    }
    
    // Batch insert events using UNNEST
    if !events_insert.is_empty() {
        let job_ids: Vec<Uuid> = events_insert.iter().map(|e| e.job_id).collect();
        let event_types: Vec<String> = events_insert.iter().map(|e| e.event_type.clone()).collect();
        let levels: Vec<String> = events_insert.iter().map(|e| e.level.clone()).collect();
        let entity_types: Vec<Option<String>> = events_insert.iter().map(|e| e.entity_type.clone()).collect();
        let entity_ids: Vec<Option<Uuid>> = events_insert.iter().map(|e| e.entity_id).collect();
        let entity_names: Vec<Option<String>> = events_insert.iter().map(|e| e.entity_name.clone()).collect();
        let messages: Vec<Option<String>> = events_insert.iter().map(|e| e.message.clone()).collect();
        let details: Vec<Option<serde_json::Value>> = events_insert.iter().map(|e| e.detail.clone()).collect();

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
}
