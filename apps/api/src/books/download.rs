use std::path::Path;

use axum::{
    body::Body,
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use sqlx::Row;
use tokio_util::io::ReaderStream;
use tracing::instrument;
use uuid::Uuid;

use stripstream_core::paths::remap_libraries_path;

use crate::{error::ApiError, state::AppState};

fn file_content_type(format: &str) -> &'static str {
    match format {
        "cbz" => "application/vnd.comicbook+zip",
        "cbr" => "application/vnd.comicbook-rar",
        "pdf" => "application/pdf",
        "epub" => "application/epub+zip",
        _ => "application/octet-stream",
    }
}

/// Characters percent-encoded in the RFC 5987 `filename*` value: everything that is
/// not an `attr-char` (ALPHA / DIGIT / `!#$&+-.^_\`|~`). Non-ASCII bytes are always encoded.
const RFC5987: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'%')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'{')
    .add(b'}');

/// Build an RFC 6266 value: an ASCII fallback that every browser understands, plus the
/// RFC 5987 `filename*` UTF-8 form so accented names survive the round-trip.
fn content_disposition(filename: &str) -> String {
    let fallback: String = filename
        .chars()
        .map(|c| match c {
            '"' | '\\' => '_',
            c if c.is_ascii_graphic() || c == ' ' => c,
            _ => '_',
        })
        .collect();
    let fallback = if fallback.is_empty() {
        "book"
    } else {
        &fallback
    };
    let encoded = utf8_percent_encode(filename, RFC5987);
    format!("attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}")
}

