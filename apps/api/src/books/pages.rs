use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use image::ImageFormat;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tracing::instrument;
use utoipa::ToSchema;
use uuid::Uuid;

use stripstream_thumbnails::{EncodedThumbnail, ThumbnailStore};

use super::files::current_book_file;
use super::renderer::{
    ArchivePages, Origin, OutputFormat, PageRenderer, PageSpec, RenderOptions, Urgency,
};
use crate::{error::ApiError, state::AppState};

/// Pages rendered in the background after a freshly rendered page.
const PREFETCH_AHEAD: u32 = 2;

fn parse_filter(s: &str) -> image::imageops::FilterType {
    match s {
        "lanczos3" => image::imageops::FilterType::Lanczos3,
        "nearest" => image::imageops::FilterType::Nearest,
        _ => image::imageops::FilterType::Triangle, // Triangle (bilinear) is fast and good enough for comics
    }
}

#[derive(Deserialize, ToSchema, Debug)]
pub struct PageQuery {
    #[schema(value_type = Option<String>, example = "webp")]
    pub format: Option<String>,
    #[schema(value_type = Option<u8>, example = 80)]
    pub quality: Option<u8>,
    #[schema(value_type = Option<u32>, example = 1200)]
    pub width: Option<u32>,
}

/// Detect content type from raw image bytes.
fn detect_content_type(data: &[u8]) -> &'static str {
    match image::guess_format(data) {
        Ok(ImageFormat::Jpeg) => "image/jpeg",
        Ok(ImageFormat::Png) => "image/png",
        Ok(ImageFormat::WebP) => "image/webp",
        Ok(ImageFormat::Avif) => "image/avif",
        _ => "application/octet-stream",
    }
}

