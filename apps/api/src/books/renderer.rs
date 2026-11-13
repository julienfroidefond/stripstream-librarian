//! The one page-rendering pipeline of the API (ADR 0023).
//!
//! [`PageRenderer::get`] owns the sequence every page read goes through: memory cache →
//! disk cache → per-key render lock (coalesces concurrent misses) → render permit
//! (`page_render_limit`) → timeout → `spawn_blocking`. [`PageRenderer::run`] exposes the
//! permit → timeout → `spawn_blocking` stage alone for uncached work (on-the-fly thumbnail).
//!
//! The archive I/O sits behind [`PageSource`]: [`ArchivePages`] reads real books through
//! `parsers`, tests inject a fake that counts and delays extractions.

use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

use image::{
    codecs::jpeg::JpegEncoder, codecs::png::PngEncoder, imageops::FilterType, ColorType,
    ImageEncoder, ImageFormat,
};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tracing::{debug, error, info};
use uuid::Uuid;

use parsers::BookFormat;
use stripstream_thumbnails::fast_jpeg_decode;

use crate::{
    error::ApiError,
    state::{AppState, Metrics, PageCache, PageRenderLocks, RenderLimit},
};

/// How long a prefetch waits for a render permit before giving up.
const PREFETCH_PERMIT_WAIT: Duration = Duration::from_millis(100);
/// Render width used for PDF pages when the request does not ask for one.
const DEFAULT_PDF_RENDER_WIDTH: u32 = 1200;

/// Blocking access to the raw bytes of one page of a book file.
pub(crate) trait PageSource: Send + Sync + 'static {
    fn extract(
        &self,
        path: &Path,
        format: BookFormat,
        page: u32,
        pdf_render_width: u32,
    ) -> anyhow::Result<Vec<u8>>;
}

/// Production [`PageSource`]: CBZ/CBR/PDF/EPUB through `parsers`.
pub(crate) struct ArchivePages;

impl PageSource for ArchivePages {
    fn extract(
        &self,
        path: &Path,
        format: BookFormat,
        page: u32,
        pdf_render_width: u32,
    ) -> anyhow::Result<Vec<u8>> {
        parsers::extract_page(path, format, page, pdf_render_width)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    /// Serve raw bytes from the archive — no decode, no re-encode.
    Original,
    Jpeg,
    Png,
    Webp,
}

impl OutputFormat {
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, ApiError> {
        match value {
            None => Ok(Self::Original),
            Some("original") => Ok(Self::Original),
            Some("jpeg") | Some("jpg") => Ok(Self::Jpeg),
            Some("png") => Ok(Self::Png),
            Some("webp") => Ok(Self::Webp),
            _ => Err(ApiError::bad_request(
                "format must be original|webp|jpeg|png",
            )),
        }
    }

    pub(crate) fn content_type(&self) -> &'static str {
        match self {
            Self::Original => "application/octet-stream", // will be overridden by detected type
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
        }
    }

    pub(crate) fn extension(&self) -> &'static str {
        match self {
            Self::Original => "orig",
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }
}

/// Output parameters of a rendered page, shared by a page and its prefetched neighbours.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RenderOptions {
    pub output: OutputFormat,
    pub quality: u8,
    /// Target width; `0` keeps the source size.
    pub width: u32,
    pub filter: FilterType,
}

/// One page of one book file, rendered with given options. The cache key (file path,
/// mtime and options) is computed once, at construction.
#[derive(Clone, Debug)]
pub(crate) struct PageSpec {
    path: PathBuf,
    format: BookFormat,
    page: u32,
    options: RenderOptions,
    key: String,
}

impl PageSpec {
    /// `format` is the database format of the file (`book_files.format`), never guessed
    /// from the extension.
    pub(crate) fn new(
        path: PathBuf,
        format: BookFormat,
        page: u32,
        options: RenderOptions,
    ) -> Self {
        let path_text = path.to_string_lossy();
        let key = get_cache_key(
            &path_text,
            file_mtime_ns(&path_text),
            page,
            options.output.extension(),
            options.quality,
            options.width,
        );
        Self {
            path,
            format,
            page,
            options,
            key,
        }
    }

