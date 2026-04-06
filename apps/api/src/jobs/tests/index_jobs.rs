use uuid::Uuid;

async fn create_test_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(name)
        .bind(format!("/libraries/{name}"))
        .execute(pool)
        .await
        .unwrap();
    id
}

/// The list query used in list_index_jobs / get_active_jobs / cancel_job.
const LIST_JOBS_SQL: &str =
    "SELECT j.id, j.library_id, l.name AS library_name, j.book_id, j.type, j.status, j.started_at, j.finished_at, j.stats_json, j.error_opt, j.created_at, j.progress_percent, j.processed_files, j.total_files FROM index_jobs j LEFT JOIN libraries l ON l.id = j.library_id WHERE j.id = $1";

#[sqlx::test(migrations = "../../infra/migrations")]
async fn job_with_library_has_library_name(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "My Comics").await;
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
    )
    .bind(job_id)
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    let row = sqlx::query(LIST_JOBS_SQL)
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let resp = crate::jobs::index_jobs::map_row(row);
    assert_eq!(resp.id, job_id);
    assert_eq!(resp.library_id, Some(lib_id));
    assert_eq!(resp.library_name.as_deref(), Some("My Comics"));
    assert_eq!(resp.r#type, "scan");
    assert_eq!(resp.status, "pending");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn job_without_library_has_null_library_name(pool: sqlx::PgPool) {
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, NULL, 'scan', 'pending')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    let row = sqlx::query(LIST_JOBS_SQL)
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let resp = crate::jobs::index_jobs::map_row(row);
    assert_eq!(resp.id, job_id);
    assert!(resp.library_id.is_none());
    assert!(resp.library_name.is_none(), "library_name should be null when no library_id");
}

// -- Helper to create a job and insert events --

async fn create_test_job(pool: &sqlx::PgPool) -> Uuid {
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

async fn insert_event(
    pool: &sqlx::PgPool,
    job_id: Uuid,
    event_type: &str,
    level: &str,
    entity_type: Option<&str>,
    entity_name: Option<&str>,
    message: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_name, message) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(job_id)
    .bind(event_type)
    .bind(level)
    .bind(entity_type)
    .bind(entity_name)
    .bind(message)
    .execute(pool)
    .await
    .unwrap();
}

const EVENTS_SQL: &str =
    "SELECT id, job_id, event_type, level, entity_type, entity_id, entity_name, message, detail, created_at \
     FROM index_job_events \
     WHERE job_id = $1 \
       AND ($2::text IS NULL OR level = $2) \
       AND ($3::text IS NULL OR event_type = $3) \
     ORDER BY created_at ASC \
     LIMIT $4";

fn map_event_rows(rows: Vec<sqlx::postgres::PgRow>) -> Vec<crate::jobs::index_jobs::JobEventDto> {
    use sqlx::Row;
    rows.into_iter()
        .map(|row| crate::jobs::index_jobs::JobEventDto {
            id: row.get("id"),
            job_id: row.get("job_id"),
            event_type: row.get("event_type"),
            level: row.get("level"),
            entity_type: row.get("entity_type"),
            entity_id: row.get("entity_id"),
            entity_name: row.get("entity_name"),
            message: row.get("message"),
            detail: row.get("detail"),
            created_at: row.get("created_at"),
        })
        .collect()
}