/// Get a specific page image from a book with optional format conversion
#[utoipa::path(
    get,
    path = "/books/{book_id}/pages/{n}",
    tag = "books",
    params(
        ("book_id" = String, Path, description = "Book UUID"),
        ("n" = u32, Path, description = "Page number (starts at 1)"),
        ("format" = Option<String>, Query, description = "Output format: webp, jpeg, png"),
        ("quality" = Option<u8>, Query, description = "JPEG quality 1-100"),
        ("width" = Option<u32>, Query, description = "Max width (max 2160)"),
    ),
    responses(
        (status = 200, description = "Page image", content_type = "image/webp"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Book or page not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
#[instrument(skip(state, headers), fields(book_id = %book_id, page = n))]
pub async fn get_page(
    State(state): State<AppState>,
    AxumPath((book_id, n)): AxumPath<(Uuid, u32)>,
    Query(query): Query<PageQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let renderer = PageRenderer::for_state(&state, Arc::new(ArchivePages)).await;
    serve_page(&state, &renderer, book_id, n, &query, &headers).await
}

/// `get_page` over an explicit renderer, so tests can inject a fake page source.
async fn serve_page(
    state: &AppState,
    renderer: &PageRenderer,
    book_id: Uuid,
    n: u32,
    query: &PageQuery,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    if n == 0 {
        return Err(ApiError::bad_request("page index starts at 1"));
    }

    let (default_quality, max_width, filter) = {
        let s = state.settings.read().await;
        (
            s.image_quality,
            s.image_max_width,
            parse_filter(&s.image_filter),
        )
    };
    let output = OutputFormat::parse(query.format.as_deref())?;
    let quality = query.quality.unwrap_or(default_quality).clamp(1, 100);
    let width = query.width.unwrap_or(0);
    if width > max_width {
        return Err(ApiError::bad_request(format!(
            "width must be <= {}",
            max_width
        )));
    }

    let file = current_book_file(&state.pool, &state.library_root, book_id).await?;
    if !file.local_path.exists() {
        return Err(ApiError::not_found("source file not accessible"));
    }
    let options = RenderOptions {
        output,
        quality,
        width,
        filter,
    };
    let spec = PageSpec::new(file.local_path, file.format, n, options);

    // The cache key is the ETag: a client holding this version needs no bytes at all.
    let etag = format!("\"{}\"", spec.key());
    if is_not_modified(headers, &etag) {
        return Ok(not_modified(&etag));
    }

    let rendered = renderer
        .get(&spec, Urgency::Foreground)
        .await?
        .ok_or_else(|| ApiError::internal("page rendering skipped"))?;
    if rendered.origin == Origin::Rendered {
        for next_page in (1..=PREFETCH_AHEAD).filter_map(|ahead| n.checked_add(ahead)) {
            renderer.prefetch(spec.at_page(next_page));
        }
    }

    Ok(image_response(
        rendered.bytes,
        output,
        Some(&rendered.key),
        headers,
    ))
}

fn is_not_modified(req_headers: &HeaderMap, etag: &str) -> bool {
    req_headers
        .get(header::IF_NONE_MATCH)
        .is_some_and(|value| value.as_bytes() == etag.as_bytes())
}

fn not_modified(etag: &str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    if let Ok(v) = HeaderValue::from_str(etag) {
        headers.insert(header::ETAG, v);
    }
    (StatusCode::NOT_MODIFIED, headers).into_response()
}

fn image_response(
    bytes: Arc<[u8]>,
    format: OutputFormat,
    etag_suffix: Option<&str>,
    req_headers: &HeaderMap,
) -> Response {
    let content_type = match format {
        OutputFormat::Original => detect_content_type(&bytes),
        _ => format.content_type(),
    };
    let etag = if let Some(suffix) = etag_suffix {
        format!("\"{}\"", suffix)
    } else {
        let mut hasher = Sha256::new();
        hasher.update(&*bytes);
        format!(
            "\"{}\"",
            hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        )
    };

    if is_not_modified(req_headers, &etag) {
        return not_modified(&etag);
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    if let Ok(v) = HeaderValue::from_str(&etag) {
        headers.insert(header::ETAG, v);
    }
    // Keep the cache's Arc as the owner of the response bytes. `unwrap_or_clone` would
    // clone every RAM-cache hit because the LRU still holds a reference.
    let body_bytes = axum::body::Bytes::from_owner(bytes);
    (StatusCode::OK, headers, Body::from(body_bytes)).into_response()
}

/// Extract page 1 of a book and encode it with the thumbnail store settings
/// (the on-the-fly fallback of `GET /books/{id}/thumbnail`).
pub async fn render_book_cover(
    state: &AppState,
    book_id: Uuid,
    store: &ThumbnailStore,
) -> Result<EncodedThumbnail, ApiError> {
    let file = current_book_file(&state.pool, &state.library_root, book_id).await?;
    if !file.local_path.exists() {
        return Err(ApiError::not_found("source file not accessible"));
    }

    let store = store.clone();
    let pdf_scale = store.settings().width.max(store.settings().height);
    PageRenderer::for_state(state, Arc::new(ArchivePages))
        .await
        .run(move |source| {
            let page = source
                .extract(&file.local_path, file.format, 1, pdf_scale)
                .map_err(|e| ApiError::internal(format!("page extraction failed: {e}")))?;
            store
                .encode(&page)
                .map_err(|e| ApiError::internal(format!("thumbnail encode failed: {e}")))
        })
        .await
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::atomic::Ordering, time::Duration};

    use parsers::BookFormat;

    use super::*;
    use crate::books::renderer::{solid_png, FakePages};
    use crate::state::test_state;

    #[test]
    fn detect_content_type_recognises_common_formats() {
        assert_eq!(
            detect_content_type(&solid_png(2, 2, [255, 0, 0, 255])),
            "image/png"
        );
        assert_eq!(
            detect_content_type(&solid_jpeg(2, 2, [0, 255, 0])),
            "image/jpeg"
        );
        // `guess_format` reads the RIFF/WEBP magic even without a decoder.
        let webp = [b'R', b'I', b'F', b'F', 0, 0, 0, 0, b'W', b'E', b'B', b'P'];
        assert_eq!(detect_content_type(&webp), "image/webp");
        assert_eq!(
            detect_content_type(b"not an image at all"),
            "application/octet-stream"
        );
    }

    #[test]
    fn parse_filter_maps_names_and_defaults_to_triangle() {
        assert_eq!(
            parse_filter("lanczos3"),
            image::imageops::FilterType::Lanczos3
        );
        assert_eq!(
            parse_filter("nearest"),
            image::imageops::FilterType::Nearest
        );
        assert_eq!(
            parse_filter("triangle"),
            image::imageops::FilterType::Triangle
        );
        // Unknown values must not fail the request: they fall back to Triangle.
        assert_eq!(
            parse_filter("catmullrom"),
            image::imageops::FilterType::Triangle
        );
    }

    #[tokio::test]
    async fn image_response_returns_304_for_a_matching_etag() {
        let bytes: Arc<[u8]> = Arc::from(&solid_png(2, 2, [0, 0, 0, 255])[..]);

        let first = image_response(bytes.clone(), OutputFormat::Png, None, &HeaderMap::new());
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(
            first.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        let etag = first.headers()[header::ETAG].to_str().unwrap().to_string();

        let mut request_headers = HeaderMap::new();
        request_headers.insert(header::IF_NONE_MATCH, HeaderValue::from_str(&etag).unwrap());
        let second = image_response(bytes, OutputFormat::Png, None, &request_headers);

        assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    }

    fn solid_jpeg(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        let image = image::RgbImage::from_pixel(width, height, image::Rgb(rgb));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut out, ImageFormat::Jpeg)
            .unwrap();
        out.into_inner()
    }

    /// Write a one-entry CBZ (stored, no compression) — the image bytes are used
    /// verbatim by `parsers::extract_page`.
    fn write_cbz(path: &Path, entry: &str, data: &[u8]) {
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut zip = ::zip::ZipWriter::new(file);
        let opts: ::zip::write::SimpleFileOptions = ::zip::write::SimpleFileOptions::default()
            .compression_method(::zip::CompressionMethod::Stored);
        zip.start_file(entry, opts).unwrap();
        zip.write_all(data).unwrap();
        zip.finish().unwrap();
    }

    fn first_pixel(bytes: &[u8]) -> [u8; 4] {
        image::load_from_memory(bytes)
            .unwrap()
            .to_rgba8()
            .get_pixel(0, 0)
            .0
    }

    fn count_files(directory: &Path) -> usize {
        let mut count = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                count += count_files(&entry.path());
            } else {
                count += 1;
            }
        }
        count
    }

    /// Insert a library, a book and one `book_files` row; returns the book id.
    async fn seed_book_file(pool: &sqlx::PgPool, path: &Path, format: &str) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind("pages-e2e")
            .bind("/libraries/pages-e2e")
            .execute(pool)
            .await
            .unwrap();

        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, authors, locked_fields) VALUES ($1, $2, 'T01', 'comic', '{}'::text[], '{}'::jsonb)",
        )
        .bind(book_id)
        .bind(library_id)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status) VALUES ($1, $2, $3, $4, 1024, NOW(), 'fp', 'ok')",
        )
        .bind(Uuid::new_v4())
        .bind(book_id)
        .bind(format)
        .bind(path.to_string_lossy().to_string())
        .execute(pool)
        .await
        .unwrap();
        book_id
    }

    fn png_request() -> PageQuery {
        PageQuery {
            format: Some("png".to_string()),
            quality: Some(100),
            width: None,
        }
    }

    /// `get_page` through the renderer with a fake page source: the input format comes
    /// from the database (never the extension) for the page and its prefetches, a
    /// matching `If-None-Match` is answered without rendering, and a second read is a
    /// memory hit.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn serve_page_renders_with_the_database_format_and_prefetches(pool: sqlx::PgPool) {
        let cache_dir = tempfile::tempdir().unwrap();
        let library_dir = tempfile::tempdir().unwrap();
        // The extension lies: the database says CBR.
        let book_path = library_dir.path().join("T01.cbz");
        std::fs::write(&book_path, b"archive").unwrap();
        let book_id = seed_book_file(&pool, &book_path, "cbr").await;

        let state = test_state(pool);
        state.settings.write().await.cache_directory =
            cache_dir.path().to_string_lossy().to_string();
        let source = Arc::new(FakePages::new(solid_png(4, 4, [0, 255, 0, 255])));
        let renderer = PageRenderer::for_state(&state, source.clone()).await;

        let first = serve_page(
            &state,
            &renderer,
            book_id,
            1,
            &png_request(),
            &HeaderMap::new(),
        )
        .await
        .expect("first render");
        assert_eq!(first.status(), StatusCode::OK);
        let etag = first.headers()[header::ETAG].to_str().unwrap().to_string();
        let body = axum::body::to_bytes(first.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(first_pixel(&body), [0, 255, 0, 255]);

        // Pages 2 and 3 are prefetched in the background.
        tokio::time::timeout(Duration::from_secs(5), async {
            while source.calls().len() < 3 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("prefetches ran");
        let calls = source.calls();
        let mut pages: Vec<u32> = calls.iter().map(|(_, _, page)| *page).collect();
        pages.sort_unstable();
        assert_eq!(pages, vec![1, 2, 3]);
        assert!(calls
            .iter()
            .all(|(path, format, _)| path == &book_path && *format == BookFormat::Cbr));

        let mut conditional = HeaderMap::new();
        conditional.insert(header::IF_NONE_MATCH, HeaderValue::from_str(&etag).unwrap());
        let not_modified = serve_page(&state, &renderer, book_id, 1, &png_request(), &conditional)
            .await
            .unwrap();
        assert_eq!(not_modified.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(not_modified.headers()[header::ETAG], etag.as_str());

        let hits_before = state.metrics.page_cache_hits.load(Ordering::Relaxed);
        let second = serve_page(
            &state,
            &renderer,
            book_id,
            2,
            &png_request(),
            &HeaderMap::new(),
        )
        .await
        .unwrap();
        assert_eq!(second.status(), StatusCode::OK);
        assert_eq!(
            state.metrics.page_cache_hits.load(Ordering::Relaxed),
            hits_before + 1,
            "the prefetched page is a memory hit"
        );
        assert_eq!(source.calls().len(), 3, "nothing was rendered twice");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn serve_page_rejects_page_zero_and_missing_files(pool: sqlx::PgPool) {
        let book_id =
            seed_book_file(&pool, Path::new("/libraries/pages-e2e/gone.cbz"), "cbz").await;
        let state = test_state(pool);
        let source = Arc::new(FakePages::new(solid_png(1, 1, [0, 0, 0, 255])));
        let renderer = PageRenderer::for_state(&state, source.clone()).await;

        let zero = serve_page(
            &state,
            &renderer,
            book_id,
            0,
            &png_request(),
            &HeaderMap::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(zero.status, StatusCode::BAD_REQUEST);

        let missing = serve_page(
            &state,
            &renderer,
            book_id,
            1,
            &png_request(),
            &HeaderMap::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(missing.status, StatusCode::NOT_FOUND);

        let no_book = serve_page(
            &state,
            &renderer,
            Uuid::new_v4(),
            1,
            &png_request(),
            &HeaderMap::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(no_book.status, StatusCode::NOT_FOUND);
        assert!(source.calls().is_empty());
    }

    /// End-to-end exercise of the double cache with the real archive reader: the
    /// memory cache serves the second request, and a real in-place file change
    /// (different mtime and content) must invalidate both levels — the rendered page
    /// must reflect the new bytes instead of the stale ones.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_page_serves_memory_cache_then_invalidates_on_file_change(pool: sqlx::PgPool) {
        let cache_dir = tempfile::tempdir().unwrap();
        let library_dir = tempfile::tempdir().unwrap();
        let book_path = library_dir.path().join("T01.cbz");
        write_cbz(&book_path, "001.png", &solid_png(8, 8, [255, 0, 0, 255]));
        let book_id = seed_book_file(&pool, &book_path, "cbz").await;

        let state = test_state(pool.clone());
        state.settings.write().await.cache_directory =
            cache_dir.path().to_string_lossy().to_string();

        let request = || {
            Query(PageQuery {
                format: Some("png".to_string()),
                quality: Some(100),
                width: Some(8),
            })
        };

        // 1) Cold miss: rendered from disk and stored in both caches.
        let first = get_page(
            State(state.clone()),
            AxumPath((book_id, 1)),
            request(),
            HeaderMap::new(),
        )
        .await
        .expect("first render");
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()[header::CONTENT_TYPE], "image/png");
        let etag_before = first.headers()[header::ETAG].to_str().unwrap().to_string();
        let body_before = axum::body::to_bytes(first.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(first_pixel(&body_before), [255, 0, 0, 255]);
        assert_eq!(state.metrics.page_cache_misses.load(Ordering::Relaxed), 1);
        assert_eq!(state.metrics.page_cache_hits.load(Ordering::Relaxed), 0);

        // 2) Warm hit: served from the memory cache, identical bytes and etag.
        let second = get_page(
            State(state.clone()),
            AxumPath((book_id, 1)),
            request(),
            HeaderMap::new(),
        )
        .await
        .expect("cached render");
        assert_eq!(second.status(), StatusCode::OK);
        assert_eq!(
            second.headers()[header::ETAG].to_str().unwrap(),
            etag_before
        );
        let body_second = axum::body::to_bytes(second.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body_second, body_before);
        assert_eq!(state.metrics.page_cache_hits.load(Ordering::Relaxed), 1);

        // 3) Replace the archive in place and bump its mtime.
        write_cbz(&book_path, "001.png", &solid_png(8, 8, [0, 0, 255, 255]));
        let future = std::time::SystemTime::now() + Duration::from_secs(10);
        std::fs::File::options()
            .write(true)
            .open(&book_path)
            .unwrap()
            .set_modified(future)
            .unwrap();

        // 4) The changed mtime yields a new key: both caches miss and the new
        //    content is rendered instead of the stale page.
        let third = get_page(
            State(state.clone()),
            AxumPath((book_id, 1)),
            request(),
            HeaderMap::new(),
        )
        .await
        .expect("render after replacement");
        assert_eq!(third.status(), StatusCode::OK);
        assert_ne!(third.headers()[header::ETAG].to_str().unwrap(), etag_before);
        let body_after = axum::body::to_bytes(third.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(first_pixel(&body_after), [0, 0, 255, 255]);
        assert_eq!(state.metrics.page_cache_misses.load(Ordering::Relaxed), 2);

        // 5) The disk cache now holds one entry per version, sharded on disk.
        assert_eq!(count_files(cache_dir.path()), 2);
    }
}