    /// The same file and options, another page.
    pub(crate) fn at_page(&self, page: u32) -> Self {
        Self::new(self.path.clone(), self.format, page, self.options)
    }

    /// Cache key, also used as the HTTP `ETag`.
    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    fn cache_path(&self, cache_dir: &Path) -> PathBuf {
        get_cache_path(&self.key, &self.options.output, cache_dir)
    }
}

/// Who is asking: a reader waiting for the page, or a best-effort prefetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Urgency {
    /// Waits for a permit, reports errors and counts cache hits/misses.
    Foreground,
    /// Skips cached pages, gives up when no permit frees up quickly, swallows errors.
    Prefetch,
}

/// Where the bytes of a [`Rendered`] page came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    Memory,
    Disk,
    Rendered,
}

pub(crate) struct Rendered {
    pub bytes: Arc<[u8]>,
    pub key: String,
    pub origin: Origin,
}

/// The page-rendering pipeline over the shared caches and limits of [`AppState`].
#[derive(Clone)]
pub(crate) struct PageRenderer {
    cache: Arc<Mutex<PageCache>>,
    locks: Arc<PageRenderLocks>,
    limit: Arc<RenderLimit>,
    metrics: Arc<Metrics>,
    cache_dir: PathBuf,
    timeout: Duration,
    source: Arc<dyn PageSource>,
}

impl PageRenderer {
    pub(crate) fn new(
        cache: Arc<Mutex<PageCache>>,
        locks: Arc<PageRenderLocks>,
        limit: Arc<RenderLimit>,
        metrics: Arc<Metrics>,
        cache_dir: PathBuf,
        timeout: Duration,
        source: Arc<dyn PageSource>,
    ) -> Self {
        Self {
            cache,
            locks,
            limit,
            metrics,
            cache_dir,
            timeout,
            source,
        }
    }

    /// A renderer over the state's caches, with the current cache directory and timeout.
    pub(crate) async fn for_state(state: &AppState, source: Arc<dyn PageSource>) -> Self {
        let (cache_dir, timeout_secs) = {
            let settings = state.settings.read().await;
            (settings.cache_directory.clone(), settings.timeout_seconds)
        };
        Self::new(
            state.page_cache.clone(),
            state.page_render_locks.clone(),
            state.page_render_limit.clone(),
            state.metrics.clone(),
            PathBuf::from(cache_dir),
            Duration::from_secs(timeout_secs),
            source,
        )
    }

