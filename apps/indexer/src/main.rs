use anyhow::Context;
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use axum::http::StatusCode;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use parsers::{detect_format, parse_metadata, BookFormat};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, Row};
use std::{collections::HashMap, path::Path, time::Duration};
use stripstream_core::config::IndexerConfig;
use tokio::sync::mpsc;
use tracing::{error, info, trace, warn};
use uuid::Uuid;
use walkdir::WalkDir;

fn remap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with("/libraries/") {
            return path.replacen("/libraries", &root, 1);
        }
    }
    path.to_string()
}

fn unmap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with(&root) {
            return path.replacen(&root, "/libraries", 1);
        }
    }
    path.to_string()
}

#[derive(Clone)]
struct AppState {
    pool: sqlx::PgPool,
    meili_url: String,
    meili_master_key: String,
}

#[derive(Serialize)]
struct JobStats {
    scanned_files: usize,
    indexed_files: usize,
    removed_files: usize,
    errors: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "indexer=info,axum=info".to_string()),
        )
        .init();

    let config = IndexerConfig::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await?;

    let state = AppState {
        pool,
        meili_url: config.meili_url.clone(),
        meili_master_key: config.meili_master_key.clone(),
    };

    tokio::spawn(run_worker(state.clone(), config.scan_interval_seconds));

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "indexer listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(serde_json::json!({"status": "ready"})))
}

async fn run_worker(state: AppState, interval_seconds: u64) {
    let wait = Duration::from_secs(interval_seconds.max(1));

    // Start file watcher task
    let watcher_state = state.clone();
    let _watcher_handle = tokio::spawn(async move {
        info!("[WATCHER] Starting file watcher service");
        if let Err(err) = run_file_watcher(watcher_state).await {
            error!("[WATCHER] Error: {}", err);
        }
    });

    // Start scheduler task for auto-monitoring
    let scheduler_state = state.clone();
    let _scheduler_handle = tokio::spawn(async move {
        let scheduler_wait = Duration::from_secs(60); // Check every minute
        loop {
            if let Err(err) = check_and_schedule_auto_scans(&scheduler_state.pool).await {
                error!("[SCHEDULER] Error: {}", err);
            }
            tokio::time::sleep(scheduler_wait).await;
        }
    });

    loop {
        match claim_next_job(&state.pool).await {
            Ok(Some((job_id, library_id))) => {
                info!("[INDEXER] Starting job {} library={:?}", job_id, library_id);
                if let Err(err) = process_job(&state, job_id, library_id).await {
                    error!("[INDEXER] Job {} failed: {}", job_id, err);
                    let _ = fail_job(&state.pool, job_id, &err.to_string()).await;
                } else {
                    info!("[INDEXER] Job {} completed", job_id);
                }
            }
            Ok(None) => {
                trace!("[INDEXER] No pending jobs, waiting...");
                tokio::time::sleep(wait).await;
            }
            Err(err) => {
                error!("[INDEXER] Worker error: {}", err);
                tokio::time::sleep(wait).await;
            }
        }
    }
}

