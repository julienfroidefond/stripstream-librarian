use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use axum::{
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use image::{codecs::jpeg::JpegEncoder, codecs::png::PngEncoder, ColorType, ImageEncoder, ImageFormat};
use serde::Deserialize;
use utoipa::ToSchema;
use sha2::{Digest, Sha256};
use sqlx::Row;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

fn remap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with("/libraries/") {
            return path.replacen("/libraries", &root, 1);
        }
    }
    path.to_string()
}

fn parse_filter(s: &str) -> image::imageops::FilterType {
    match s {
        "lanczos3" => image::imageops::FilterType::Lanczos3,
        "nearest" => image::imageops::FilterType::Nearest,
        _ => image::imageops::FilterType::Triangle, // Triangle (bilinear) is fast and good enough for comics
    }
}

fn get_cache_key(abs_path: &str, page: u32, format: &str, quality: u8, width: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(abs_path.as_bytes());
    hasher.update(page.to_le_bytes());
    hasher.update(format.as_bytes());
    hasher.update(quality.to_le_bytes());
    hasher.update(width.to_le_bytes());
    format!("{:x}", hasher.finalize())
}

fn get_cache_path(cache_key: &str, format: &OutputFormat, cache_dir: &Path) -> PathBuf {
    let prefix = &cache_key[..2];
    let ext = format.extension();
    cache_dir.join(prefix).join(format!("{}.{}", cache_key, ext))
}

fn read_from_disk_cache(cache_path: &Path) -> Option<Vec<u8>> {
    std::fs::read(cache_path).ok()
}

fn write_to_disk_cache(cache_path: &Path, data: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(cache_path)?;
    file.write_all(data)?;
    // No sync_data() — this is a cache, durability is not critical
    Ok(())
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

#[derive(Clone, Copy, Debug)]
enum OutputFormat {
    /// Serve raw bytes from the archive — no decode, no re-encode.
    Original,
    Jpeg,
    Png,
    Webp,
}

impl OutputFormat {
    fn parse(value: Option<&str>) -> Result<Self, ApiError> {
        match value {
            None => Ok(Self::Original),
            Some("original") => Ok(Self::Original),
            Some("jpeg") | Some("jpg") => Ok(Self::Jpeg),
            Some("png") => Ok(Self::Png),
            Some("webp") => Ok(Self::Webp),
            _ => Err(ApiError::bad_request("format must be original|webp|jpeg|png")),
        }
    }

    fn content_type(&self) -> &'static str {
        match self {
            Self::Original => "application/octet-stream", // will be overridden by detected type
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
        }
    }

    fn extension(&self) -> &'static str {
        match self {
            Self::Original => "orig",
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }
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
    if n == 0 {
        return Err(ApiError::bad_request("page index starts at 1"));
    }

    let (default_quality, max_width, filter_str, timeout_secs, cache_dir) = {
        let s = state.settings.read().await;
        (s.image_quality, s.image_max_width, s.image_filter.clone(), s.timeout_seconds, s.cache_directory.clone())
    };

    let format = OutputFormat::parse(query.format.as_deref())?;
    let quality = query.quality.unwrap_or(default_quality).clamp(1, 100);
    let width = query.width.unwrap_or(0);
    if width > max_width {
        return Err(ApiError::bad_request(format!("width must be <= {}", max_width)));
    }
    let filter = parse_filter(&filter_str);
    let cache_dir_path = std::path::PathBuf::from(&cache_dir);

    let memory_cache_key = format!("{book_id}:{n}:{}:{quality}:{width}", format.extension());

    if let Some(cached) = state.page_cache.lock().await.get(&memory_cache_key).cloned() {
        state.metrics.page_cache_hits.fetch_add(1, Ordering::Relaxed);
        return Ok(image_response(cached, format, None, &headers));
    }
    state.metrics.page_cache_misses.fetch_add(1, Ordering::Relaxed);

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
    .await
    .map_err(|e| {
        error!("Database error fetching book file for book_id {}: {}", book_id, e);
        e
    })?;

    let row = match row {
        Some(r) => r,
        None => {
            return Err(ApiError::not_found("book file not found"));
        }
    };

    let abs_path: String = row.get("abs_path");
    let abs_path = remap_libraries_path(&abs_path);
    let input_format: String = row.get("format");

    let disk_cache_key = get_cache_key(&abs_path, n, format.extension(), quality, width);
    let cache_path = get_cache_path(&disk_cache_key, &format, &cache_dir_path);

    // If-None-Match: return 304 if the client already has this version
    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
        let expected_etag = format!("\"{}\"", disk_cache_key);
        if if_none_match.as_bytes() == expected_etag.as_bytes() {
            return Ok(StatusCode::NOT_MODIFIED.into_response());
        }
    }

    if let Some(cached_bytes) = read_from_disk_cache(&cache_path) {
        let bytes = Arc::new(cached_bytes);
        state.page_cache.lock().await.put(memory_cache_key, bytes.clone());
        return Ok(image_response(bytes, format, Some(&disk_cache_key), &headers));
    }

    let _permit = state
        .page_render_limit
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| {
            error!("Failed to acquire render permit: {}", e);
            ApiError::internal("render limiter unavailable")
        })?;

    let abs_path_clone = abs_path.clone();
    let format_clone = format;
    let start_time = std::time::Instant::now();

    let bytes = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tokio::task::spawn_blocking(move || {
            render_page(&abs_path_clone, &input_format, n, &format_clone, quality, width, filter)
        }),
    )
    .await
    .map_err(|_| {
        error!("Page rendering timeout for {} page {}", abs_path, n);
        ApiError::internal("page rendering timeout")
    })?
    .map_err(|e| {
        error!("Render task panicked for {} page {}: {}", abs_path, n, e);
        ApiError::internal(format!("render task failed: {e}"))
    })?;

    let duration = start_time.elapsed();

    match bytes {
        Ok(data) => {
            info!("Rendered page {} in {:?}", n, duration);

            if let Err(e) = write_to_disk_cache(&cache_path, &data) {
                warn!("Failed to write to disk cache: {}", e);
            }

            let bytes = Arc::new(data);
            state.page_cache.lock().await.put(memory_cache_key.clone(), bytes.clone());

            // Prefetch next 2 pages in background (fire-and-forget)
            for next_page in [n + 1, n + 2] {
                let state2 = state.clone();
                let abs_path2 = abs_path.clone();
                let cache_dir2 = cache_dir_path.clone();
                let format2 = format;
                tokio::spawn(async move {
                    prefetch_page(state2, &PrefetchParams {
                        book_id,
                        abs_path: &abs_path2,
                        page: next_page,
                        format: format2,
                        quality,
                        width,
                        filter,
                        timeout_secs,
                        cache_dir: &cache_dir2,
                    }).await;
                });
            }

            Ok(image_response(bytes, format, Some(&disk_cache_key), &headers))
        }
        Err(e) => {
            error!("Failed to render page {} from {}: {:?}", n, abs_path, e);
            Err(e)
        }
    }
}

