use std::{
    io::Read,
    path::Path,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use axum::{
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use image::{codecs::jpeg::JpegEncoder, codecs::png::PngEncoder, codecs::webp::WebPEncoder, ColorType, ImageEncoder};
use serde::Deserialize;
use utoipa::ToSchema;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::{error::ApiError, AppState};

#[derive(Deserialize, ToSchema)]
pub struct PageQuery {
    #[schema(value_type = Option<String>, example = "webp")]
    pub format: Option<String>,
    #[schema(value_type = Option<u8>, example = 80)]
    pub quality: Option<u8>,
    #[schema(value_type = Option<u32>, example = 1200)]
    pub width: Option<u32>,
}

#[derive(Clone, Copy)]
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
pub async fn get_page(
    State(state): State<AppState>,
    AxumPath((book_id, n)): AxumPath<(Uuid, u32)>,
    Query(query): Query<PageQuery>,
) -> Result<Response, ApiError> {
    if n == 0 {
        return Err(ApiError::bad_request("page index starts at 1"));
    }

    let format = OutputFormat::parse(query.format.as_deref())?;
    let quality = query.quality.unwrap_or(80).clamp(1, 100);
    let width = query.width.unwrap_or(0);
    if width > 2160 {
        return Err(ApiError::bad_request("width must be <= 2160"));
    }

    let cache_key = format!("{book_id}:{n}:{}:{quality}:{width}", format.extension());
    if let Some(cached) = state.page_cache.lock().await.get(&cache_key).cloned() {
        state.metrics.page_cache_hits.fetch_add(1, Ordering::Relaxed);
        return Ok(image_response(cached, format.content_type()));
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
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("book file not found"))?;
    let abs_path: String = row.get("abs_path");
    let input_format: String = row.get("format");

    let _permit = state
        .page_render_limit
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiError::internal("render limiter unavailable"))?;

    let bytes = tokio::time::timeout(
        Duration::from_secs(12),
        tokio::task::spawn_blocking(move || render_page(&abs_path, &input_format, n, &format, quality, width)),
    )
    .await
    .map_err(|_| ApiError::internal("page rendering timeout"))?
    .map_err(|e| ApiError::internal(format!("render task failed: {e}")))??;

    let bytes = Arc::new(bytes);
    state.page_cache.lock().await.put(cache_key, bytes.clone());

    Ok(image_response(bytes, format.content_type()))
}

fn image_response(bytes: Arc<Vec<u8>>, content_type: &str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=300"));
    let mut hasher = Sha256::new();
    hasher.update(&*bytes);
    let etag = format!("\"{:x}\"", hasher.finalize());
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
    let file = std::fs::File::open(abs_path).map_err(|e| ApiError::internal(format!("cannot open cbz: {e}")))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| ApiError::internal(format!("invalid cbz: {e}")))?;

    let mut image_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| ApiError::internal(format!("cbz entry read failed: {e}")))?;
        let name = entry.name().to_ascii_lowercase();
        if is_image_name(&name) {
            image_names.push(entry.name().to_string());
        }
    }
    image_names.sort();

    let index = page_number as usize - 1;
    let selected = image_names.get(index).ok_or_else(|| ApiError::not_found("page out of range"))?;
    let mut entry = archive.by_name(selected).map_err(|e| ApiError::internal(format!("cbz page read failed: {e}")))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).map_err(|e| ApiError::internal(format!("cbz page load failed: {e}")))?;
    Ok(buf)
}

fn extract_cbr_page(abs_path: &str, page_number: u32) -> Result<Vec<u8>, ApiError> {
    let list_output = std::process::Command::new("unrar")
        .arg("lb")
        .arg(abs_path)
        .output()
        .map_err(|e| ApiError::internal(format!("unrar list failed: {e}")))?;
    if !list_output.status.success() {
        return Err(ApiError::internal("unrar could not list archive"));
    }

    let mut entries: Vec<String> = String::from_utf8_lossy(&list_output.stdout)
        .lines()
        .filter(|line| is_image_name(&line.to_ascii_lowercase()))
        .map(|s| s.to_string())
        .collect();
    entries.sort();
    let index = page_number as usize - 1;
    let selected = entries.get(index).ok_or_else(|| ApiError::not_found("page out of range"))?;

    let page_output = std::process::Command::new("unrar")
        .arg("p")
        .arg("-inul")
        .arg(abs_path)
        .arg(selected)
        .output()
        .map_err(|e| ApiError::internal(format!("unrar extract failed: {e}")))?;
    if !page_output.status.success() {
        return Err(ApiError::internal("unrar could not extract page"));
    }
    Ok(page_output.stdout)
}

fn render_pdf_page(abs_path: &str, page_number: u32, width: u32) -> Result<Vec<u8>, ApiError> {
    let tmp_dir = std::env::temp_dir().join(format!("stripstream-pdf-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_dir).map_err(|e| ApiError::internal(format!("cannot create temp dir: {e}")))?;
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

    let output = cmd
        .output()
        .map_err(|e| ApiError::internal(format!("pdf render failed: {e}")))?;
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(ApiError::internal("pdf render command failed"));
    }

    let image_path = output_prefix.with_extension("png");
    let bytes = std::fs::read(&image_path).map_err(|e| ApiError::internal(format!("render output missing: {e}")))?;
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(bytes)
}

fn transcode_image(input: &[u8], out_format: &OutputFormat, quality: u8, width: u32) -> Result<Vec<u8>, ApiError> {
    let mut image = image::load_from_memory(input).map_err(|e| ApiError::internal(format!("invalid source image: {e}")))?;
    if width > 0 {
        image = image.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3);
    }

    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
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
            let encoder = WebPEncoder::new_lossless(&mut out);
            encoder
                .write_image(&rgba, w, h, ColorType::Rgba8.into())
                .map_err(|e| ApiError::internal(format!("webp encode failed: {e}")))?;
        }
    }
    Ok(out)
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
