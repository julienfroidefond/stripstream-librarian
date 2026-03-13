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
use tracing::{debug, error, info, instrument, warn};
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
        "triangle" => image::imageops::FilterType::Triangle,
        "nearest" => image::imageops::FilterType::Nearest,
        _ => image::imageops::FilterType::Lanczos3,
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
    file.sync_data()?;
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
    Jpeg,
    Png,
    Webp,
}

impl OutputFormat {
    fn parse(value: Option<&str>) -> Result<Self, ApiError> {
        match value.unwrap_or("webp") {
            "jpeg" | "jpg" => Ok(Self::Jpeg),
            "png" => Ok(Self::Png),
            "webp" => Ok(Self::Webp),
            _ => Err(ApiError::bad_request("format must be webp|jpeg|png")),
        }
    }

    fn content_type(&self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
        }
    }

    fn extension(&self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
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
#[instrument(skip(state), fields(book_id = %book_id, page = n))]
pub async fn get_page(
    State(state): State<AppState>,
    AxumPath((book_id, n)): AxumPath<(Uuid, u32)>,
    Query(query): Query<PageQuery>,
) -> Result<Response, ApiError> {
    info!("Processing image request");
    
    if n == 0 {
        warn!("Invalid page number: 0");
        return Err(ApiError::bad_request("page index starts at 1"));
    }

    let (default_format, default_quality, max_width, filter_str, timeout_secs, cache_dir) = {
        let s = state.settings.read().await;
        (s.image_format.clone(), s.image_quality, s.image_max_width, s.image_filter.clone(), s.timeout_seconds, s.cache_directory.clone())
    };

    let format_str = query.format.as_deref().unwrap_or(default_format.as_str());
    let format = OutputFormat::parse(Some(format_str))?;
    let quality = query.quality.unwrap_or(default_quality).clamp(1, 100);
    let width = query.width.unwrap_or(0);
    if width > max_width {
        warn!("Invalid width: {}", width);
        return Err(ApiError::bad_request(format!("width must be <= {}", max_width)));
    }
    let filter = parse_filter(&filter_str);
    let cache_dir_path = std::path::PathBuf::from(&cache_dir);

    let memory_cache_key = format!("{book_id}:{n}:{}:{quality}:{width}", format.extension());
    
    if let Some(cached) = state.page_cache.lock().await.get(&memory_cache_key).cloned() {
        state.metrics.page_cache_hits.fetch_add(1, Ordering::Relaxed);
        debug!("Memory cache hit for key: {}", memory_cache_key);
        return Ok(image_response(cached, format.content_type(), None));
    }
    state.metrics.page_cache_misses.fetch_add(1, Ordering::Relaxed);
    debug!("Memory cache miss for key: {}", memory_cache_key);

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
            error!("Book file not found for book_id: {}", book_id);
            return Err(ApiError::not_found("book file not found"));
        }
    };
    
    let abs_path: String = row.get("abs_path");
    let abs_path = remap_libraries_path(&abs_path);
    let input_format: String = row.get("format");
    
    info!("Processing book file: {} (format: {})", abs_path, input_format);

    let disk_cache_key = get_cache_key(&abs_path, n, format.extension(), quality, width);
    let cache_path = get_cache_path(&disk_cache_key, &format, &cache_dir_path);
    
    if let Some(cached_bytes) = read_from_disk_cache(&cache_path) {
        info!("Disk cache hit for: {}", cache_path.display());
        let bytes = Arc::new(cached_bytes);
        state.page_cache.lock().await.put(memory_cache_key, bytes.clone());
        return Ok(image_response(bytes, format.content_type(), Some(&disk_cache_key)));
    }
    debug!("Disk cache miss for: {}", cache_path.display());

    let _permit = state
        .page_render_limit
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| {
            error!("Failed to acquire render permit: {}", e);
            ApiError::internal("render limiter unavailable")
        })?;

    info!("Rendering page {} from {}", n, abs_path);
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
            info!("Successfully rendered page {} in {:?}", n, duration);
            
            if let Err(e) = write_to_disk_cache(&cache_path, &data) {
                warn!("Failed to write to disk cache: {}", e);
            } else {
                info!("Cached rendered image to: {}", cache_path.display());
            }

            let bytes = Arc::new(data);
            state.page_cache.lock().await.put(memory_cache_key, bytes.clone());

            Ok(image_response(bytes, format.content_type(), Some(&disk_cache_key)))
        }
        Err(e) => {
            error!("Failed to render page {} from {}: {:?}", n, abs_path, e);
            Err(e)
        }
    }
}

fn image_response(bytes: Arc<Vec<u8>>, content_type: &str, etag_suffix: Option<&str>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable"));
    
    let etag = if let Some(suffix) = etag_suffix {
        format!("\"{}\"", suffix)
    } else {
        let mut hasher = Sha256::new();
        hasher.update(&*bytes);
        format!("\"{:x}\"", hasher.finalize())
    };
    
    if let Ok(v) = HeaderValue::from_str(&etag) {
        headers.insert(header::ETAG, v);
    }
    (StatusCode::OK, headers, Body::from((*bytes).clone())).into_response()
}

/// Render page 1 of a book (for thumbnail fallback or thumbnail checkup). Uses thumbnail dimensions by default.
pub async fn render_book_page_1(
    state: &AppState,
    book_id: Uuid,
    width: u32,
    quality: u8,
) -> Result<Vec<u8>, ApiError> {
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
                &OutputFormat::Webp,
                quality,
                width,
                filter,
            )
        }),
    )
    .await
    .map_err(|_| ApiError::internal("page rendering timeout"))?
    .map_err(|e| ApiError::internal(format!("render task failed: {e}")))?;

    bytes
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

    transcode_image(&page_bytes, out_format, quality, width, filter)
}


fn transcode_image(input: &[u8], out_format: &OutputFormat, quality: u8, width: u32, filter: image::imageops::FilterType) -> Result<Vec<u8>, ApiError> {
    debug!("Transcoding image: {} bytes, format: {:?}, quality: {}, width: {}", input.len(), out_format, quality, width);
    let source_format = image::guess_format(input).ok();
    debug!("Source format detected: {:?}", source_format);
    let needs_transcode = source_format.map(|f| !format_matches(&f, out_format)).unwrap_or(true);

    if width == 0 && !needs_transcode {
        debug!("No transcoding needed, returning original");
        return Ok(input.to_vec());
    }

    debug!("Loading image from memory...");
    let mut image = image::load_from_memory(input).map_err(|e| {
        error!("Failed to load image from memory: {} (input size: {} bytes)", e, input.len());
        ApiError::internal(format!("invalid source image: {e}"))
    })?;

    if width > 0 {
        debug!("Resizing image to width: {}", width);
        image = image.resize(width, u32::MAX, filter);
    }

    debug!("Converting to RGBA...");
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    debug!("Image dimensions: {}x{}", w, h);
    
    let mut out = Vec::new();
    match out_format {
        OutputFormat::Jpeg => {
            let mut encoder = JpegEncoder::new_with_quality(&mut out, quality);
            encoder
                .encode(&rgba, w, h, ColorType::Rgba8.into())
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
                .encode(f32::max(quality as f32, 85.0));
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

