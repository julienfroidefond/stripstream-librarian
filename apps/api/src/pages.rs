use std::{
    io::{Read, Write},
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

use crate::{error::ApiError, AppState};

fn remap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with("/libraries/") {
            return path.replacen("/libraries", &root, 1);
        }
    }
    path.to_string()
}

fn get_image_cache_dir() -> PathBuf {
    std::env::var("IMAGE_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/stripstream-image-cache"))
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

fn get_cache_path(cache_key: &str, format: &OutputFormat) -> PathBuf {
    let cache_dir = get_image_cache_dir();
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

    let format = OutputFormat::parse(query.format.as_deref())?;
    let quality = query.quality.unwrap_or(80).clamp(1, 100);
    let width = query.width.unwrap_or(0);
    if width > 2160 {
        warn!("Invalid width: {}", width);
        return Err(ApiError::bad_request("width must be <= 2160"));
    }

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
    let cache_path = get_cache_path(&disk_cache_key, &format);
    
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
        Duration::from_secs(12),
        tokio::task::spawn_blocking(move || {
            render_page(&abs_path_clone, &input_format, n, &format_clone, quality, width)
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

fn render_page(
    abs_path: &str,
    input_format: &str,
    page_number: u32,
    out_format: &OutputFormat,
    quality: u8,
    width: u32,
) -> Result<Vec<u8>, ApiError> {
    let page_bytes = match input_format {
        "cbz" => extract_cbz_page(abs_path, page_number)?,
        "cbr" => extract_cbr_page(abs_path, page_number)?,
        "pdf" => render_pdf_page(abs_path, page_number, width)?,
        _ => return Err(ApiError::bad_request("unsupported source format")),
    };

    transcode_image(&page_bytes, out_format, quality, width)
}

fn extract_cbz_page(abs_path: &str, page_number: u32) -> Result<Vec<u8>, ApiError> {
    debug!("Opening CBZ archive: {}", abs_path);
    let file = std::fs::File::open(abs_path).map_err(|e| {
        error!("Cannot open CBZ file {}: {}", abs_path, e);
        ApiError::internal(format!("cannot open cbz: {e}"))
    })?;
    
    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        error!("Invalid CBZ archive {}: {}", abs_path, e);
        ApiError::internal(format!("invalid cbz: {e}"))
    })?;

    let mut image_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| {
            error!("Failed to read CBZ entry {} in {}: {}", i, abs_path, e);
            ApiError::internal(format!("cbz entry read failed: {e}"))
        })?;
        let name = entry.name().to_ascii_lowercase();
        if is_image_name(&name) {
            image_names.push(entry.name().to_string());
        }
    }
    image_names.sort();
    debug!("Found {} images in CBZ {}", image_names.len(), abs_path);

    let index = page_number as usize - 1;
    let selected = image_names.get(index).ok_or_else(|| {
        error!("Page {} out of range in {} (total: {})", page_number, abs_path, image_names.len());
        ApiError::not_found("page out of range")
    })?;
    
    debug!("Extracting page {} ({}) from {}", page_number, selected, abs_path);
    let mut entry = archive.by_name(selected).map_err(|e| {
        error!("Failed to read CBZ page {} from {}: {}", selected, abs_path, e);
        ApiError::internal(format!("cbz page read failed: {e}"))
    })?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).map_err(|e| {
        error!("Failed to load CBZ page {} from {}: {}", selected, abs_path, e);
        ApiError::internal(format!("cbz page load failed: {e}"))
    })?;
    Ok(buf)
}