struct PrefetchParams<'a> {
    book_id: Uuid,
    abs_path: &'a str,
    page: u32,
    format: OutputFormat,
    quality: u8,
    width: u32,
    filter: image::imageops::FilterType,
    timeout_secs: u64,
    cache_dir: &'a Path,
}

/// Prefetch a single page into disk+memory cache (best-effort, ignores errors).
async fn prefetch_page(state: AppState, params: &PrefetchParams<'_>) {
    let book_id = params.book_id;
    let page = params.page;
    let format = params.format;
    let quality = params.quality;
    let width = params.width;
    let filter = params.filter;
    let timeout_secs = params.timeout_secs;
    let abs_path = params.abs_path;
    let cache_dir = params.cache_dir;

    let mem_key = format!("{book_id}:{page}:{}:{quality}:{width}", format.extension());
    // Already in memory cache?
    if state.page_cache.lock().await.contains(&mem_key) {
        return;
    }
    // Already on disk?
    let disk_key = get_cache_key(abs_path, page, format.extension(), quality, width);
    let cache_path = get_cache_path(&disk_key, &format, cache_dir);
    if cache_path.exists() {
        return;
    }
    // Acquire render permit (don't block too long — if busy, skip)
    let permit = tokio::time::timeout(
        Duration::from_millis(100),
        state.page_render_limit.clone().acquire_owned(),
    )
    .await;
    let _permit = match permit {
        Ok(Ok(p)) => p,
        _ => return,
    };

    // Fetch the book format from the path extension as a shortcut
    let input_format = match abs_path.rsplit('.').next().map(|e| e.to_ascii_lowercase()) {
        Some(ref e) if e == "cbz" => "cbz",
        Some(ref e) if e == "cbr" => "cbr",
        Some(ref e) if e == "pdf" => "pdf",
        Some(ref e) if e == "epub" => "epub",
        _ => return,
    }
    .to_string();

    let abs_clone = abs_path.to_string();
    let fmt = format;
    let result = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tokio::task::spawn_blocking(move || {
            render_page(&abs_clone, &input_format, page, &fmt, quality, width, filter)
        }),
    )
    .await;

    if let Ok(Ok(Ok(data))) = result {
        let _ = write_to_disk_cache(&cache_path, &data);
        let bytes = Arc::new(data);
        state.page_cache.lock().await.put(mem_key, bytes);
    }
}