    /// Serve a page from the caches or render it.
    ///
    /// `Foreground` always yields `Some` or an error. `Prefetch` yields `None` when the
    /// page is already cached, no permit freed up in time, or the render failed; it
    /// never renders a key that another request is rendering.
    pub(crate) async fn get(
        &self,
        spec: &PageSpec,
        urgency: Urgency,
    ) -> Result<Option<Rendered>, ApiError> {
        let cache_path = spec.cache_path(&self.cache_dir);

        let _render_lock = match urgency {
            Urgency::Foreground => {
                if let Some(hit) = self.memory_hit(spec).await {
                    return Ok(Some(hit));
                }
                self.metrics
                    .page_cache_misses
                    .fetch_add(1, Ordering::Relaxed);
                if let Some(hit) = self.disk_hit(spec, &cache_path).await {
                    return Ok(Some(hit));
                }
                // Coalesce concurrent misses. Recheck both levels after waiting: the
                // holder of this key may have completed the render in the meantime.
                let lock = self.locks.acquire(spec.key()).await;
                if let Some(hit) = self.memory_hit(spec).await {
                    return Ok(Some(hit));
                }
                if let Some(hit) = self.disk_hit(spec, &cache_path).await {
                    return Ok(Some(hit));
                }
                lock
            }
            Urgency::Prefetch => {
                if self.is_cached(spec, &cache_path).await {
                    return Ok(None);
                }
                let lock = self.locks.acquire(spec.key()).await;
                if self.is_cached(spec, &cache_path).await {
                    return Ok(None);
                }
                lock
            }
        };

        let job_spec = spec.clone();
        let started = Instant::now();
        let rendered = self
            .run_with(urgency, move |source| render_page(source, &job_spec))
            .await;
        let data = match (rendered, urgency) {
            (Ok(Some(data)), _) => data,
            (Ok(None), _) => return Ok(None),
            (Err(e), Urgency::Prefetch) => {
                debug!(
                    "Prefetch of page {} from {} failed: {}",
                    spec.page,
                    spec.path.display(),
                    e.message
                );
                return Ok(None);
            }
            (Err(e), Urgency::Foreground) => {
                error!(
                    "Failed to render page {} from {}: {}",
                    spec.page,
                    spec.path.display(),
                    e.message
                );
                return Err(e);
            }
        };
        if urgency == Urgency::Foreground {
            info!("Rendered page {} in {:?}", spec.page, started.elapsed());
        }

        let bytes: Arc<[u8]> = Arc::from(data);
        if let Err(e) = write_to_disk_cache(cache_path, bytes.clone()).await {
            tracing::warn!("Failed to write to disk cache: {}", e);
        }
        self.cache
            .lock()
            .await
            .put(spec.key().to_string(), bytes.clone());
        Ok(Some(Rendered {
            bytes,
            key: spec.key().to_string(),
            origin: Origin::Rendered,
        }))
    }

    /// Render `spec` in the background, best effort.
    pub(crate) fn prefetch(&self, spec: PageSpec) {
        let renderer = self.clone();
        tokio::spawn(async move {
            let _ = renderer.get(&spec, Urgency::Prefetch).await;
        });
    }

    /// Run uncached blocking work on a [`PageSource`] under the render permit and timeout.
    pub(crate) async fn run<T, F>(&self, job: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&dyn PageSource) -> Result<T, ApiError> + Send + 'static,
    {
        self.run_with(Urgency::Foreground, job)
            .await?
            .ok_or_else(|| ApiError::internal("render limiter unavailable"))
    }

    /// Permit → timeout → `spawn_blocking`. `None` when a prefetch found no permit.
    async fn run_with<T, F>(&self, urgency: Urgency, job: F) -> Result<Option<T>, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&dyn PageSource) -> Result<T, ApiError> + Send + 'static,
    {
        let _permit = match urgency {
            Urgency::Foreground => self.limit.clone().acquire_owned().await.map_err(|e| {
                error!("Failed to acquire render permit: {}", e);
                ApiError::internal("render limiter unavailable")
            })?,
            Urgency::Prefetch => {
                match tokio::time::timeout(PREFETCH_PERMIT_WAIT, self.limit.clone().acquire_owned())
                    .await
                {
                    Ok(Ok(permit)) => permit,
                    _ => return Ok(None),
                }
            }
        };

        let source = self.source.clone();
        tokio::time::timeout(
            self.timeout,
            tokio::task::spawn_blocking(move || job(source.as_ref())),
        )
        .await
        .map_err(|_| ApiError::internal("page rendering timeout"))?
        .map_err(|e| ApiError::internal(format!("render task failed: {e}")))?
        .map(Some)
    }

    async fn memory_hit(&self, spec: &PageSpec) -> Option<Rendered> {
        let bytes = self.cache.lock().await.get(spec.key()).cloned()?;
        self.metrics.page_cache_hits.fetch_add(1, Ordering::Relaxed);
        Some(Rendered {
            bytes,
            key: spec.key().to_string(),
            origin: Origin::Memory,
        })
    }

    async fn disk_hit(&self, spec: &PageSpec, cache_path: &Path) -> Option<Rendered> {
        let bytes: Arc<[u8]> = Arc::from(read_from_disk_cache(cache_path.to_path_buf()).await?);
        self.cache
            .lock()
            .await
            .put(spec.key().to_string(), bytes.clone());
        Some(Rendered {
            bytes,
            key: spec.key().to_string(),
            origin: Origin::Disk,
        })
    }

    async fn is_cached(&self, spec: &PageSpec, cache_path: &Path) -> bool {
        self.cache.lock().await.contains(spec.key()) || cache_path.exists()
    }
}