fn extract_cbr_page(abs_path: &str, page_number: u32) -> Result<Vec<u8>, ApiError> {
    debug!("Listing CBR archive: {}", abs_path);
    let list_output = std::process::Command::new("unrar")
        .arg("lb")
        .arg(abs_path)
        .output()
        .map_err(|e| {
            error!("unrar list command failed for {}: {}", abs_path, e);
            ApiError::internal(format!("unrar list failed: {e}"))
        })?;
    if !list_output.status.success() {
        let stderr = String::from_utf8_lossy(&list_output.stderr);
        error!("unrar could not list archive {}: {}", abs_path, stderr);
        return Err(ApiError::internal("unrar could not list archive"));
    }

    let mut entries: Vec<String> = String::from_utf8_lossy(&list_output.stdout)
        .lines()
        .filter(|line| is_image_name(&line.to_ascii_lowercase()))
        .map(|s| s.to_string())
        .collect();
    entries.sort();
    
    // Debug: show first few entries
    if entries.len() > 0 {
        debug!("First 5 entries in CBR {}: {:?}", abs_path, &entries[..entries.len().min(5)]);
    }
    debug!("Found {} images in CBR {}", entries.len(), abs_path);

    let index = page_number as usize - 1;
    let selected = entries.get(index).ok_or_else(|| {
        error!("Page {} out of range in {} (total: {})", page_number, abs_path, entries.len());
        ApiError::not_found("page out of range")
    })?;

    debug!("Extracting page {} ({}) from {}", page_number, selected, abs_path);
    let page_output = std::process::Command::new("unrar")
        .arg("p")
        .arg("-inul")
        .arg("-y")
        .arg(abs_path)
        .arg(selected)
        .output()
        .map_err(|e| {
            error!("unrar extract command failed for {} page {}: {}", abs_path, selected, e);
            ApiError::internal(format!("unrar extract failed: {e}"))
        })?;
    if !page_output.status.success() {
        let stderr = String::from_utf8_lossy(&page_output.stderr);
        error!("unrar could not extract page {} from {}: {}", selected, abs_path, stderr);
        return Err(ApiError::internal("unrar could not extract page"));
    }
    
    let extracted_data = &page_output.stdout;
    debug!("Extracted {} bytes from CBR page {}", extracted_data.len(), page_number);
    
    // Verify it's actually an image by checking magic bytes
    if extracted_data.len() < 1000 {
        // Show first few bytes for debugging
        let preview: Vec<u8> = extracted_data.iter().take(32).copied().collect();
        let hex_preview: String = preview.iter().map(|b| format!("{:02x}", b)).collect();
        error!("Extracted data too small ({} bytes) for page {} from {} - first bytes: {}", 
            extracted_data.len(), page_number, abs_path, hex_preview);
        return Err(ApiError::internal("extracted data too small - not a valid image"));
    }
    
    // Check magic bytes to verify it's an image
    let is_valid_image = extracted_data.len() > 4 && (
        // JPEG
        extracted_data.starts_with(&[0xFF, 0xD8, 0xFF]) ||
        // PNG
        extracted_data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) ||
        // GIF
        extracted_data.starts_with(b"GIF87a") || extracted_data.starts_with(b"GIF89a") ||
        // WebP
        extracted_data.starts_with(b"RIFF") && extracted_data.len() > 12 && &extracted_data[8..12] == b"WEBP" ||
        // TIFF
        extracted_data.starts_with(&[0x49, 0x49, 0x2A, 0x00]) || extracted_data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A])
    );
    
    if !is_valid_image {
        // Show first few bytes for debugging
        let preview: Vec<u8> = extracted_data.iter().take(32).copied().collect();
        let hex_preview: String = preview.iter().map(|b| format!("{:02x}", b)).collect();
        error!("Extracted data for page {} from {} is not a valid image format. First bytes: {} (size: {})", 
            page_number, abs_path, hex_preview, extracted_data.len());
        return Err(ApiError::internal("extracted data is not a valid image"));
    }
    
    debug!("Successfully extracted {} bytes from CBR page {}", extracted_data.len(), page_number);
    Ok(extracted_data.to_vec())
}

fn render_pdf_page(abs_path: &str, page_number: u32, width: u32) -> Result<Vec<u8>, ApiError> {
    let tmp_dir = std::env::temp_dir().join(format!("stripstream-pdf-{}", Uuid::new_v4()));
    debug!("Creating temp dir for PDF rendering: {}", tmp_dir.display());
    std::fs::create_dir_all(&tmp_dir).map_err(|e| {
        error!("Cannot create temp dir {}: {}", tmp_dir.display(), e);
        ApiError::internal(format!("cannot create temp dir: {e}"))
    })?;
    let output_prefix = tmp_dir.join("page");

    let mut cmd = std::process::Command::new("pdftoppm");
    cmd.arg("-f")
        .arg(page_number.to_string())
        .arg("-singlefile")
        .arg("-png");
    if width > 0 {
        cmd.arg("-scale-to-x").arg(width.to_string()).arg("-scale-to-y").arg("-1");
    }
    cmd.arg(abs_path).arg(&output_prefix);

    debug!("Running pdftoppm for page {} of {} (width: {})", page_number, abs_path, width);
    let output = cmd
        .output()
        .map_err(|e| {
            error!("pdftoppm command failed for {} page {}: {}", abs_path, page_number, e);
            ApiError::internal(format!("pdf render failed: {e}"))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&tmp_dir);
        error!("pdftoppm failed for {} page {}: {}", abs_path, page_number, stderr);
        return Err(ApiError::internal("pdf render command failed"));
    }

    let image_path = output_prefix.with_extension("png");
    debug!("Reading rendered PDF page from: {}", image_path.display());
    let bytes = std::fs::read(&image_path).map_err(|e| {
        error!("Failed to read rendered PDF output {}: {}", image_path.display(), e);
        ApiError::internal(format!("render output missing: {e}"))
    })?;
    let _ = std::fs::remove_dir_all(&tmp_dir);
    debug!("Successfully rendered PDF page {} to {} bytes", page_number, bytes.len());
    Ok(bytes)
}

fn transcode_image(input: &[u8], out_format: &OutputFormat, quality: u8, width: u32) -> Result<Vec<u8>, ApiError> {
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
        image = image.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3);
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
    match (source, target) {
        (ImageFormat::Jpeg, OutputFormat::Jpeg) => true,
        (ImageFormat::Png, OutputFormat::Png) => true,
        (ImageFormat::WebP, OutputFormat::Webp) => true,
        _ => false,
    }
}

fn is_image_name(name: &str) -> bool {
    name.ends_with(".jpg")
        || name.ends_with(".jpeg")
        || name.ends_with(".png")
        || name.ends_with(".webp")
        || name.ends_with(".avif")
}

#[allow(dead_code)]
fn _is_absolute_path(value: &str) -> bool {
    Path::new(value).is_absolute()
}