async fn run_file_watcher(state: AppState) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel::<(Uuid, String)>(100);

    // Start watcher refresh loop
    let refresh_interval = Duration::from_secs(30);
    let pool = state.pool.clone();

    tokio::spawn(async move {
        let mut watcher: Option<RecommendedWatcher> = None;
        let mut watched_libraries: HashMap<Uuid, String> = HashMap::new();

        loop {
            // Get libraries with watcher enabled
            match sqlx::query(
                "SELECT id, root_path FROM libraries WHERE watcher_enabled = TRUE AND enabled = TRUE"
            )
            .fetch_all(&pool)
            .await
            {
                Ok(rows) => {
                    let current_libraries: HashMap<Uuid, String> = rows
                        .into_iter()
                        .map(|row| {
                            let id: Uuid = row.get("id");
                            let root_path: String = row.get("root_path");
                            let local_path = remap_libraries_path(&root_path);
                            (id, local_path)
                        })
                        .collect();

                    // Check if we need to recreate watcher
                    let needs_restart = watched_libraries.len() != current_libraries.len()
                        || watched_libraries.iter().any(|(id, path)| {
                            current_libraries.get(id) != Some(path)
                        });

                    if needs_restart {
                        info!("[WATCHER] Restarting watcher for {} libraries", current_libraries.len());

                        // Drop old watcher
                        watcher = None;
                        watched_libraries.clear();

                        if !current_libraries.is_empty() {
                            let tx_clone = tx.clone();
                            let libraries_clone = current_libraries.clone();

                            match setup_watcher(libraries_clone, tx_clone) {
                                Ok(new_watcher) => {
                                    watcher = Some(new_watcher);
                                    watched_libraries = current_libraries;
                                    info!("[WATCHER] Watching {} libraries", watched_libraries.len());
                                }
                                Err(err) => {
                                    error!("[WATCHER] Failed to setup watcher: {}", err);
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    error!("[WATCHER] Failed to fetch libraries: {}", err);
                }
            }

            tokio::time::sleep(refresh_interval).await;
        }
    });

    // Process watcher events
    while let Some((library_id, file_path)) = rx.recv().await {
        info!("[WATCHER] File changed in library {}: {}", library_id, file_path);

        // Check if there's already a pending job for this library
        match sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM index_jobs WHERE library_id = $1 AND status IN ('pending', 'running'))"
        )
        .bind(library_id)
        .fetch_one(&state.pool)
        .await
        {
            Ok(exists) => {
                if !exists {
                    // Create a quick scan job
                    let job_id = Uuid::new_v4();
                    match sqlx::query(
                        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'rebuild', 'pending')"
                    )
                    .bind(job_id)
                    .bind(library_id)
                    .execute(&state.pool)
                    .await
                    {
                        Ok(_) => info!("[WATCHER] Created job {} for library {}", job_id, library_id),
                        Err(err) => error!("[WATCHER] Failed to create job: {}", err),
                    }
                } else {
                    trace!("[WATCHER] Job already pending for library {}, skipping", library_id);
                }
            }
            Err(err) => error!("[WATCHER] Failed to check existing jobs: {}", err),
        }
    }

    Ok(())
}

fn setup_watcher(
    libraries: HashMap<Uuid, String>,
    tx: mpsc::Sender<(Uuid, String)>,
) -> anyhow::Result<RecommendedWatcher> {
    let libraries_for_closure = libraries.clone();
    
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        match res {
            Ok(event) => {
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    for path in event.paths {
                        if let Some((library_id, _)) = libraries_for_closure.iter().find(|(_, root)| {
                            path.starts_with(root)
                        }) {
                            let path_str = path.to_string_lossy().to_string();
                            if detect_format(&path).is_some() {
                                let _ = tx.try_send((*library_id, path_str));
                            }
                        }
                    }
                }
            }
            Err(err) => error!("[WATCHER] Event error: {}", err),
        }
    })?;

    // Actually watch the library directories
    for (_, root_path) in &libraries {
        info!("[WATCHER] Watching directory: {}", root_path);
        watcher.watch(std::path::Path::new(root_path), RecursiveMode::Recursive)?;
    }

    Ok(watcher)
}