fn image_response(bytes: Arc<Vec<u8>>, format: OutputFormat, etag_suffix: Option<&str>, req_headers: &HeaderMap) -> Response {
    let content_type = match format {
        OutputFormat::Original => detect_content_type(&bytes),
        _ => format.content_type(),
    };
    let etag = if let Some(suffix) = etag_suffix {
        format!("\"{}\"", suffix)
    } else {
        let mut hasher = Sha256::new();
        hasher.update(&*bytes);
        format!("\"{:x}\"", hasher.finalize())
    };

    // Check If-None-Match for 304
    if let Some(if_none_match) = req_headers.get(header::IF_NONE_MATCH) {
        if if_none_match.as_bytes() == etag.as_bytes() {
            let mut headers = HeaderMap::new();
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable"));
            if let Ok(v) = HeaderValue::from_str(&etag) {
                headers.insert(header::ETAG, v);
            }
            return (StatusCode::NOT_MODIFIED, headers).into_response();
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable"));
    if let Ok(v) = HeaderValue::from_str(&etag) {
        headers.insert(header::ETAG, v);
    }
    // Use Bytes to avoid cloning the Vec — shares the Arc's allocation via zero-copy
    let body_bytes = axum::body::Bytes::from(Arc::unwrap_or_clone(bytes));
    (StatusCode::OK, headers, Body::from(body_bytes)).into_response()
}

/// Render page 1 of a book (for thumbnail fallback or thumbnail checkup). Uses thumbnail dimensions by default.
/// Render page 1 as a thumbnail fallback. Returns (bytes, content_type).
pub async fn render_book_page_1(
    state: &AppState,
    book_id: Uuid,
    width: u32,
    quality: u8,
) -> Result<(Vec<u8>, &'static str), ApiError> {
    let row = sqlx::query(
        r#"SELECT abs_path, format FROM book_files WHERE book_id = $1 ORDER BY updated_at DESC LIMIT 1"#,
    )
    .bind(book_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let row = row.ok_or_else(|| ApiError::not_found("book file not found"))?;
    let abs_path: String = row.get("abs_path");
    let abs_path = remap_libraries_path(&abs_path);
    let input_format: String = row.get("format");

    let _permit = state
        .page_render_limit
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiError::internal("render limiter unavailable"))?;

    let (timeout_secs, filter_str) = {
        let s = state.settings.read().await;
        (s.timeout_seconds, s.image_filter.clone())
    };
    let filter = parse_filter(&filter_str);

    let abs_path_clone = abs_path.clone();
    let bytes = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tokio::task::spawn_blocking(move || {
            render_page(
                &abs_path_clone,
                &input_format,
                1,
                &OutputFormat::Original,
                quality,
                width,
                filter,
            )
        }),
    )
    .await
    .map_err(|_| ApiError::internal("page rendering timeout"))?
    .map_err(|e| ApiError::internal(format!("render task failed: {e}")))?;

    let bytes = bytes?;
    let content_type = detect_content_type(&bytes);
    Ok((bytes, content_type))
}

fn render_page(
    abs_path: &str,
    input_format: &str,
    page_number: u32,
    out_format: &OutputFormat,
    quality: u8,
    width: u32,
    filter: image::imageops::FilterType,
) -> Result<Vec<u8>, ApiError> {
    let format = match input_format {
        "cbz" => parsers::BookFormat::Cbz,
        "cbr" => parsers::BookFormat::Cbr,
        "pdf" => parsers::BookFormat::Pdf,
        "epub" => parsers::BookFormat::Epub,
        _ => return Err(ApiError::bad_request("unsupported source format")),
    };

    let pdf_render_width = if width > 0 { width } else { 1200 };
    let page_bytes = parsers::extract_page(
        std::path::Path::new(abs_path),
        format,
        page_number,
        pdf_render_width,
    )
    .map_err(|e| {
        error!("Failed to extract page {} from {}: {}", page_number, abs_path, e);
        ApiError::internal(format!("page extraction failed: {e}"))
    })?;

    // Original mode or source matches output with no resize → return raw bytes (zero transcoding)
    if matches!(out_format, OutputFormat::Original) && width == 0 {
        return Ok(page_bytes);
    }
    if width == 0 {
        if let Ok(source_fmt) = image::guess_format(&page_bytes) {
            if format_matches(&source_fmt, out_format) {
                return Ok(page_bytes);
            }
        }
    }

    transcode_image(&page_bytes, out_format, quality, width, filter)
}


/// Fast JPEG decode with DCT scaling: decodes directly at reduced resolution.
fn fast_jpeg_decode(input: &[u8], target_w: u32, target_h: u32) -> Option<image::DynamicImage> {
    if image::guess_format(input).ok()? != ImageFormat::Jpeg {
        return None;
    }
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(input));
    decoder.read_info().ok()?;
    decoder.scale(target_w as u16, target_h as u16).ok()?;
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let w = info.width as u32;
    let h = info.height as u32;
    match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            let buf = image::RgbImage::from_raw(w, h, pixels)?;
            Some(image::DynamicImage::ImageRgb8(buf))
        }
        jpeg_decoder::PixelFormat::L8 => {
            let buf = image::GrayImage::from_raw(w, h, pixels)?;
            Some(image::DynamicImage::ImageLuma8(buf))
        }
        _ => None,
    }
}