fn get_cache_key(
    abs_path: &str,
    mtime_ns: i128,
    page: u32,
    format: &str,
    quality: u8,
    width: u32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(abs_path.as_bytes());
    hasher.update(mtime_ns.to_le_bytes());
    hasher.update(page.to_le_bytes());
    hasher.update(format.as_bytes());
    hasher.update(quality.to_le_bytes());
    hasher.update(width.to_le_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Read the file mtime as nanoseconds since UNIX epoch. Returns 0 if unavailable
/// — that is OK because callers feed it as part of the cache key, not for correctness.
fn file_mtime_ns(path: &str) -> i128 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0)
}

fn get_cache_path(cache_key: &str, format: &OutputFormat, cache_dir: &Path) -> PathBuf {
    let prefix = &cache_key[..2];
    let ext = format.extension();
    cache_dir
        .join(prefix)
        .join(format!("{}.{}", cache_key, ext))
}

async fn read_from_disk_cache(cache_path: PathBuf) -> Option<Vec<u8>> {
    tokio::task::spawn_blocking(move || std::fs::read(cache_path).ok())
        .await
        .ok()
        .flatten()
}

async fn write_to_disk_cache(cache_path: PathBuf, data: Arc<[u8]>) -> Result<(), std::io::Error> {
    tokio::task::spawn_blocking(move || write_to_disk_cache_blocking(&cache_path, &data))
        .await
        .map_err(std::io::Error::other)?
}

fn write_to_disk_cache_blocking(cache_path: &Path, data: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let filename = cache_path.file_name().unwrap_or_default().to_string_lossy();
    let temporary_path = cache_path.with_file_name(format!(".{filename}-{}.tmp", Uuid::new_v4()));
    let mut file = std::fs::File::create(&temporary_path)?;
    file.write_all(data)?;
    // No sync_data() — this is a cache, durability is not critical
    drop(file);
    std::fs::rename(temporary_path, cache_path)
}

fn render_page(source: &dyn PageSource, spec: &PageSpec) -> Result<Vec<u8>, ApiError> {
    let RenderOptions {
        output,
        quality,
        width,
        filter,
    } = spec.options;
    let pdf_render_width = if width > 0 {
        width
    } else {
        DEFAULT_PDF_RENDER_WIDTH
    };
    let page_bytes = source
        .extract(&spec.path, spec.format, spec.page, pdf_render_width)
        .map_err(|e| ApiError::internal(format!("page extraction failed: {e}")))?;

    // Original mode or source matches output with no resize → return raw bytes (zero transcoding)
    if output == OutputFormat::Original && width == 0 {
        return Ok(page_bytes);
    }
    if width == 0 {
        if let Ok(source_fmt) = image::guess_format(&page_bytes) {
            if format_matches(&source_fmt, &output) {
                return Ok(page_bytes);
            }
        }
    }

    transcode_image(&page_bytes, &output, quality, width, filter)
}