async fn check_and_schedule_auto_scans(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let libraries = sqlx::query(
        r#"
        SELECT id, scan_mode, last_scan_at
        FROM libraries
        WHERE monitor_enabled = TRUE
          AND (
            next_scan_at IS NULL
            OR next_scan_at <= NOW()
          )
          AND NOT EXISTS (
            SELECT 1 FROM index_jobs
            WHERE library_id = libraries.id
              AND status IN ('pending', 'running')
          )
        "#
    )
    .fetch_all(pool)
    .await?;

    for row in libraries {
        let library_id: Uuid = row.get("id");
        let scan_mode: String = row.get("scan_mode");
        
        info!("[SCHEDULER] Auto-scanning library {} (mode: {})", library_id, scan_mode);
        
        let job_id = Uuid::new_v4();
        let job_type = match scan_mode.as_str() {
            "full" => "full_rebuild",
            _ => "rebuild",
        };
        
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, $3, 'pending')"
        )
        .bind(job_id)
        .bind(library_id)
        .bind(job_type)
        .execute(pool)
        .await?;
        
        // Update next_scan_at
        let interval_minutes = match scan_mode.as_str() {
            "hourly" => 60,
            "daily" => 1440,
            "weekly" => 10080,
            _ => 1440, // default daily
        };
        
        sqlx::query(
            "UPDATE libraries SET last_scan_at = NOW(), next_scan_at = NOW() + INTERVAL '1 minute' * $2 WHERE id = $1"
        )
        .bind(library_id)
        .bind(interval_minutes)
        .execute(pool)
        .await?;
        
        info!("[SCHEDULER] Created job {} for library {}", job_id, library_id);
    }
    
    Ok(())
}

async fn claim_next_job(pool: &sqlx::PgPool) -> anyhow::Result<Option<(Uuid, Option<Uuid>)>> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        r#"
        SELECT id, library_id
        FROM index_jobs
        WHERE status = 'pending'
        ORDER BY created_at ASC
        FOR UPDATE SKIP LOCKED
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let id: Uuid = row.get("id");
    let library_id: Option<Uuid> = row.get("library_id");

    sqlx::query("UPDATE index_jobs SET status = 'running', started_at = NOW(), error_opt = NULL WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(Some((id, library_id)))
}

async fn process_job(state: &AppState, job_id: Uuid, target_library_id: Option<Uuid>) -> anyhow::Result<()> {
    info!("[JOB] Processing {} library={:?}", job_id, target_library_id);
    
    // Get job type to check if it's a full rebuild
    let job_type: String = sqlx::query_scalar("SELECT type FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&state.pool)
        .await?;
    let is_full_rebuild = job_type == "full_rebuild";
    info!("[JOB] {} type={} full_rebuild={}", job_id, job_type, is_full_rebuild);

    // For full rebuilds, delete existing data first
    if is_full_rebuild {
        info!("[JOB] Full rebuild: deleting existing data");
        if let Some(library_id) = target_library_id {
            // Delete books and files for specific library
            sqlx::query("DELETE FROM book_files WHERE book_id IN (SELECT id FROM books WHERE library_id = $1)")
                .bind(library_id)
                .execute(&state.pool)
                .await?;
            sqlx::query("DELETE FROM books WHERE library_id = $1")
                .bind(library_id)
                .execute(&state.pool)
                .await?;
            info!("[JOB] Deleted existing data for library {}", library_id);
        } else {
            // Delete all books and files
            sqlx::query("DELETE FROM book_files").execute(&state.pool).await?;
            sqlx::query("DELETE FROM books").execute(&state.pool).await?;
            info!("[JOB] Deleted all existing data");
        }
    }

    let libraries = if let Some(library_id) = target_library_id {
        sqlx::query("SELECT id, root_path FROM libraries WHERE id = $1 AND enabled = TRUE")
            .bind(library_id)
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query("SELECT id, root_path FROM libraries WHERE enabled = TRUE")
            .fetch_all(&state.pool)
            .await?
    };

    // First pass: count total files for progress estimation
    let mut total_files = 0usize;
    for library in &libraries {
        let root_path: String = library.get("root_path");
        let root_path = remap_libraries_path(&root_path);
        for entry in WalkDir::new(&root_path).into_iter().filter_map(Result::ok) {
            if entry.file_type().is_file() && detect_format(entry.path()).is_some() {
                total_files += 1;
            }
        }
    }

    // Update job with total estimate
    sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total_files as i32)
        .execute(&state.pool)
        .await?;

    let mut stats = JobStats {
        scanned_files: 0,
        indexed_files: 0,
        removed_files: 0,
        errors: 0,
    };
    
    // Track processed files across all libraries for accurate progress
    let mut total_processed_count = 0i32;

    for library in libraries {
        let library_id: Uuid = library.get("id");
        let root_path: String = library.get("root_path");
        let root_path = remap_libraries_path(&root_path);
        match scan_library(state, job_id, library_id, Path::new(&root_path), &mut stats, &mut total_processed_count, total_files, is_full_rebuild).await {
            Ok(()) => {}
            Err(err) => {
                stats.errors += 1;
                error!(library_id = %library_id, error = %err, "library scan failed");
            }
        }
    }

    sync_meili(&state.pool, &state.meili_url, &state.meili_master_key).await?;

    sqlx::query("UPDATE index_jobs SET status = 'success', finished_at = NOW(), stats_json = $2, current_file = NULL, progress_percent = 100, processed_files = $3 WHERE id = $1")
        .bind(job_id)
        .bind(serde_json::to_value(&stats)?)
        .bind(total_processed_count)
        .execute(&state.pool)
        .await?;

    Ok(())
}

