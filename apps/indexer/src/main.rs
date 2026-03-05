use anyhow::Context;
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use axum::http::StatusCode;
use parsers::{detect_format, parse_metadata, BookFormat};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, Row};
use std::{collections::HashMap, path::Path, time::Duration};
use stripstream_core::config::IndexerConfig;
use tracing::{error, info};
use uuid::Uuid;
use walkdir::WalkDir;

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
        .max_connections(5)
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
    loop {
        match claim_next_job(&state.pool).await {
            Ok(Some((job_id, library_id))) => {
                if let Err(err) = process_job(&state, job_id, library_id).await {
                    error!(job_id = %job_id, error = %err, "index job failed");
                    let _ = fail_job(&state.pool, job_id, &err.to_string()).await;
                }
            }
            Ok(None) => tokio::time::sleep(wait).await,
            Err(err) => {
                error!(error = %err, "worker loop error");
                tokio::time::sleep(wait).await;
            }
        }
    }
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

    let mut stats = JobStats {
        scanned_files: 0,
        indexed_files: 0,
        removed_files: 0,
        errors: 0,
    };

    for library in libraries {
        let library_id: Uuid = library.get("id");
        let root_path: String = library.get("root_path");
        match scan_library(state, library_id, Path::new(&root_path), &mut stats).await {
            Ok(()) => {}
            Err(err) => {
                stats.errors += 1;
                error!(library_id = %library_id, error = %err, "library scan failed");
            }
        }
    }

    sync_meili(&state.pool, &state.meili_url, &state.meili_master_key).await?;

    sqlx::query("UPDATE index_jobs SET status = 'success', finished_at = NOW(), stats_json = $2 WHERE id = $1")
        .bind(job_id)
        .bind(serde_json::to_value(&stats)?)
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

async fn scan_library(
    state: &AppState,
    library_id: Uuid,
    root: &Path,
    stats: &mut JobStats,
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
    for row in existing_rows {
        existing.insert(
            row.get("abs_path"),
            (row.get("file_id"), row.get("book_id"), row.get("fingerprint")),
        );
    }

    let mut seen: HashMap<String, bool> = HashMap::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let Some(format) = detect_format(path) else {
            continue;
        };

        stats.scanned_files += 1;
        let abs_path = path.to_string_lossy().to_string();
        seen.insert(abs_path.clone(), true);

        let metadata = std::fs::metadata(path)
            .with_context(|| format!("cannot stat {}", path.display()))?;
        let mtime: DateTime<Utc> = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        let fingerprint = compute_fingerprint(path, metadata.len(), &mtime)?;

        if let Some((file_id, book_id, old_fingerprint)) = existing.get(&abs_path).cloned() {
            if old_fingerprint == fingerprint {
                continue;
            }

            match parse_metadata(path, format, root) {
                Ok(parsed) => {
                    sqlx::query(
                        "UPDATE books SET title = $2, kind = $3, series = $4, volume = $5, page_count = $6, updated_at = NOW() WHERE id = $1",
                    )
                    .bind(book_id)
                    .bind(&parsed.title)
                    .bind(kind_from_format(format))
                    .bind(&parsed.series)
                    .bind(&parsed.volume)
                    .bind(parsed.page_count)
                    .execute(&state.pool)
                    .await?;

                    sqlx::query(
                        "UPDATE book_files SET format = $2, size_bytes = $3, mtime = $4, fingerprint = $5, parse_status = 'ok', parse_error_opt = NULL, updated_at = NOW() WHERE id = $1",
                    )
                    .bind(file_id)
                    .bind(format.as_str())
                    .bind(metadata.len() as i64)
                    .bind(mtime)
                    .bind(fingerprint)
                    .execute(&state.pool)
                    .await?;

                    stats.indexed_files += 1;
                }
                Err(err) => {
                    stats.errors += 1;
                    sqlx::query(
                        "UPDATE book_files SET parse_status = 'error', parse_error_opt = $2, updated_at = NOW() WHERE id = $1",
                    )
                    .bind(file_id)
                    .bind(err.to_string())
                    .execute(&state.pool)
                    .await?;
                }
            }

            continue;
        }

        match parse_metadata(path, format, root) {
            Ok(parsed) => {
                let book_id = Uuid::new_v4();
                let file_id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO books (id, library_id, kind, title, series, volume, page_count) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(book_id)
                .bind(library_id)
                .bind(kind_from_format(format))
                .bind(&parsed.title)
                .bind(&parsed.series)
                .bind(&parsed.volume)
                .bind(parsed.page_count)
                .execute(&state.pool)
                .await?;

                sqlx::query(
                    "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status) VALUES ($1, $2, $3, $4, $5, $6, $7, 'ok')",
                )
                .bind(file_id)
                .bind(book_id)
                .bind(format.as_str())
                .bind(&abs_path)
                .bind(metadata.len() as i64)
                .bind(mtime)
                .bind(fingerprint)
                .execute(&state.pool)
                .await?;

                stats.indexed_files += 1;
            }
            Err(err) => {
                stats.errors += 1;
                let book_id = Uuid::new_v4();
                let file_id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO books (id, library_id, kind, title, page_count) VALUES ($1, $2, $3, $4, NULL)",
                )
                .bind(book_id)
                .bind(library_id)
                .bind(kind_from_format(format))
                .bind(file_display_name(path))
                .execute(&state.pool)
                .await?;

                sqlx::query(
                    "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status, parse_error_opt) VALUES ($1, $2, $3, $4, $5, $6, $7, 'error', $8)",
                )
                .bind(file_id)
                .bind(book_id)
                .bind(format.as_str())
                .bind(&abs_path)
                .bind(metadata.len() as i64)
                .bind(mtime)
                .bind(fingerprint)
                .bind(err.to_string())
                .execute(&state.pool)
                .await?;
            }
        }
    }

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
    let mut hasher = Sha256::new();
    hasher.update(size.to_le_bytes());
    hasher.update(mtime.timestamp().to_le_bytes());

    let bytes = std::fs::read(path)?;
    let take = bytes.len().min(65_536);
    hasher.update(&bytes[..take]);
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
    volume: Option<String>,
    language: Option<String>,
}

async fn sync_meili(pool: &sqlx::PgPool, meili_url: &str, meili_master_key: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let base = meili_url.trim_end_matches('/');

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

    let rows = sqlx::query(
        "SELECT id, library_id, kind, title, author, series, volume, language FROM books",
    )
    .fetch_all(pool)
    .await?;

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

    client
        .put(format!("{base}/indexes/books/documents?primaryKey=id"))
        .header("Authorization", format!("Bearer {meili_master_key}"))
        .json(&docs)
        .send()
        .await
        .context("failed to push docs to meili")?;

    Ok(())
}