fn transcode_image(input: &[u8], out_format: &OutputFormat, quality: u8, width: u32, filter: image::imageops::FilterType) -> Result<Vec<u8>, ApiError> {
    let source_format = image::guess_format(input).ok();

    // Resolve "Original" to the actual source format for encoding
    let effective_format = match out_format {
        OutputFormat::Original => match source_format {
            Some(ImageFormat::Png) => OutputFormat::Png,
            Some(ImageFormat::WebP) => OutputFormat::Webp,
            _ => OutputFormat::Jpeg, // default to JPEG for original resize
        },
        other => *other,
    };

    let needs_transcode = source_format.map(|f| !format_matches(&f, &effective_format)).unwrap_or(true);

    if width == 0 && !needs_transcode {
        return Ok(input.to_vec());
    }

    // For JPEG with resize: use DCT scaling to decode at ~target size (much faster)
    let mut image = if width > 0 {
        fast_jpeg_decode(input, width, u32::MAX)
            .unwrap_or_else(|| {
                image::load_from_memory(input).unwrap_or_default()
            })
    } else {
        image::load_from_memory(input).map_err(|e| {
            ApiError::internal(format!("invalid source image: {e}"))
        })?
    };

    if width > 0 {
        image = image.resize(width, u32::MAX, filter);
    }

    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();

    let mut out = Vec::new();
    match effective_format {
        OutputFormat::Jpeg | OutputFormat::Original => {
            // JPEG doesn't support alpha — convert RGBA to RGB
            let rgb = image::DynamicImage::ImageRgba8(rgba.clone()).to_rgb8();
            let mut encoder = JpegEncoder::new_with_quality(&mut out, quality);
            encoder
                .encode(&rgb, w, h, ColorType::Rgb8.into())
                .map_err(|e| ApiError::internal(format!("jpeg encode failed: {e}")))?;
        }
        OutputFormat::Png => {
            let encoder = PngEncoder::new(&mut out);
            encoder
                .write_image(&rgba, w, h, ColorType::Rgba8.into())
                .map_err(|e| ApiError::internal(format!("png encode failed: {e}")))?;
        }
        OutputFormat::Webp => {
            let rgb_data: Vec<u8> = rgba
                .pixels()
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            let webp_data = webp::Encoder::new(&rgb_data, webp::PixelLayout::Rgb, w, h)
                .encode(quality as f32);
            out.extend_from_slice(&webp_data);
        }
    }
    Ok(out)
}

fn format_matches(source: &ImageFormat, target: &OutputFormat) -> bool {
    matches!(
        (source, target),
        (ImageFormat::Jpeg, OutputFormat::Jpeg)
            | (ImageFormat::Png, OutputFormat::Png)
            | (ImageFormat::WebP, OutputFormat::Webp)
    )
}