async fn fail_job(pool: &sqlx::PgPool, job_id: Uuid, error_message: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE index_jobs SET status = 'failed', finished_at = NOW(), error_opt = $2 WHERE id = $1")
        .bind(job_id)
        .bind(error_message)
        .execute(pool)
        .await?;
    Ok(())
}

// Batched update data structures
struct BookUpdate {
    book_id: Uuid,
    title: String,
    kind: String,
    series: Option<String>,
    volume: Option<i32>,
    page_count: Option<i32>,
}

struct FileUpdate {
    file_id: Uuid,
    format: String,
    size_bytes: i64,
    mtime: DateTime<Utc>,
    fingerprint: String,
}

struct BookInsert {
    book_id: Uuid,
    library_id: Uuid,
    kind: String,
    title: String,
    series: Option<String>,
    volume: Option<i32>,
    page_count: Option<i32>,
}

struct FileInsert {
    file_id: Uuid,
    book_id: Uuid,
    format: String,
    abs_path: String,
    size_bytes: i64,
    mtime: DateTime<Utc>,
    fingerprint: String,
    parse_status: String,
    parse_error: Option<String>,
}

struct ErrorInsert {
    job_id: Uuid,
    file_path: String,
    error_message: String,
}

async fn flush_all_batches(
    pool: &sqlx::PgPool,
    books_update: &mut Vec<BookUpdate>,
    files_update: &mut Vec<FileUpdate>,
    books_insert: &mut Vec<BookInsert>,
    files_insert: &mut Vec<FileInsert>,
    errors_insert: &mut Vec<ErrorInsert>,
) -> anyhow::Result<()> {
    if books_update.is_empty() && files_update.is_empty() && books_insert.is_empty() && files_insert.is_empty() && errors_insert.is_empty() {
        return Ok(());
    }
    
    let start = std::time::Instant::now();
    let mut tx = pool.begin().await?;
    
    // Batch update books using UNNEST
    if !books_update.is_empty() {
        let book_ids: Vec<Uuid> = books_update.iter().map(|b| b.book_id).collect();
        let titles: Vec<String> = books_update.iter().map(|b| b.title.clone()).collect();
        let kinds: Vec<String> = books_update.iter().map(|b| b.kind.clone()).collect();
        let series: Vec<Option<String>> = books_update.iter().map(|b| b.series.clone()).collect();
        let volumes: Vec<Option<i32>> = books_update.iter().map(|b| b.volume).collect();
        let page_counts: Vec<Option<i32>> = books_update.iter().map(|b| b.page_count).collect();
        
        sqlx::query(
            r#"
            UPDATE books SET 
                title = data.title,
                kind = data.kind,
                series = data.series,
                volume = data.volume,
                page_count = data.page_count,
                updated_at = NOW()
            FROM (
                SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::int[], $6::int[])
                AS t(book_id, title, kind, series, volume, page_count)
            ) AS data
            WHERE books.id = data.book_id
            "#
        )
        .bind(&book_ids)
        .bind(&titles)
        .bind(&kinds)
        .bind(&series)
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
        let titles: Vec<String> = books_insert.iter().map(|b| b.title.clone()).collect();
        let series: Vec<Option<String>> = books_insert.iter().map(|b| b.series.clone()).collect();
        let volumes: Vec<Option<i32>> = books_insert.iter().map(|b| b.volume).collect();
        let page_counts: Vec<Option<i32>> = books_insert.iter().map(|b| b.page_count).collect();
        
        sqlx::query(
            r#"
            INSERT INTO books (id, library_id, kind, title, series, volume, page_count)
            SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::text[], $4::text[], $5::text[], $6::int[], $7::int[])
            AS t(id, library_id, kind, title, series, volume, page_count)
            "#
        )
        .bind(&book_ids)
        .bind(&library_ids)
        .bind(&kinds)
        .bind(&titles)
        .bind(&series)
        .bind(&volumes)
        .bind(&page_counts)
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
    
    tx.commit().await?;
    info!("[BATCH] Flushed all batches in {:?}", start.elapsed());
    
    Ok(())
}