// -- get_job_events tests --

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_job_events_returns_all_events_ordered(pool: sqlx::PgPool) {
    let job_id = create_test_job(&pool).await;

    insert_event(&pool, job_id, "book_added", "info", Some("book"), Some("Book A"), None).await;
    insert_event(&pool, job_id, "book_updated", "warning", Some("book"), Some("Book B"), Some("cover missing")).await;
    insert_event(&pool, job_id, "parse_error", "error", Some("book"), Some("Book C"), Some("corrupt archive")).await;

    let rows = sqlx::query(EVENTS_SQL)
        .bind(job_id)
        .bind(None::<String>) // no level filter
        .bind(None::<String>) // no event_type filter
        .bind(500_i64)
        .fetch_all(&pool)
        .await
        .unwrap();

    let events = map_event_rows(rows);
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, "book_added");
    assert_eq!(events[1].event_type, "book_updated");
    assert_eq!(events[2].event_type, "parse_error");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_job_events_filter_by_level(pool: sqlx::PgPool) {
    let job_id = create_test_job(&pool).await;

    insert_event(&pool, job_id, "book_added", "info", None, None, None).await;
    insert_event(&pool, job_id, "book_updated", "warning", None, None, None).await;
    insert_event(&pool, job_id, "parse_error", "error", None, None, Some("bad file")).await;
    insert_event(&pool, job_id, "thumbnail_error", "error", None, None, Some("bad image")).await;

    let rows = sqlx::query(EVENTS_SQL)
        .bind(job_id)
        .bind(Some("error"))
        .bind(None::<String>)
        .bind(500_i64)
        .fetch_all(&pool)
        .await
        .unwrap();

    let events = map_event_rows(rows);
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| e.level == "error"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_job_events_filter_by_event_type(pool: sqlx::PgPool) {
    let job_id = create_test_job(&pool).await;

    insert_event(&pool, job_id, "book_added", "info", Some("book"), Some("A"), None).await;
    insert_event(&pool, job_id, "book_added", "info", Some("book"), Some("B"), None).await;
    insert_event(&pool, job_id, "book_updated", "info", Some("book"), Some("C"), None).await;

    let rows = sqlx::query(EVENTS_SQL)
        .bind(job_id)
        .bind(None::<String>)
        .bind(Some("book_added"))
        .bind(500_i64)
        .fetch_all(&pool)
        .await
        .unwrap();

    let events = map_event_rows(rows);
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| e.event_type == "book_added"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_job_events_with_limit(pool: sqlx::PgPool) {
    let job_id = create_test_job(&pool).await;

    for i in 0..10 {
        insert_event(&pool, job_id, "book_added", "info", None, Some(&format!("Book {i}")), None).await;
    }

    let rows = sqlx::query(EVENTS_SQL)
        .bind(job_id)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(3_i64)
        .fetch_all(&pool)
        .await
        .unwrap();

    let events = map_event_rows(rows);
    assert_eq!(events.len(), 3);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_job_events_empty_for_job_with_no_events(pool: sqlx::PgPool) {
    let job_id = create_test_job(&pool).await;

    let rows = sqlx::query(EVENTS_SQL)
        .bind(job_id)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(500_i64)
        .fetch_all(&pool)
        .await
        .unwrap();

    let events = map_event_rows(rows);
    assert!(events.is_empty());
}

// -----------------------------------------------------------------------
// get_job_errors: reads events WHERE level='error'
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_job_errors_reads_error_events(pool: sqlx::PgPool) {
    use sqlx::Row;

    let job_id = create_test_job(&pool).await;

    // Insert mix of error and non-error events
    insert_event(&pool, job_id, "book_added", "info", Some("book"), Some("good_file.cbz"), None).await;
    insert_event(&pool, job_id, "parse_error", "error", None, Some("/path/to/bad_file.cbz"), Some("corrupt archive")).await;
    insert_event(&pool, job_id, "thumbnail_error", "error", None, Some("/path/to/another.cbr"), Some("image decode failed")).await;
    insert_event(&pool, job_id, "book_updated", "warning", Some("book"), Some("warn_file.pdf"), Some("cover missing")).await;

    // Run the actual query used by get_job_errors
    let rows = sqlx::query(
        "SELECT id, entity_name, message, created_at
         FROM index_job_events
         WHERE job_id = $1 AND level = 'error'
         ORDER BY created_at ASC"
    )
    .bind(job_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 2, "should only return level='error' events");

    // Map the same way the endpoint does
    let errors: Vec<crate::jobs::index_jobs::JobErrorResponse> = rows
        .into_iter()
        .map(|row| crate::jobs::index_jobs::JobErrorResponse {
            id: row.get("id"),
            file_path: row.get::<Option<String>, _>("entity_name").unwrap_or_default(),
            error_message: row.get::<Option<String>, _>("message").unwrap_or_default(),
            created_at: row.get("created_at"),
        })
        .collect();

    assert_eq!(errors[0].file_path, "/path/to/bad_file.cbz");
    assert_eq!(errors[0].error_message, "corrupt archive");
    assert_eq!(errors[1].file_path, "/path/to/another.cbr");
    assert_eq!(errors[1].error_message, "image decode failed");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_job_errors_empty_when_no_errors(pool: sqlx::PgPool) {
    let job_id = create_test_job(&pool).await;

    // Insert only non-error events
    insert_event(&pool, job_id, "book_added", "info", Some("book"), Some("file.cbz"), None).await;
    insert_event(&pool, job_id, "book_updated", "warning", Some("book"), Some("file2.pdf"), Some("cover missing")).await;

    let rows = sqlx::query(
        "SELECT id, entity_name, message, created_at
         FROM index_job_events
         WHERE job_id = $1 AND level = 'error'
         ORDER BY created_at ASC"
    )
    .bind(job_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(rows.is_empty(), "should return no rows when there are no error-level events");
}