fn transcode_image(
    input: &[u8],
    out_format: &OutputFormat,
    quality: u8,
    width: u32,
    filter: FilterType,
) -> Result<Vec<u8>, ApiError> {
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

    let needs_transcode = source_format
        .map(|f| !format_matches(&f, &effective_format))
        .unwrap_or(true);

    if width == 0 && !needs_transcode {
        return Ok(input.to_vec());
    }

    // For JPEG with resize: use DCT scaling to decode at ~target size (much faster)
    let mut image = if width > 0 {
        fast_jpeg_decode(input, width, u32::MAX)
            .map(|decoded| decoded.image)
            .unwrap_or_else(|| image::load_from_memory(input).unwrap_or_default())
    } else {
        image::load_from_memory(input)
            .map_err(|e| ApiError::internal(format!("invalid source image: {e}")))?
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
            let rgb_data: Vec<u8> = rgba.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect();
            let webp_data =
                webp::Encoder::new(&rgb_data, webp::PixelLayout::Rgb, w, h).encode(quality as f32);
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

/// Test [`PageSource`]: serves one fixed image for every page, optionally after a delay,
/// and records each extraction.
#[cfg(test)]
pub(crate) struct FakePages {
    pub image: Vec<u8>,
    pub delay: Duration,
    pub calls: std::sync::Mutex<Vec<(PathBuf, BookFormat, u32)>>,
}

#[cfg(test)]
impl FakePages {
    pub(crate) fn new(image: Vec<u8>) -> Self {
        Self {
            image,
            delay: Duration::ZERO,
            calls: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn slow(image: Vec<u8>, delay: Duration) -> Self {
        Self {
            delay,
            ..Self::new(image)
        }
    }

    pub(crate) fn calls(&self) -> Vec<(PathBuf, BookFormat, u32)> {
        self.calls.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl PageSource for FakePages {
    fn extract(
        &self,
        path: &Path,
        format: BookFormat,
        page: u32,
        _pdf_render_width: u32,
    ) -> anyhow::Result<Vec<u8>> {
        self.calls
            .lock()
            .unwrap()
            .push((path.to_path_buf(), format, page));
        std::thread::sleep(self.delay);
        Ok(self.image.clone())
    }
}

#[cfg(test)]
pub(crate) fn solid_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba(rgba));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut out, ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_OPTIONS: RenderOptions = RenderOptions {
        output: OutputFormat::Png,
        quality: 90,
        width: 0,
        filter: FilterType::Triangle,
    };

    struct Fixture {
        _library: tempfile::TempDir,
        cache_dir: tempfile::TempDir,
        book: PathBuf,
        metrics: Arc<Metrics>,
        limit: Arc<RenderLimit>,
        source: Arc<FakePages>,
    }

    impl Fixture {
        fn new(source: FakePages) -> Self {
            let library = tempfile::tempdir().unwrap();
            let book = library.path().join("T01.cbz");
            std::fs::write(&book, b"archive").unwrap();
            Self {
                _library: library,
                cache_dir: tempfile::tempdir().unwrap(),
                book,
                metrics: Arc::new(Metrics::new()),
                limit: Arc::new(RenderLimit::new(std::num::NonZeroUsize::MIN)),
                source: Arc::new(source),
            }
        }

        /// A renderer over a fresh memory cache, sharing the disk cache, metrics,
        /// permits and page source of the fixture.
        fn renderer(&self, timeout: Duration) -> PageRenderer {
            PageRenderer::new(
                Arc::new(Mutex::new(PageCache::new(16))),
                Arc::new(PageRenderLocks::new(16)),
                self.limit.clone(),
                self.metrics.clone(),
                self.cache_dir.path().to_path_buf(),
                timeout,
                self.source.clone(),
            )
        }

        fn spec(&self, page: u32) -> PageSpec {
            PageSpec::new(self.book.clone(), BookFormat::Cbz, page, PNG_OPTIONS)
        }

        fn hits(&self) -> u64 {
            self.metrics.page_cache_hits.load(Ordering::Relaxed)
        }

        fn misses(&self) -> u64 {
            self.metrics.page_cache_misses.load(Ordering::Relaxed)
        }
    }

    fn red_png() -> Vec<u8> {
        solid_png(4, 4, [255, 0, 0, 255])
    }

    #[tokio::test]
    async fn get_renders_once_then_serves_the_memory_cache() {
        let fixture = Fixture::new(FakePages::new(red_png()));
        let renderer = fixture.renderer(Duration::from_secs(5));
        let spec = fixture.spec(1);

        let first = renderer
            .get(&spec, Urgency::Foreground)
            .await
            .unwrap()
            .unwrap();
        let second = renderer
            .get(&spec, Urgency::Foreground)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(first.origin, Origin::Rendered);
        assert_eq!(second.origin, Origin::Memory);
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(first.key, spec.key());
        assert_eq!(fixture.source.calls().len(), 1);
        assert_eq!((fixture.hits(), fixture.misses()), (1, 1));
    }

    #[tokio::test]
    async fn get_serves_the_disk_cache_to_a_cold_memory_cache() {
        let fixture = Fixture::new(FakePages::new(red_png()));
        let spec = fixture.spec(1);
        fixture
            .renderer(Duration::from_secs(5))
            .get(&spec, Urgency::Foreground)
            .await
            .unwrap();

        // A restart: same disk cache, empty memory cache.
        let restarted = fixture.renderer(Duration::from_secs(5));
        let from_disk = restarted
            .get(&spec, Urgency::Foreground)
            .await
            .unwrap()
            .unwrap();
        let from_memory = restarted
            .get(&spec, Urgency::Foreground)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(from_disk.origin, Origin::Disk);
        assert_eq!(&from_disk.bytes[..], &red_png()[..]);
        assert_eq!(from_memory.origin, Origin::Memory);
        assert_eq!(fixture.source.calls().len(), 1);
    }

    #[tokio::test]
    async fn get_times_out_without_caching_anything() {
        let fixture = Fixture::new(FakePages::slow(red_png(), Duration::from_millis(300)));
        let renderer = fixture.renderer(Duration::from_millis(20));
        let spec = fixture.spec(1);

        let error = match renderer.get(&spec, Urgency::Foreground).await {
            Ok(_) => panic!("expected a timeout"),
            Err(error) => error,
        };

        assert_eq!(error.status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.message, "page rendering timeout");
        assert!(
            !renderer
                .is_cached(&spec, &spec.cache_path(&renderer.cache_dir))
                .await
        );
    }

    #[tokio::test]
    async fn prefetch_skips_a_page_already_rendered() {
        let fixture = Fixture::new(FakePages::new(red_png()));
        let renderer = fixture.renderer(Duration::from_secs(5));
        let spec = fixture.spec(2);
        renderer.get(&spec, Urgency::Foreground).await.unwrap();

        let prefetched = renderer.get(&spec, Urgency::Prefetch).await.unwrap();

        assert!(prefetched.is_none());
        assert_eq!(fixture.source.calls().len(), 1);
        // Prefetches never touch the hit/miss metrics.
        assert_eq!((fixture.hits(), fixture.misses()), (0, 1));
    }

    #[tokio::test]
    async fn prefetch_racing_a_reader_does_not_render_twice() {
        let fixture = Fixture::new(FakePages::slow(red_png(), Duration::from_millis(100)));
        let renderer = fixture.renderer(Duration::from_secs(5));
        // Two permits so that only the render lock can prevent a second extraction.
        fixture.limit.set_limit(std::num::NonZeroUsize::new(2).unwrap());
        let spec = fixture.spec(3);

        let (reader, prefetch) = tokio::join!(renderer.get(&spec, Urgency::Foreground), async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            renderer.get(&spec, Urgency::Prefetch).await
        });

        assert_eq!(reader.unwrap().unwrap().origin, Origin::Rendered);
        assert!(prefetch.unwrap().is_none());
        assert_eq!(fixture.source.calls().len(), 1);
    }

    #[tokio::test]
    async fn prefetch_gives_up_when_no_permit_frees_up() {
        let fixture = Fixture::new(FakePages::new(red_png()));
        let renderer = fixture.renderer(Duration::from_secs(5));
        let _busy = fixture.limit.clone().acquire_owned().await.unwrap();

        let prefetched = renderer
            .get(&fixture.spec(4), Urgency::Prefetch)
            .await
            .unwrap();

        assert!(prefetched.is_none());
        assert!(fixture.source.calls().is_empty());
    }

    #[tokio::test]
    async fn run_executes_uncached_work_with_the_source() {
        let fixture = Fixture::new(FakePages::new(red_png()));
        let renderer = fixture.renderer(Duration::from_secs(5));
        let book = fixture.book.clone();

        let length = renderer
            .run(move |source| {
                source
                    .extract(&book, BookFormat::Pdf, 1, 300)
                    .map(|bytes| bytes.len())
                    .map_err(|e| ApiError::internal(e.to_string()))
            })
            .await
            .unwrap();

        assert_eq!(length, red_png().len());
        assert_eq!(
            fixture.source.calls(),
            vec![(fixture.book.clone(), BookFormat::Pdf, 1)]
        );
    }

    #[test]
    fn page_spec_key_follows_page_and_options() {
        let fixture = Fixture::new(FakePages::new(red_png()));
        let spec = fixture.spec(1);

        assert_eq!(spec.key(), fixture.spec(1).key());
        assert_ne!(spec.key(), spec.at_page(2).key());
        assert_eq!(spec.at_page(2).key(), fixture.spec(2).key());
        let webp = PageSpec::new(
            fixture.book.clone(),
            BookFormat::Cbz,
            1,
            RenderOptions {
                output: OutputFormat::Webp,
                ..PNG_OPTIONS
            },
        );
        assert_ne!(spec.key(), webp.key());
    }

    #[test]
    fn cache_key_changes_when_mtime_changes() {
        // Same file, same render params, different mtime → different cache key.
        // This ensures an in-place file replacement invalidates the disk cache.
        let a = get_cache_key("/libraries/x.cbz", 1000, 1, "webp", 80, 600);
        let b = get_cache_key("/libraries/x.cbz", 2000, 1, "webp", 80, 600);
        assert_ne!(a, b, "cache key must include mtime");
    }

    #[test]
    fn cache_key_stable_when_inputs_unchanged() {
        let a = get_cache_key("/libraries/x.cbz", 1000, 1, "webp", 80, 600);
        let b = get_cache_key("/libraries/x.cbz", 1000, 1, "webp", 80, 600);
        assert_eq!(a, b);
    }

    #[test]
    fn cache_key_changes_with_path_and_render_params() {
        let base = get_cache_key("/libraries/x.cbz", 1000, 1, "webp", 80, 600);
        assert_ne!(
            base,
            get_cache_key("/libraries/y.cbz", 1000, 1, "webp", 80, 600)
        );
        assert_ne!(
            base,
            get_cache_key("/libraries/x.cbz", 1000, 2, "webp", 80, 600)
        );
        assert_ne!(
            base,
            get_cache_key("/libraries/x.cbz", 1000, 1, "jpeg", 80, 600)
        );
        assert_ne!(
            base,
            get_cache_key("/libraries/x.cbz", 1000, 1, "webp", 90, 600)
        );
        assert_ne!(
            base,
            get_cache_key("/libraries/x.cbz", 1000, 1, "webp", 80, 800)
        );
    }

    #[tokio::test]
    async fn disk_cache_round_trip_uses_the_published_file() {
        let directory = tempfile::tempdir().expect("temporary cache directory");
        let cache_path = directory.path().join("ab/page.webp");
        let expected: Arc<[u8]> = Arc::from(&b"image bytes"[..]);

        write_to_disk_cache(cache_path.clone(), expected.clone())
            .await
            .expect("write cache entry");

        assert_eq!(
            read_from_disk_cache(cache_path).await.as_deref(),
            Some(expected.as_ref())
        );
    }

    #[test]
    fn output_format_parse_accepts_aliases_and_rejects_unknown() {
        assert_eq!(OutputFormat::parse(None).unwrap(), OutputFormat::Original);
        assert_eq!(
            OutputFormat::parse(Some("original")).unwrap(),
            OutputFormat::Original
        );
        assert_eq!(
            OutputFormat::parse(Some("jpeg")).unwrap(),
            OutputFormat::Jpeg
        );
        assert_eq!(
            OutputFormat::parse(Some("jpg")).unwrap(),
            OutputFormat::Jpeg
        );
        assert_eq!(OutputFormat::parse(Some("png")).unwrap(), OutputFormat::Png);
        assert_eq!(
            OutputFormat::parse(Some("webp")).unwrap(),
            OutputFormat::Webp
        );
        assert!(OutputFormat::parse(Some("bmp")).is_err());
    }

    #[test]
    fn output_format_exposes_content_type_and_extension() {
        assert_eq!(OutputFormat::Jpeg.content_type(), "image/jpeg");
        assert_eq!(OutputFormat::Png.content_type(), "image/png");
        assert_eq!(OutputFormat::Webp.content_type(), "image/webp");
        assert_eq!(OutputFormat::Original.extension(), "orig");
        assert_eq!(OutputFormat::Jpeg.extension(), "jpg");
        assert_eq!(OutputFormat::Png.extension(), "png");
        assert_eq!(OutputFormat::Webp.extension(), "webp");
    }

    #[test]
    fn get_cache_path_shards_by_prefix_and_appends_extension() {
        let cache_key = "ab12cd34ef56";
        let path = get_cache_path(cache_key, &OutputFormat::Png, Path::new("/cache"));

        assert_eq!(path, PathBuf::from(format!("/cache/ab/{cache_key}.png")));
    }

    #[test]
    fn format_matches_only_the_same_image_family() {
        assert!(format_matches(&ImageFormat::Jpeg, &OutputFormat::Jpeg));
        assert!(format_matches(&ImageFormat::Png, &OutputFormat::Png));
        assert!(format_matches(&ImageFormat::WebP, &OutputFormat::Webp));
        assert!(!format_matches(&ImageFormat::Png, &OutputFormat::Jpeg));
        assert!(!format_matches(&ImageFormat::Jpeg, &OutputFormat::Webp));
        assert!(!format_matches(&ImageFormat::Avif, &OutputFormat::Png));
    }

    #[test]
    fn transcode_image_returns_the_input_when_no_work_is_needed() {
        let png = solid_png(4, 4, [10, 20, 30, 255]);

        // Same format, no resize → strict passthrough (no re-encode).
        let out = transcode_image(&png, &OutputFormat::Png, 100, 0, FilterType::Triangle).unwrap();

        assert_eq!(out, png);
    }

    #[test]
    fn transcode_image_resizes_and_re_encodes() {
        let png = solid_png(4, 4, [10, 20, 30, 255]);

        // Width 2 → aspect-preserving resize to 2×2, still PNG.
        let resized =
            transcode_image(&png, &OutputFormat::Png, 100, 2, FilterType::Triangle).unwrap();
        let decoded = image::load_from_memory(&resized).unwrap();
        assert_eq!(decoded.width(), 2);
        assert_eq!(decoded.height(), 2);

        // Forced JPEG output even though the source is PNG.
        let jpeg = transcode_image(&png, &OutputFormat::Jpeg, 90, 0, FilterType::Triangle).unwrap();
        assert_eq!(image::guess_format(&jpeg).unwrap(), ImageFormat::Jpeg);

        // WebP output keeps the RIFF/WEBP container magic.
        let webp = transcode_image(&png, &OutputFormat::Webp, 80, 0, FilterType::Triangle).unwrap();
        assert_eq!(&webp[0..4], b"RIFF");
        assert_eq!(&webp[8..12], b"WEBP");
    }

    #[test]
    fn file_mtime_ns_is_zero_for_a_missing_file_and_positive_otherwise() {
        assert_eq!(file_mtime_ns("/definitely/not/here.cbz"), 0);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.cbz");
        std::fs::write(&path, b"x").unwrap();
        assert!(file_mtime_ns(&path.to_string_lossy()) > 0);
    }
}