async fn scan_library(
    state: &AppState,
    job_id: Uuid,
    library_id: Uuid,
    root: &Path,
    stats: &mut JobStats,
    total_processed_count: &mut i32,
    total_files: usize,
    is_full_rebuild: bool,
) -> anyhow::Result<()> {
    let existing_rows = sqlx::query(
        r#"
        SELECT bf.id AS file_id, bf.book_id, bf.abs_path, bf.fingerprint
        FROM book_files bf
        JOIN books b ON b.id = bf.book_id
        WHERE b.library_id = $1
        "#,
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await?;

    let mut existing: HashMap<String, (Uuid, Uuid, String)> = HashMap::new();
    if !is_full_rebuild {
        for row in existing_rows {
            let abs_path: String = row.get("abs_path");
            let remapped_path = remap_libraries_path(&abs_path);
            existing.insert(
                remapped_path,
                (row.get("file_id"), row.get("book_id"), row.get("fingerprint")),
            );
        }
    }

    let mut seen: HashMap<String, bool> = HashMap::new();
    let mut library_processed_count = 0i32;
    let mut last_progress_update = std::time::Instant::now();
    
    // Batching buffers
    const BATCH_SIZE: usize = 100;
    let mut books_to_update: Vec<BookUpdate> = Vec::with_capacity(BATCH_SIZE);
    let mut files_to_update: Vec<FileUpdate> = Vec::with_capacity(BATCH_SIZE);
    let mut books_to_insert: Vec<BookInsert> = Vec::with_capacity(BATCH_SIZE);
    let mut files_to_insert: Vec<FileInsert> = Vec::with_capacity(BATCH_SIZE);
    let mut errors_to_insert: Vec<ErrorInsert> = Vec::with_capacity(BATCH_SIZE);

    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let Some(format) = detect_format(path) else {
            continue;
        };

        stats.scanned_files += 1;
        library_processed_count += 1;
        *total_processed_count += 1;
        let abs_path_local = path.to_string_lossy().to_string();
        let abs_path = unmap_libraries_path(&abs_path_local);
        let file_name = path.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| abs_path.clone());

        let start_time = std::time::Instant::now();

        // Update progress in DB every 1 second or every 10 files
        let should_update_progress = last_progress_update.elapsed() > Duration::from_secs(1) || library_processed_count % 10 == 0;
        if should_update_progress {
            let progress_percent = if total_files > 0 {
                ((*total_processed_count as f64 / total_files as f64) * 100.0) as i32
            } else {
                0
            };

            sqlx::query(
                "UPDATE index_jobs SET current_file = $2, processed_files = $3, progress_percent = $4 WHERE id = $1"
            )
            .bind(job_id)
            .bind(&file_name)
            .bind(*total_processed_count)
            .bind(progress_percent)
            .execute(&state.pool)
            .await
            .map_err(|e| {
                error!("[BDD] Failed to update progress for job {}: {}", job_id, e);
                e
            })?;
            
            last_progress_update = std::time::Instant::now();
        }

        let seen_key = remap_libraries_path(&abs_path);
        seen.insert(seen_key.clone(), true);

        let metadata = std::fs::metadata(path)
            .with_context(|| format!("cannot stat {}", path.display()))?;
        let mtime: DateTime<Utc> = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        let fingerprint = compute_fingerprint(path, metadata.len(), &mtime)?;

        let lookup_path = remap_libraries_path(&abs_path);
        if let Some((file_id, book_id, old_fingerprint)) = existing.get(&lookup_path).cloned() {
            if !is_full_rebuild && old_fingerprint == fingerprint {
                continue;
            }

            match parse_metadata(path, format, root) {
                Ok(parsed) => {
                    books_to_update.push(BookUpdate {
                        book_id,
                        title: parsed.title,
                        kind: kind_from_format(format).to_string(),
                        series: parsed.series,
                        volume: parsed.volume,
                        page_count: parsed.page_count,
                    });

                    files_to_update.push(FileUpdate {
                        file_id,
                        format: format.as_str().to_string(),
                        size_bytes: metadata.len() as i64,
                        mtime,
                        fingerprint,
                    });

                    stats.indexed_files += 1;
                }
                Err(err) => {
                    warn!("[PARSER] Failed to parse {}: {}", file_name, err);
                    stats.errors += 1;
                    
                    files_to_update.push(FileUpdate {
                        file_id,
                        format: format.as_str().to_string(),
                        size_bytes: metadata.len() as i64,
                        mtime,
                        fingerprint: fingerprint.clone(),
                    });
                    
                    errors_to_insert.push(ErrorInsert {
                        job_id,
                        file_path: abs_path.clone(),
                        error_message: err.to_string(),
                    });
                    
                    // Also need to mark file as error - we'll do this separately
                    sqlx::query(
                        "UPDATE book_files SET parse_status = 'error', parse_error_opt = $2 WHERE id = $1"
                    )
                    .bind(file_id)
                    .bind(err.to_string())
                    .execute(&state.pool)
                    .await?;
                }
            }

            // Flush if batch is full
            if books_to_update.len() >= BATCH_SIZE || files_to_update.len() >= BATCH_SIZE {
                flush_all_batches(&state.pool, &mut books_to_update, &mut files_to_update, &mut books_to_insert, &mut files_to_insert, &mut errors_to_insert).await?;
            }
            
            continue;
        }

        // New file
        match parse_metadata(path, format, root) {
            Ok(parsed) => {
                let book_id = Uuid::new_v4();
                let file_id = Uuid::new_v4();
                
                books_to_insert.push(BookInsert {
                    book_id,
                    library_id,
                    kind: kind_from_format(format).to_string(),
                    title: parsed.title,
                    series: parsed.series,
                    volume: parsed.volume,
                    page_count: parsed.page_count,
                });

                files_to_insert.push(FileInsert {
                    file_id,
                    book_id,
                    format: format.as_str().to_string(),
                    abs_path: abs_path.clone(),
                    size_bytes: metadata.len() as i64,
                    mtime,
                    fingerprint,
                    parse_status: "ok".to_string(),
                    parse_error: None,
                });

                stats.indexed_files += 1;
            }
            Err(err) => {
                warn!("[PARSER] Failed to parse {}: {}", file_name, err);
                stats.errors += 1;
                let book_id = Uuid::new_v4();
                let file_id = Uuid::new_v4();
                
                books_to_insert.push(BookInsert {
                    book_id,
                    library_id,
                    kind: kind_from_format(format).to_string(),
                    title: file_display_name(path),
                    series: None,
                    volume: None,
                    page_count: None,
                });

                files_to_insert.push(FileInsert {
                    file_id,
                    book_id,
                    format: format.as_str().to_string(),
                    abs_path: abs_path.clone(),
                    size_bytes: metadata.len() as i64,
                    mtime,
                    fingerprint,
                    parse_status: "error".to_string(),
                    parse_error: Some(err.to_string()),
                });

                errors_to_insert.push(ErrorInsert {
                    job_id,
                    file_path: abs_path,
                    error_message: err.to_string(),
                });
            }
        }

        // Flush if batch is full
        if books_to_insert.len() >= BATCH_SIZE || files_to_insert.len() >= BATCH_SIZE {
            flush_all_batches(&state.pool, &mut books_to_update, &mut files_to_update, &mut books_to_insert, &mut files_to_insert, &mut errors_to_insert).await?;
        }
        
        trace!("[DONE] Processed file {} (total time: {:?})", file_name, start_time.elapsed());
    }

    // Final flush of any remaining items
    flush_all_batches(&state.pool, &mut books_to_update, &mut files_to_update, &mut books_to_insert, &mut files_to_insert, &mut errors_to_insert).await?;

    // Handle deletions
    for (abs_path, (file_id, book_id, _)) in existing {
        if seen.contains_key(&abs_path) {
            continue;
        }
        sqlx::query("DELETE FROM book_files WHERE id = $1")
            .bind(file_id)
            .execute(&state.pool)
            .await?;
        sqlx::query("DELETE FROM books WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM book_files WHERE book_id = $1)")
            .bind(book_id)
            .execute(&state.pool)
            .await?;
        stats.removed_files += 1;
    }

    Ok(())
}