/// Download the original book file (CBZ, CBR, PDF, EPUB…) straight from disk
#[utoipa::path(
    get,
    path = "/books/{book_id}/file",
    tag = "books",
    params(("book_id" = String, Path, description = "Book UUID")),
    responses(
        (status = 200, description = "Original book file", content_type = "application/octet-stream"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book or file not found"),
    ),
    security(("Bearer" = []))
)]
#[instrument(skip(state), fields(book_id = %book_id))]
pub async fn get_book_file(
    State(state): State<AppState>,
    AxumPath(book_id): AxumPath<Uuid>,
) -> Result<Response, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT abs_path, format
        FROM book_files
        WHERE book_id = $1
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(book_id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("book file not found"))?;
    let abs_path: String = row.get("abs_path");
    let format: String = row.get("format");
    let local_path = remap_libraries_path(&abs_path);

    let file = tokio::fs::File::open(&local_path)
        .await
        .map_err(|_| ApiError::not_found("source file not accessible"))?;
    let size = file
        .metadata()
        .await
        .map_err(|_| ApiError::not_found("source file not accessible"))?
        .len();
    let filename = Path::new(&abs_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("book");

    let content_length = HeaderValue::from_str(&size.to_string())
        .map_err(|e| ApiError::internal(format!("invalid content length: {e}")))?;
    let disposition = HeaderValue::from_str(&content_disposition(filename))
        .map_err(|e| ApiError::internal(format!("invalid content disposition: {e}")))?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(file_content_type(&format)),
    );
    headers.insert(header::CONTENT_LENGTH, content_length);
    headers.insert(header::CONTENT_DISPOSITION, disposition);
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );

    let body = Body::from_stream(ReaderStream::new(file));
    Ok((StatusCode::OK, headers, body).into_response())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{atomic::AtomicU64, Arc};

    use sqlx::PgPool;
    use tokio::sync::{Mutex, RwLock, Semaphore};

    use super::*;
    use crate::state::{
        DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
    };

    fn test_state(pool: PgPool) -> AppState {
        AppState {
            pool,
            bootstrap_token: Arc::from("test-token"),
            page_cache: Arc::new(Mutex::new(crate::state::PageCache::new(1))),
            disk_cache_stats: Arc::new(Mutex::new(None::<DiskCacheStatsSnapshot>)),
            page_render_locks: Arc::new(PageRenderLocks::new(1)),
            page_render_limit: Arc::new(Semaphore::new(1)),
            metrics: Arc::new(Metrics {
                requests_total: AtomicU64::new(0),
                page_cache_hits: AtomicU64::new(0),
                page_cache_misses: AtomicU64::new(0),
            }),
            read_rate_limit: Arc::new(std::sync::Mutex::new(ReadRateLimit::new())),
            stats_cache: Arc::new(crate::stats::StatsCache::new()),
            settings: Arc::new(RwLock::new(DynamicSettings::default())),
            prowlarr_fetch_lock: Arc::new(Mutex::new(())),
            pending_tg_auth: Arc::new(Mutex::new(None)),
            telegram_download_limit: Arc::new(Semaphore::new(1)),
            telegram_abort_handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn seed_book(pool: &PgPool) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind("test-lib")
            .bind("/libraries/test-lib")
            .execute(pool)
            .await
            .unwrap();

        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, authors, locked_fields) VALUES ($1, $2, $3, $4, '{}'::text[], '{}'::jsonb)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind("Test Book")
        .bind("comic")
        .execute(pool)
        .await
        .unwrap();
        book_id
    }

    async fn add_book_file(pool: &PgPool, book_id: Uuid, abs_path: &str, format: &str) {
        sqlx::query(
            "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status) VALUES ($1, $2, $3, $4, 1024, NOW(), 'fp-test', 'ok')",
        )
        .bind(Uuid::new_v4())
        .bind(book_id)
        .bind(format)
        .bind(abs_path)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn download_error(state: AppState, book_id: Uuid) -> ApiError {
        match get_book_file(State(state), AxumPath(book_id)).await {
            Ok(_) => panic!("expected the download to fail"),
            Err(error) => error,
        }
    }

    #[test]
    fn file_content_type_maps_known_formats() {
        assert_eq!(file_content_type("cbz"), "application/vnd.comicbook+zip");
        assert_eq!(file_content_type("cbr"), "application/vnd.comicbook-rar");
        assert_eq!(file_content_type("pdf"), "application/pdf");
        assert_eq!(file_content_type("epub"), "application/epub+zip");
        assert_eq!(file_content_type("mobi"), "application/octet-stream");
    }

    #[test]
    fn content_disposition_encodes_utf8_names() {
        let value = content_disposition("Astérix le Gaulois.cbz");
        assert!(value.starts_with("attachment; "));
        assert!(value.contains("filename=\"Ast_rix le Gaulois.cbz\""));
        assert!(value.contains("filename*=UTF-8''Ast%C3%A9rix%20le%20Gaulois.cbz"));
    }

    #[test]
    fn content_disposition_sanitizes_quotes_and_backslashes() {
        let value = content_disposition("we\"ird\\name.cbz");
        assert!(value.contains("filename=\"we_ird_name.cbz\""));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_book_file_streams_the_original_file(pool: PgPool) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("Astérix le Gaulois.cbz");
        std::fs::write(&path, b"cbz-bytes").expect("write temp file");
        let book_id = seed_book(&pool).await;
        add_book_file(&pool, book_id, path.to_str().expect("utf-8 path"), "cbz").await;

        let response = get_book_file(State(test_state(pool)), AxumPath(book_id))
            .await
            .expect("download should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .expect("content type"),
            "application/vnd.comicbook+zip"
        );
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_LENGTH)
                .expect("content length"),
            "9"
        );
        let disposition = response
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .expect("content disposition")
            .to_str()
            .expect("ascii header");
        assert!(disposition.starts_with("attachment; "));
        assert!(disposition.contains("filename*=UTF-8''Ast%C3%A9rix%20le%20Gaulois.cbz"));

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        assert_eq!(&body[..], b"cbz-bytes");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_book_file_returns_404_without_a_file_row(pool: PgPool) {
        let book_id = seed_book(&pool).await;

        let error = download_error(test_state(pool), book_id).await;

        assert_eq!(error.status, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_book_file_returns_404_when_the_disk_file_is_missing(pool: PgPool) {
        let book_id = seed_book(&pool).await;
        add_book_file(&pool, book_id, "/libraries/test-lib/missing.cbz", "cbz").await;

        let error = download_error(test_state(pool), book_id).await;

        assert_eq!(error.status, StatusCode::NOT_FOUND);
    }
}