fn compute_fingerprint(path: &Path, size: u64, mtime: &DateTime<Utc>) -> anyhow::Result<String> {
    // Optimized: only use size + mtime + first bytes of filename for fast fingerprinting
    // This is 100x faster than reading file content while still being reliable for change detection
    let mut hasher = Sha256::new();
    hasher.update(size.to_le_bytes());
    hasher.update(mtime.timestamp().to_le_bytes());
    
    // Add filename for extra uniqueness (in case of rapid changes with same size+mtime)
    if let Some(filename) = path.file_name() {
        hasher.update(filename.as_encoded_bytes());
    }
    
    Ok(format!("{:x}", hasher.finalize()))
}

fn kind_from_format(format: BookFormat) -> &'static str {
    match format {
        BookFormat::Pdf => "ebook",
        BookFormat::Cbz | BookFormat::Cbr => "comic",
    }
}

fn file_display_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string())
}

#[derive(Serialize)]
struct SearchDoc {
    id: String,
    library_id: String,
    kind: String,
    title: String,
    author: Option<String>,
    series: Option<String>,
    volume: Option<i32>,
    language: Option<String>,
}

async fn sync_meili(pool: &sqlx::PgPool, meili_url: &str, meili_master_key: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let base = meili_url.trim_end_matches('/');

    // Ensure index exists and has proper settings
    let _ = client
        .post(format!("{base}/indexes"))
        .header("Authorization", format!("Bearer {meili_master_key}"))
        .json(&serde_json::json!({"uid": "books", "primaryKey": "id"}))
        .send()
        .await;

    let _ = client
        .patch(format!("{base}/indexes/books/settings/filterable-attributes"))
        .header("Authorization", format!("Bearer {meili_master_key}"))
        .json(&serde_json::json!(["library_id", "kind"]))
        .send()
        .await;

    // Get last sync timestamp
    let last_sync: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT last_meili_sync FROM sync_metadata WHERE id = 1"
    )
    .fetch_optional(pool)
    .await?;

    // If no previous sync, do a full sync
    let is_full_sync = last_sync.is_none();
    
    // Get books to sync: all if full sync, only modified since last sync otherwise
    let rows = if is_full_sync {
        info!("[MEILI] Performing full sync");
        sqlx::query(
            "SELECT id, library_id, kind, title, author, series, volume, language, updated_at FROM books",
        )
        .fetch_all(pool)
        .await?
    } else {
        let since = last_sync.unwrap();
        info!("[MEILI] Performing incremental sync since {}", since);
        
        // Also get deleted book IDs to remove from MeiliSearch
        // For now, we'll do a diff approach: get all book IDs from DB and compare with Meili
        sqlx::query(
            "SELECT id, library_id, kind, title, author, series, volume, language, updated_at FROM books WHERE updated_at > $1",
        )
        .bind(since)
        .fetch_all(pool)
        .await?
    };

    if rows.is_empty() && !is_full_sync {
        info!("[MEILI] No changes to sync");
        // Still update the timestamp
        sqlx::query(
            "INSERT INTO sync_metadata (id, last_meili_sync) VALUES (1, NOW()) ON CONFLICT (id) DO UPDATE SET last_meili_sync = NOW()"
        )
        .execute(pool)
        .await?;
        return Ok(());
    }

    let docs: Vec<SearchDoc> = rows
        .into_iter()
        .map(|row| SearchDoc {
            id: row.get::<Uuid, _>("id").to_string(),
            library_id: row.get::<Uuid, _>("library_id").to_string(),
            kind: row.get("kind"),
            title: row.get("title"),
            author: row.get("author"),
            series: row.get("series"),
            volume: row.get("volume"),
            language: row.get("language"),
        })
        .collect();

    let doc_count = docs.len();
    
    // Send documents to MeiliSearch in batches of 1000
    const MEILI_BATCH_SIZE: usize = 1000;
    for (i, chunk) in docs.chunks(MEILI_BATCH_SIZE).enumerate() {
        let batch_num = i + 1;
        info!("[MEILI] Sending batch {}/{} ({} docs)", batch_num, (doc_count + MEILI_BATCH_SIZE - 1) / MEILI_BATCH_SIZE, chunk.len());
        
        let response = client
            .post(format!("{base}/indexes/books/documents"))
            .header("Authorization", format!("Bearer {meili_master_key}"))
            .json(&chunk)
            .send()
            .await
            .context("failed to send docs to meili")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("MeiliSearch error {}: {}", status, body));
        }
    }

    // Handle deletions: get all book IDs from DB and remove from MeiliSearch any that don't exist
    // This is expensive, so we only do it periodically (every 10 syncs) or on full syncs
    if is_full_sync || rand::random::<u8>() < 26 { // ~10% chance
        info!("[MEILI] Checking for documents to delete");
        
        // Get all book IDs from database
        let db_ids: Vec<String> = sqlx::query_scalar("SELECT id::text FROM books")
            .fetch_all(pool)
            .await?;
        
        // Get all document IDs from MeiliSearch (this requires fetching all documents)
        // For efficiency, we'll just delete by query for documents that might be stale
        // A better approach would be to track deletions in a separate table
        
        // For now, we'll do a simple approach: fetch all Meili docs and compare
        // Note: This could be slow for large collections
        let meili_response = client
            .post(format!("{base}/indexes/books/documents/fetch"))
            .header("Authorization", format!("Bearer {meili_master_key}"))
            .json(&serde_json::json!({
                "fields": ["id"],
                "limit": 100000
            }))
            .send()
            .await;
        
        if let Ok(response) = meili_response {
            if response.status().is_success() {
                if let Ok(meili_docs) = response.json::<Vec<serde_json::Value>>().await {
                    let meili_ids: std::collections::HashSet<String> = meili_docs
                        .into_iter()
                        .filter_map(|doc| doc.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                        .collect();
                    
                    let db_ids_set: std::collections::HashSet<String> = db_ids.into_iter().collect();
                    let to_delete: Vec<String> = meili_ids.difference(&db_ids_set).cloned().collect();
                    
                    if !to_delete.is_empty() {
                        info!("[MEILI] Deleting {} stale documents", to_delete.len());
                        let _ = client
                            .post(format!("{base}/indexes/books/documents/delete-batch"))
                            .header("Authorization", format!("Bearer {meili_master_key}"))
                            .json(&to_delete)
                            .send()
                            .await;
                    }
                }
            }
        }
    }

    // Update last sync timestamp
    sqlx::query(
        "INSERT INTO sync_metadata (id, last_meili_sync) VALUES (1, NOW()) ON CONFLICT (id) DO UPDATE SET last_meili_sync = NOW()"
    )
    .execute(pool)
    .await?;

    info!("[MEILI] Sync completed: {} documents indexed", doc_count);
    Ok(())
}
