use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookFormat {
    Cbz,
    Cbr,
    Pdf,
    Epub,
}

impl BookFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Pdf => "pdf",
            Self::Epub => "epub",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedMetadata {
    pub title: String,
    pub series: Option<String>,
    pub volume: Option<i32>,
    pub page_count: Option<i32>,
}

pub fn detect_format(path: &Path) -> Option<BookFormat> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "cbz" => Some(BookFormat::Cbz),
        "cbr" => Some(BookFormat::Cbr),
        "pdf" => Some(BookFormat::Pdf),
        "epub" => Some(BookFormat::Epub),
        _ => None,
    }
}

// Cache compiled regex patterns — compiled once on first use
static VOLUME_PATTERNS: OnceLock<Vec<(regex::Regex, usize)>> = OnceLock::new();

fn get_volume_patterns() -> &'static Vec<(regex::Regex, usize)> {
    VOLUME_PATTERNS.get_or_init(|| {
        [
            // Tome 3, Tome.007 (must be before T(\d+) to avoid false match on the T)
            (r"(?i)Tome\.?\s*(\d+)", 1usize),
            // T01, T02 pattern (most common for manga/comics)
            (r"(?i)T(\d+)", 1),
            // Vol 1, Vol. 1, Volume 1
            (r"(?i)Vol\.?\s*(\d+)", 1),
            (r"(?i)Volume\s*(\d+)", 1),
            // #1, #01
            (r"#(\d+)", 1),
            // - 1, - 01 at the end
            (r"-\s*(\d+)\s*$", 1),
        ]
        .iter()
        .filter_map(|(pattern, group)| {
            regex::Regex::new(pattern).ok().map(|re| (re, *group))
        })
        .collect()
    })
}

fn extract_volume(filename: &str) -> Option<i32> {
    for (re, group) in get_volume_patterns() {
        if let Some(caps) = re.captures(filename) {
            if let Some(mat) = caps.get(*group) {
                return mat.as_str().parse::<i32>().ok();
            }
        }
    }
    None
}

fn extract_series(path: &Path, library_root: &Path) -> Option<String> {
    path.parent().and_then(|parent| {
        let parent_str = parent.to_string_lossy().to_string();
        let root_str = library_root.to_string_lossy().to_string();

        let relative = if let Some(idx) = parent_str.find(&root_str) {
            let after_root = &parent_str[idx + root_str.len()..];
            Path::new(after_root)
        } else if let Ok(relative) = parent.strip_prefix(library_root) {
            relative
        } else {
            tracing::warn!(
                "[PARSER] Cannot determine series: parent '{}' doesn't start with root '{}'",
                parent.display(),
                library_root.display()
            );
            return None;
        };

        let relative_str = relative.to_string_lossy().to_string();
        let relative_clean = relative_str.trim_start_matches(['/', '\\']);

        if relative_clean.is_empty() {
            return None;
        }

        let first_sep = relative_clean.find(['/', '\\']);
        let series_name = match first_sep {
            Some(idx) => &relative_clean[..idx],
            None => relative_clean,
        };

        if series_name.is_empty() {
            None
        } else {
            Some(series_name.to_string())
        }
    })
}

/// Fast metadata extraction from filename only — no archive I/O. Always succeeds.
pub fn parse_metadata_fast(path: &Path, _format: BookFormat, library_root: &Path) -> ParsedMetadata {
    let filename = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    let volume = extract_volume(&filename);
    let title = filename;
    let series = extract_series(path, library_root);

    ParsedMetadata {
        title,
        series,
        volume,
        page_count: None,
    }
}

pub fn parse_metadata(
    path: &Path,
    format: BookFormat,
    library_root: &Path,
) -> Result<ParsedMetadata> {
    let mut meta = parse_metadata_fast(path, format, library_root);

    meta.page_count = match format {
        BookFormat::Cbz => parse_cbz_page_count(path).ok(),
        BookFormat::Cbr => parse_cbr_page_count(path).ok(),
        BookFormat::Pdf => parse_pdf_page_count(path).ok(),
        BookFormat::Epub => parse_epub_page_count(path).ok(),
    };

    Ok(meta)
}

/// Open an archive once and return (page_count, first_page_bytes).
/// `pdf_render_scale`: max dimension used for PDF rasterization; 0 means use default (400).
pub fn analyze_book(path: &Path, format: BookFormat, pdf_render_scale: u32) -> Result<(i32, Vec<u8>)> {
    match format {
        BookFormat::Cbz => analyze_cbz(path, true),
        BookFormat::Cbr => analyze_cbr(path, true),
        BookFormat::Pdf => analyze_pdf(path, pdf_render_scale),
        BookFormat::Epub => analyze_epub(path),
    }
}

fn analyze_cbz(path: &Path, allow_fallback: bool) -> Result<(i32, Vec<u8>)> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(zip_err) => {
            if allow_fallback {
                tracing::debug!(target: "extraction", "[EXTRACTION] ZipArchive::new failed for {}: {} — trying fallbacks", path.display(), zip_err);

                // Check magic bytes to avoid expensive RAR probe on ZIP files
                let is_zip_magic = std::fs::File::open(path)
                    .and_then(|mut f| {
                        let mut magic = [0u8; 4];
                        std::io::Read::read_exact(&mut f, &mut magic)?;
                        Ok(magic[0] == b'P' && magic[1] == b'K')
                    })
                    .unwrap_or(false);

                if !is_zip_magic {
                    // Try RAR fallback (file might be a RAR with .cbz extension)
                    if let Ok(result) = analyze_cbr(path, false) {
                        tracing::debug!(target: "extraction", "[EXTRACTION] RAR fallback succeeded for {}", path.display());
                        return Ok(result);
                    }
                }

                // Try streaming fallback: read local file headers without central directory
                // (handles ZIP files with NTFS extra fields that confuse the central dir parser)
                let t0 = std::time::Instant::now();
                if let Ok(result) = analyze_cbz_streaming(path) {
                    tracing::debug!(target: "extraction", "[EXTRACTION] Streaming fallback succeeded for {} — {} pages in {:.0}ms", path.display(), result.0, t0.elapsed().as_secs_f64() * 1000.0);
                    return Ok(result);
                }
            }
            return Err(anyhow::anyhow!("invalid cbz archive for {}: {}", path.display(), zip_err));
        }
    };

    let mut image_names: Vec<String> = archive
        .file_names()
        .filter(|name| is_image_name(&name.to_ascii_lowercase()))
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    image_names.sort_by(|a, b| natord::compare(a, b));

    if image_names.is_empty() {
        return Err(anyhow::anyhow!("no images found in cbz: {}", path.display()));
    }

    // Try images in order until one reads successfully (first pages can be corrupted too)
    let count = image_names.len() as i32;
    for first_image in &image_names {
        if let Ok(mut entry) = archive.by_name(first_image) {
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                return Ok((count, buf));
            }
        }
    }

    Err(anyhow::anyhow!("all entries unreadable in cbz: {}", path.display()))
}

// ---------------------------------------------------------------------------
// Raw ZIP reader — bypasses extra field validation (CRC32 on Unicode path, NTFS, etc.)
// ---------------------------------------------------------------------------

/// Information about a ZIP local file entry (parsed from raw headers).
struct RawZipEntry {
    name: String,
    compression: u16,
    compressed_size: u64,
    uncompressed_size: u64,
    /// File offset of the compressed data (right after name + extra field).
    data_offset: u64,
}

/// Scan local file headers and return metadata for all entries.
/// Does NOT read file data — only collects names and offsets.
fn raw_zip_list_entries(path: &Path) -> Result<Vec<RawZipEntry>> {
    use std::io::{BufReader, Seek, SeekFrom};

    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open zip: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut entries = Vec::new();

    loop {
        let mut sig = [0u8; 4];
        if reader.read_exact(&mut sig).is_err() {
            break;
        }
        if u32::from_le_bytes(sig) != 0x04034b50 {
            break;
        }

        let mut hdr = [0u8; 26];
        reader.read_exact(&mut hdr).context("truncated local file header")?;

        let compression = u16::from_le_bytes([hdr[4], hdr[5]]);
        let compressed_size = u32::from_le_bytes([hdr[14], hdr[15], hdr[16], hdr[17]]) as u64;
        let uncompressed_size = u32::from_le_bytes([hdr[18], hdr[19], hdr[20], hdr[21]]) as u64;
        let name_len = u16::from_le_bytes([hdr[22], hdr[23]]) as u64;
        let extra_len = u16::from_le_bytes([hdr[24], hdr[25]]) as u64;

        let mut name_buf = vec![0u8; name_len as usize];
        reader.read_exact(&mut name_buf)?;
        let name = String::from_utf8_lossy(&name_buf).to_string();

        // Skip extra field entirely
        if extra_len > 0 {
            reader.seek(SeekFrom::Current(extra_len as i64))?;
        }

        let data_offset = reader.stream_position()?;

        entries.push(RawZipEntry {
            name,
            compression,
            compressed_size,
            uncompressed_size,
            data_offset,
        });

        // Skip file data
        if compressed_size > 0 {
            reader.seek(SeekFrom::Current(compressed_size as i64))?;
        }
    }

    Ok(entries)
}

/// Read and decompress the data for a single entry.
fn raw_zip_read_entry(path: &Path, entry: &RawZipEntry) -> Result<Vec<u8>> {
    use std::io::{BufReader, Seek, SeekFrom};

    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(entry.data_offset))?;

    let mut compressed = vec![0u8; entry.compressed_size as usize];
    reader.read_exact(&mut compressed)?;

    match entry.compression {
        0 => Ok(compressed),
        8 => {
            let mut decoder = flate2::read::DeflateDecoder::new(&compressed[..]);
            let mut decompressed = Vec::with_capacity(entry.uncompressed_size as usize);
            decoder.read_to_end(&mut decompressed)?;
            Ok(decompressed)
        }
        other => Err(anyhow::anyhow!("unsupported zip compression method: {}", other)),
    }
}

/// Fallback: list image names + extract all images (for analyze_book which needs first page + count).
fn analyze_cbz_streaming(path: &Path) -> Result<(i32, Vec<u8>)> {
    let entries = raw_zip_list_entries(path)?;
    let mut image_entries: Vec<&RawZipEntry> = entries
        .iter()
        .filter(|e| is_image_name(&e.name.to_ascii_lowercase()))
        .collect();

    if image_entries.is_empty() {
        return Err(anyhow::anyhow!("no images found in streaming cbz: {}", path.display()));
    }

    image_entries.sort_by(|a, b| natord::compare(&a.name, &b.name));
    let count = image_entries.len() as i32;
    let first_bytes = raw_zip_read_entry(path, image_entries[0])?;
    Ok((count, first_bytes))
}

/// Returns true if the error indicates the file is not a RAR archive (likely a ZIP with .cbr extension).
fn is_not_rar_error(err_str: &str) -> bool {
    err_str.contains("Not a RAR archive") || err_str.contains("bad archive")
}

/// Try to open a CBR file for listing. Returns the archive or an error string.
/// If the error indicates the file is not actually a RAR archive, the caller can fall back to CBZ.
fn open_cbr_listing(path: &Path) -> std::result::Result<unrar::OpenArchive<unrar::List, unrar::CursorBeforeHeader>, String> {
    unrar::Archive::new(path)
        .open_for_listing()
        .map_err(|e| format!("unrar listing failed for {}: {}", path.display(), e))
}

fn analyze_cbr(path: &Path, allow_fallback: bool) -> Result<(i32, Vec<u8>)> {
    // Pass 1: list all image names via unrar (in-process, no subprocess)
    let mut image_names: Vec<String> = {
        // Some .cbr files are actually ZIP archives with wrong extension — fallback to CBZ parser
        let archive = match open_cbr_listing(path) {
            Ok(a) => a,
            Err(e_str) => {
                if allow_fallback && is_not_rar_error(&e_str) {
                    return analyze_cbz(path, false).map_err(|zip_err| {
                        anyhow::anyhow!(
                            "not a RAR archive and ZIP fallback also failed for {}: RAR={}, ZIP={}",
                            path.display(),
                            e_str,
                            zip_err
                        )
                    });
                }
                return Err(anyhow::anyhow!("{}", e_str));
            }
        };
        let mut names = Vec::new();
        for entry in archive {
            let entry = entry.map_err(|e| anyhow::anyhow!("unrar entry error: {}", e))?;
            let name = entry.filename.to_string_lossy().to_string();
            if is_image_name(&name.to_ascii_lowercase()) {
                names.push(name);
            }
        }
        names
    };

    if image_names.is_empty() {
        return Err(anyhow::anyhow!("no images found in cbr: {}", path.display()));
    }

    image_names.sort_by(|a, b| natord::compare(a, b));
    let count = image_names.len() as i32;
    let first_name = image_names[0].clone();

    // Pass 2: extract first image to memory
    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| anyhow::anyhow!("unrar open for processing failed for {}: {}", path.display(), e))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        if entry_name == first_name {
            let (data, _) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read data: {}", e))?;
            return Ok((count, data));
        }
        archive = header
            .skip()
            .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
    }

    Err(anyhow::anyhow!(
        "could not find '{}' in {}",
        first_name,
        path.display()
    ))
}

fn analyze_pdf(path: &Path, pdf_render_scale: u32) -> Result<(i32, Vec<u8>)> {
    use pdfium_render::prelude::*;

    // Open PDF once — get page count and render first page in a single pass
    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library()
            .map_err(|e| anyhow::anyhow!("pdfium library not available: {:?}", e))?,
    );

    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| anyhow::anyhow!("pdfium load failed for {}: {:?}", path.display(), e))?;

    let count = document.pages().len() as i32;
    if count == 0 {
        return Err(anyhow::anyhow!("PDF has no pages: {}", path.display()));
    }

    let scale = if pdf_render_scale == 0 { 400 } else { pdf_render_scale } as i32;
    let config = PdfRenderConfig::new()
        .set_target_width(scale)
        .set_maximum_height(scale);

    let page = document
        .pages()
        .get(0)
        .map_err(|e| anyhow::anyhow!("cannot get first page of {}: {:?}", path.display(), e))?;

    let bitmap = page
        .render_with_config(&config)
        .map_err(|e| anyhow::anyhow!("pdfium render failed for {}: {:?}", path.display(), e))?;

    let image = bitmap.as_image();
    let mut buf = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut buf, image::ImageFormat::Png)
        .context("failed to encode rendered PDF page as PNG")?;

    Ok((count, buf.into_inner()))
}

fn parse_cbz_page_count(path: &Path) -> Result<i32> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    match zip::ZipArchive::new(file) {
        Ok(mut archive) => {
            let mut count: i32 = 0;
            for i in 0..archive.len() {
                let entry = archive.by_index(i).context("cannot read cbz entry")?;
                let name = entry.name().to_ascii_lowercase();
                if is_image_name(&name) {
                    count += 1;
                }
            }
            Ok(count)
        }
        Err(_) => {
            // Fallback: streaming count (bypasses extra field validation)
            parse_cbz_page_count_streaming(path)
        }
    }
}

fn parse_cbz_page_count_streaming(path: &Path) -> Result<i32> {
    let entries = raw_zip_list_entries(path)?;
    let count = entries
        .iter()
        .filter(|e| is_image_name(&e.name.to_ascii_lowercase()))
        .count() as i32;
    Ok(count)
}

fn parse_cbr_page_count(path: &Path) -> Result<i32> {
    let archive = match open_cbr_listing(path) {
        Ok(a) => a,
        Err(e_str) => {
            if is_not_rar_error(&e_str) {
                return parse_cbz_page_count(path);
            }
            return Err(anyhow::anyhow!("{}", e_str));
        }
    };
    let count = archive
        .filter(|r| {
            r.as_ref()
                .map(|e| is_image_name(&e.filename.to_string_lossy().to_ascii_lowercase()))
                .unwrap_or(false)
        })
        .count() as i32;
    Ok(count)
}

fn parse_pdf_page_count(path: &Path) -> Result<i32> {
    let doc = lopdf::Document::load(path)
        .with_context(|| format!("cannot open pdf: {}", path.display()))?;
    Ok(doc.get_pages().len() as i32)
}

pub fn is_image_name(name: &str) -> bool {
    // Skip macOS metadata entries (__MACOSX/ prefix or AppleDouble ._* files)
    if name.starts_with("__macosx/") || name.contains("/._") || name.starts_with("._") {
        return false;
    }
    name.ends_with(".jpg")
        || name.ends_with(".jpeg")
        || name.ends_with(".png")
        || name.ends_with(".webp")
        || name.ends_with(".avif")
        || name.ends_with(".gif")
        || name.ends_with(".bmp")
        || name.ends_with(".tif")
        || name.ends_with(".tiff")
}

/// Returns the sorted list of image entry names in a CBZ or CBR archive.
/// Intended to be cached by the caller; pass the result to `extract_image_by_name`.
pub fn list_archive_images(path: &Path, format: BookFormat) -> Result<Vec<String>> {
    match format {
        BookFormat::Cbz => list_cbz_images(path),
        BookFormat::Cbr => list_cbr_images(path),
        BookFormat::Pdf => Err(anyhow::anyhow!("list_archive_images not applicable for PDF")),
        BookFormat::Epub => get_epub_image_index(path),
    }
}

fn list_cbz_images(path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(zip_err) => {
            // Try RAR fallback
            if let Ok(names) = list_cbr_images(path) {
                return Ok(names);
            }
            // Try streaming fallback
            return list_cbz_images_streaming(path).map_err(|_| {
                anyhow::anyhow!("invalid cbz for {}: {}", path.display(), zip_err)
            });
        }
    };

    let mut names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let lower = entry.name().to_ascii_lowercase();
        if is_image_name(&lower) {
            names.push(entry.name().to_string());
        }
    }
    names.sort_by(|a, b| natord::compare(a, b));
    Ok(names)
}

fn list_cbz_images_streaming(path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz for streaming: {}", path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    let mut names: Vec<String> = Vec::new();

    loop {
        match zip::read::read_zipfile_from_stream(&mut reader) {
            Ok(Some(mut entry)) => {
                let name = entry.name().to_string();
                if is_image_name(&name.to_ascii_lowercase()) {
                    names.push(name);
                }
                std::io::copy(&mut entry, &mut std::io::sink())?;
            }
            Ok(None) => break,
            Err(_) => {
                if !names.is_empty() {
                    break;
                }
                return Err(anyhow::anyhow!(
                    "streaming ZIP listing failed for {}",
                    path.display()
                ));
            }
        }
    }
    names.sort_by(|a, b| natord::compare(a, b));
    Ok(names)
}

fn list_cbr_images(path: &Path) -> Result<Vec<String>> {
    let archive = match open_cbr_listing(path) {
        Ok(a) => a,
        Err(e_str) => {
            if is_not_rar_error(&e_str) {
                return list_cbz_images(path);
            }
            return Err(anyhow::anyhow!("{}", e_str));
        }
    };
    let mut names: Vec<String> = Vec::new();
    for entry in archive {
        let entry = entry.map_err(|e| anyhow::anyhow!("unrar entry error: {}", e))?;
        let name = entry.filename.to_string_lossy().to_string();
        if is_image_name(&name.to_ascii_lowercase()) {
            names.push(name);
        }
    }
    names.sort_by(|a, b| natord::compare(a, b));
    Ok(names)
}

/// Extract a specific image entry by name from a CBZ or CBR archive.
/// Use in combination with `list_archive_images` to avoid re-enumerating entries.
pub fn extract_image_by_name(path: &Path, format: BookFormat, image_name: &str) -> Result<Vec<u8>> {
    match format {
        BookFormat::Cbz => extract_cbz_by_name(path, image_name),
        BookFormat::Cbr => extract_cbr_by_name(path, image_name),
        BookFormat::Pdf => Err(anyhow::anyhow!("use extract_page for PDF")),
        BookFormat::Epub => extract_cbz_by_name(path, image_name),
    }
}

fn extract_cbz_by_name(path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return extract_cbz_by_name_streaming(path, image_name),
    };
    let mut entry = archive
        .by_name(image_name)
        .with_context(|| format!("entry '{}' not found in {}", image_name, path.display()))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

fn extract_cbz_by_name_streaming(path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz for streaming: {}", path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    loop {
        match zip::read::read_zipfile_from_stream(&mut reader) {
            Ok(Some(mut entry)) => {
                if entry.name() == image_name {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf)?;
                    return Ok(buf);
                }
                std::io::copy(&mut entry, &mut std::io::sink())?;
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    Err(anyhow::anyhow!(
        "entry '{}' not found in streaming cbz: {}",
        image_name,
        path.display()
    ))
}

fn extract_cbr_by_name(path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| {
            anyhow::anyhow!(
                "unrar open for processing failed for {}: {}",
                path.display(),
                e
            )
        })?;
    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        if entry_name == image_name {
            let (data, _) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read data: {}", e))?;
            return Ok(data);
        }
        archive = header
            .skip()
            .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
    }
    Err(anyhow::anyhow!(
        "entry '{}' not found in cbr: {}",
        image_name,
        path.display()
    ))
}

pub fn extract_first_page(path: &Path, format: BookFormat) -> Result<Vec<u8>> {
    extract_page(path, format, 1, 0)
}

/// Extract a specific page (1-based index) from a book archive.
/// `pdf_render_width`: max width for PDF rasterization; 0 means use default (1200).
pub fn extract_page(path: &Path, format: BookFormat, page_number: u32, pdf_render_width: u32) -> Result<Vec<u8>> {
    if page_number == 0 {
        return Err(anyhow::anyhow!("page index starts at 1"));
    }
    match format {
        BookFormat::Cbz => extract_cbz_page(path, page_number, true),
        BookFormat::Cbr => extract_cbr_page(path, page_number, true),
        BookFormat::Pdf => {
            let width = if pdf_render_width == 0 { 1200 } else { pdf_render_width };
            render_pdf_page_n(path, page_number, width)
        }
        BookFormat::Epub => extract_epub_page(path, page_number),
    }
}

/// Cache of sorted image names per archive path. Avoids re-listing and sorting on every page request.
static CBZ_INDEX_CACHE: OnceLock<Mutex<HashMap<PathBuf, Vec<String>>>> = OnceLock::new();

fn cbz_index_cache() -> &'static Mutex<HashMap<PathBuf, Vec<String>>> {
    CBZ_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Get sorted image names from cache, or list + sort + cache them.
fn get_cbz_image_index(path: &Path, archive: &mut zip::ZipArchive<std::fs::File>) -> Vec<String> {
    {
        let cache = cbz_index_cache().lock().unwrap();
        if let Some(names) = cache.get(path) {
            return names.clone();
        }
    }
    let mut image_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.name().to_ascii_lowercase();
        if is_image_name(&name) {
            image_names.push(entry.name().to_string());
        }
    }
    image_names.sort_by(|a, b| natord::compare(a, b));
    {
        let mut cache = cbz_index_cache().lock().unwrap();
        cache.insert(path.to_path_buf(), image_names.clone());
    }
    image_names
}

fn extract_cbz_page(path: &Path, page_number: u32, allow_fallback: bool) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let index = page_number as usize - 1;

    match zip::ZipArchive::new(file) {
        Ok(mut archive) => {
            let image_names = get_cbz_image_index(path, &mut archive);

            let selected = image_names
                .get(index)
                .with_context(|| format!("page {} out of range (total: {})", page_number, image_names.len()))?;

            let mut entry = archive.by_name(selected)
                .with_context(|| format!("cannot read page {}", selected))?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            Ok(buf)
        }
        Err(zip_err) => {
            if allow_fallback {
                // Try RAR fallback (file might be a RAR with .cbz extension)
                if let Ok(data) = extract_cbr_page(path, page_number, false) {
                    return Ok(data);
                }
                // Raw ZIP fallback (bypasses extra field validation)
                return extract_cbz_page_raw(path, page_number);
            }
            Err(anyhow::anyhow!("invalid cbz archive for {}: {}", path.display(), zip_err))
        }
    }
}

fn extract_cbz_page_raw(path: &Path, page_number: u32) -> Result<Vec<u8>> {
    let entries = raw_zip_list_entries(path)?;
    let mut image_entries: Vec<&RawZipEntry> = entries
        .iter()
        .filter(|e| is_image_name(&e.name.to_ascii_lowercase()))
        .collect();
    image_entries.sort_by(|a, b| natord::compare(&a.name, &b.name));

    let index = page_number as usize - 1;
    let entry = image_entries
        .get(index)
        .with_context(|| format!("page {} out of range (total: {})", page_number, image_entries.len()))?;

    raw_zip_read_entry(path, entry)
}

fn extract_cbr_page(path: &Path, page_number: u32, allow_fallback: bool) -> Result<Vec<u8>> {
    let index = page_number as usize - 1;

    let mut image_names: Vec<String> = {
        let archive = match open_cbr_listing(path) {
            Ok(a) => a,
            Err(e_str) => {
                if allow_fallback && is_not_rar_error(&e_str) {
                    return extract_cbz_page(path, page_number, false);
                }
                return Err(anyhow::anyhow!("{}", e_str));
            }
        };
        let mut names = Vec::new();
        for entry in archive {
            let entry = entry.map_err(|e| anyhow::anyhow!("unrar entry error: {}", e))?;
            let name = entry.filename.to_string_lossy().to_string();
            if is_image_name(&name.to_ascii_lowercase()) {
                names.push(name);
            }
        }
        names
    };

    image_names.sort_by(|a, b| natord::compare(a, b));
    let target = image_names
        .get(index)
        .with_context(|| format!("page {} out of range (total: {})", page_number, image_names.len()))?
        .clone();

    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| anyhow::anyhow!("unrar open for processing failed: {}", e))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        if entry_name == target {
            let (data, _) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read data: {}", e))?;
            return Ok(data);
        }
        archive = header
            .skip()
            .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
    }

    Err(anyhow::anyhow!("page '{}' not found in {}", target, path.display()))
}

fn render_pdf_page_n(path: &Path, page_number: u32, width: u32) -> Result<Vec<u8>> {
    use pdfium_render::prelude::*;

    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library()
            .map_err(|e| anyhow::anyhow!("pdfium library not available: {:?}", e))?,
    );

    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| anyhow::anyhow!("pdfium load failed for {}: {:?}", path.display(), e))?;

    let page_index = (page_number - 1) as u16;
    let page = document
        .pages()
        .get(page_index)
        .map_err(|_| anyhow::anyhow!("page {} out of range in {}", page_number, path.display()))?;

    let config = PdfRenderConfig::new().set_target_width(width as i32);

    let bitmap = page
        .render_with_config(&config)
        .map_err(|e| anyhow::anyhow!("pdfium render failed for {}: {:?}", path.display(), e))?;

    let image = bitmap.as_image();
    let mut buf = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut buf, image::ImageFormat::Png)
        .context("failed to encode rendered PDF page as PNG")?;

    Ok(buf.into_inner())
}


// ============================================================
// EPUB support — spine-aware image index with cache
// ============================================================

/// Cache of ordered image paths per EPUB file. Avoids re-parsing OPF/XHTML on every page request.
static EPUB_INDEX_CACHE: OnceLock<Mutex<HashMap<PathBuf, Vec<String>>>> = OnceLock::new();

fn epub_index_cache() -> &'static Mutex<HashMap<PathBuf, Vec<String>>> {
    EPUB_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

// Pre-compiled regex patterns for EPUB XML parsing (compiled once on first use)
static RE_EPUB_ROOTFILE: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ITEM: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ITEMREF: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_IMG_SRC: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_SVG_HREF: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ATTR_ID: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ATTR_HREF: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ATTR_MEDIA: OnceLock<regex::Regex> = OnceLock::new();

struct EpubManifestItem {
    href: String,
    media_type: String,
}

/// Build the ordered list of image paths for an EPUB file.
/// Walks the OPF spine to determine reading order, parses XHTML/SVG pages
/// for image references, and falls back to CBZ-style listing if no
/// images are found through the spine.
fn build_epub_image_index(path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open epub: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("invalid epub zip: {}", path.display()))?;

    // 1. Find OPF path from META-INF/container.xml
    let opf_path = {
        let mut entry = archive
            .by_name("META-INF/container.xml")
            .context("missing META-INF/container.xml — not a valid EPUB")?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let xml = String::from_utf8_lossy(&buf);
        let re = RE_EPUB_ROOTFILE.get_or_init(|| {
            regex::Regex::new(r#"<(?:\w+:)?rootfile[^>]+full-path="([^"]+)""#).unwrap()
        });
        re.captures(&xml)
            .and_then(|c| c.get(1))
            .map(|m| decode_xml_entities(m.as_str()))
            .context("no rootfile found in container.xml")?
    };

    let opf_dir = std::path::Path::new(&opf_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    // 2. Parse OPF manifest + spine
    let (manifest, spine_idrefs) = {
        let mut entry = archive
            .by_name(&opf_path)
            .with_context(|| format!("missing OPF file: {}", opf_path))?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let xml = String::from_utf8_lossy(&buf);
        parse_epub_opf(&xml, &opf_dir)?
    };

    // 3. Walk spine entries to build ordered image list
    let re_img = RE_EPUB_IMG_SRC.get_or_init(|| {
        regex::Regex::new(r#"(?i)<img\s[^>]*src=["']([^"']+)["']"#).unwrap()
    });
    let re_svg = RE_EPUB_SVG_HREF.get_or_init(|| {
        regex::Regex::new(r#"(?i)<image\s[^>]*(?:xlink:)?href=["']([^"']+)["']"#).unwrap()
    });

    let mut images: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for idref in &spine_idrefs {
        let item = match manifest.get(idref.as_str()) {
            Some(item) => item,
            None => continue,
        };

        // Direct raster image in spine (rare but possible)
        if item.media_type.starts_with("image/") && !item.media_type.contains("svg") {
            if seen.insert(item.href.clone()) {
                images.push(item.href.clone());
            }
            continue;
        }

        // Read XHTML/SVG content — entry is dropped at end of match arm, releasing archive borrow
        let content = match archive.by_name(&item.href) {
            Ok(mut entry) => {
                let mut buf = Vec::new();
                match entry.read_to_end(&mut buf) {
                    Ok(_) => String::from_utf8_lossy(&buf).to_string(),
                    Err(_) => continue,
                }
            }
            Err(_) => continue,
        };

        let content_dir = std::path::Path::new(&item.href)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // Extract <img src="..."> and <image [xlink:]href="...">
        for re in [re_img, re_svg] {
            for cap in re.captures_iter(&content) {
                if let Some(src) = cap.get(1) {
                    let src_str = src.as_str();
                    if src_str.starts_with("data:") {
                        continue;
                    }
                    let decoded = decode_xml_entities(&percent_decode_epub(src_str));
                    let resolved = resolve_epub_path(&content_dir, &decoded);
                    if seen.insert(resolved.clone()) {
                        images.push(resolved);
                    }
                }
            }
        }
    }

    // 4. Fallback: no images from spine → list all images in ZIP (CBZ-style)
    if images.is_empty() {
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                let name = entry.name().to_string();
                if is_image_name(&name.to_ascii_lowercase()) && seen.insert(name.clone()) {
                    images.push(name);
                }
            }
        }
        images.sort_by(|a, b| natord::compare(a, b));
    }

    if images.is_empty() {
        return Err(anyhow::anyhow!("no images found in epub: {}", path.display()));
    }

    Ok(images)
}

fn parse_epub_opf(
    xml: &str,
    opf_dir: &str,
) -> Result<(HashMap<String, EpubManifestItem>, Vec<String>)> {
    let re_item = RE_EPUB_ITEM.get_or_init(|| {
        regex::Regex::new(r#"(?s)<(?:\w+:)?item\s([^>]+?)/?>"#).unwrap()
    });
    let re_itemref = RE_EPUB_ITEMREF.get_or_init(|| {
        regex::Regex::new(r#"<(?:\w+:)?itemref\s[^>]*idref="([^"]+)""#).unwrap()
    });
    let re_id = RE_EPUB_ATTR_ID.get_or_init(|| {
        regex::Regex::new(r#"(?:^|\s)id="([^"]+)""#).unwrap()
    });
    let re_href = RE_EPUB_ATTR_HREF.get_or_init(|| {
        regex::Regex::new(r#"(?:^|\s)href="([^"]+)""#).unwrap()
    });
    let re_media = RE_EPUB_ATTR_MEDIA.get_or_init(|| {
        regex::Regex::new(r#"media-type="([^"]+)""#).unwrap()
    });

    let mut manifest: HashMap<String, EpubManifestItem> = HashMap::new();
    for cap in re_item.captures_iter(xml) {
        if let Some(attrs) = cap.get(1) {
            let a = attrs.as_str();
            let id = re_id.captures(a).and_then(|c| c.get(1));
            let href = re_href.captures(a).and_then(|c| c.get(1));
            let media = re_media.captures(a).and_then(|c| c.get(1));

            if let (Some(id), Some(href), Some(media)) = (id, href, media) {
                let decoded_href = decode_xml_entities(&percent_decode_epub(href.as_str()));
                let resolved = resolve_epub_path(opf_dir, &decoded_href);
                manifest.insert(
                    id.as_str().to_string(),
                    EpubManifestItem {
                        href: resolved,
                        media_type: media.as_str().to_string(),
                    },
                );
            }
        }
    }

    let spine_idrefs: Vec<String> = re_itemref
        .captures_iter(xml)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();

    Ok((manifest, spine_idrefs))
}

/// Get the cached image index for an EPUB, building it on first access.
fn get_epub_image_index(path: &Path) -> Result<Vec<String>> {
    {
        let cache = epub_index_cache().lock().unwrap();
        if let Some(names) = cache.get(path) {
            return Ok(names.clone());
        }
    }
    let images = build_epub_image_index(path)?;
    {
        let mut cache = epub_index_cache().lock().unwrap();
        cache.insert(path.to_path_buf(), images.clone());
    }
    Ok(images)
}

fn parse_epub_page_count(path: &Path) -> Result<i32> {
    let images = build_epub_image_index(path)?;
    Ok(images.len() as i32)
}

fn analyze_epub(path: &Path) -> Result<(i32, Vec<u8>)> {
    let images = get_epub_image_index(path)?;
    let count = images.len() as i32;

    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open epub: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;

    for img_path in &images {
        if let Ok(mut entry) = archive.by_name(img_path) {
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                return Ok((count, buf));
            }
        }
    }

    Err(anyhow::anyhow!(
        "no readable images in epub: {}",
        path.display()
    ))
}

fn extract_epub_page(path: &Path, page_number: u32) -> Result<Vec<u8>> {
    let images = get_epub_image_index(path)?;
    let index = page_number as usize - 1;
    let img_path = images
        .get(index)
        .with_context(|| {
            format!(
                "page {} out of range (total: {})",
                page_number,
                images.len()
            )
        })?;

    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open epub: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive
        .by_name(img_path)
        .with_context(|| format!("image '{}' not found in epub", img_path))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

// --- EPUB path/encoding helpers ---

fn resolve_epub_path(base_dir: &str, href: &str) -> String {
    if let Some(stripped) = href.strip_prefix('/') {
        return normalize_epub_path(stripped);
    }
    if base_dir.is_empty() {
        return normalize_epub_path(href);
    }
    normalize_epub_path(&format!("{}/{}", base_dir, href))
}

fn normalize_epub_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

fn percent_decode_epub(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (epub_hex_val(bytes[i + 1]), epub_hex_val(bytes[i + 2])) {
                result.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

fn epub_hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn decode_xml_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Convert a CBR file to CBZ in-place (same directory, same stem).
///
/// The conversion is safe: a `.cbz.tmp` file is written first, verified, then
/// atomically renamed to `.cbz`. The original CBR is **not** deleted by this
/// function — the caller is responsible for removing it after a successful DB update.
///
/// Returns the path of the newly created `.cbz` file.
pub fn convert_cbr_to_cbz(cbr_path: &Path) -> Result<PathBuf> {
    let parent = cbr_path
        .parent()
        .with_context(|| format!("no parent directory for {}", cbr_path.display()))?;
    let stem = cbr_path
        .file_stem()
        .with_context(|| format!("no file stem for {}", cbr_path.display()))?;

    let cbz_path = parent.join(format!("{}.cbz", stem.to_string_lossy()));
    let tmp_path = parent.join(format!("{}.cbz.tmp", stem.to_string_lossy()));

    if cbz_path.exists() {
        return Err(anyhow::anyhow!(
            "CBZ file already exists: {}",
            cbz_path.display()
        ));
    }

    // Extract all images from CBR into memory using unrar crate (no subprocess)
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();
    let mut archive = unrar::Archive::new(cbr_path)
        .open_for_processing()
        .map_err(|e| anyhow::anyhow!("unrar open failed for {}: {}", cbr_path.display(), e))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        let file_name = Path::new(&entry_name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| entry_name.clone());

        if is_image_name(&entry_name.to_ascii_lowercase()) {
            let (data, next) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read: {}", e))?;
            images.push((file_name, data));
            archive = next;
        } else {
            archive = header
                .skip()
                .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
        }
    }

    if images.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in CBR: {}",
            cbr_path.display()
        ));
    }

    images.sort_by(|(a, _), (b, _)| natord::compare(a, b));
    let image_count = images.len();

    // Pack images into the .cbz.tmp file
    let pack_result = (|| -> Result<()> {
        let cbz_file = std::fs::File::create(&tmp_path)
            .with_context(|| format!("cannot create {}", tmp_path.display()))?;
        let mut zip = zip::ZipWriter::new(cbz_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for (file_name, data) in &images {
            zip.start_file(file_name, options)
                .with_context(|| format!("cannot add file {} to zip", file_name))?;
            zip.write_all(data)
                .with_context(|| format!("cannot write {} to zip", file_name))?;
        }
        zip.finish().context("cannot finalize zip")?;
        Ok(())
    })();

    if let Err(err) = pack_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    // Verify the CBZ contains the expected number of images
    let verify_result = (|| -> Result<()> {
        let file = std::fs::File::open(&tmp_path)
            .with_context(|| format!("cannot open {}", tmp_path.display()))?;
        let archive = zip::ZipArchive::new(file).context("invalid zip archive")?;
        let packed_count = (0..archive.len())
            .filter(|&i| {
                archive
                    .name_for_index(i)
                    .map(|n| is_image_name(&n.to_ascii_lowercase()))
                    .unwrap_or(false)
            })
            .count();
        if packed_count != image_count {
            return Err(anyhow::anyhow!(
                "CBZ verification failed: expected {} images, found {}",
                image_count,
                packed_count
            ));
        }
        Ok(())
    })();

    if let Err(err) = verify_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    std::fs::rename(&tmp_path, &cbz_path)
        .with_context(|| format!("cannot rename {} to {}", tmp_path.display(), cbz_path.display()))?;

    Ok(cbz_path)
}

#[allow(dead_code)]
fn clean_title(filename: &str) -> String {
    let cleaned = regex::Regex::new(r"(?i)\s*T\d+\s*")
        .ok()
        .map(|re| re.replace_all(filename, " ").to_string())
        .unwrap_or_else(|| filename.to_string());

    let cleaned = regex::Regex::new(r"(?i)\s*Vol\.?\s*\d+\s*")
        .ok()
        .map(|re| re.replace_all(&cleaned, " ").to_string())
        .unwrap_or(cleaned);

    let cleaned = regex::Regex::new(r"(?i)\s*Volume\s*\d+\s*")
        .ok()
        .map(|re| re.replace_all(&cleaned, " ").to_string())
        .unwrap_or(cleaned);

    let cleaned = regex::Regex::new(r"#\d+")
        .ok()
        .map(|re| re.replace_all(&cleaned, " ").to_string())
        .unwrap_or(cleaned);

    let cleaned = regex::Regex::new(r"-\s*\d+\s*$")
        .ok()
        .map(|re| re.replace_all(&cleaned, " ").to_string())
        .unwrap_or(cleaned);

    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_not_rar_detects_not_rar_archive() {
        assert!(is_not_rar_error("unrar listing failed for /path/file.cbr: Not a RAR archive"));
    }

    #[test]
    fn is_not_rar_detects_bad_archive() {
        assert!(is_not_rar_error("unrar listing failed for /path/file.cbr: bad archive"));
    }

    #[test]
    fn is_not_rar_ignores_other_errors() {
        assert!(!is_not_rar_error("unrar listing failed for /path/file.cbr: file not found"));
        assert!(!is_not_rar_error("some other error"));
        assert!(!is_not_rar_error(""));
    }

    #[test]
    fn open_cbr_listing_nonexistent_file() {
        let result = open_cbr_listing(Path::new("/nonexistent/file.cbr"));
        assert!(result.is_err());
    }

    #[test]
    fn detect_format_cbz() {
        assert_eq!(detect_format(Path::new("test.cbz")), Some(BookFormat::Cbz));
        assert_eq!(detect_format(Path::new("test.CBZ")), Some(BookFormat::Cbz));
    }

    #[test]
    fn detect_format_cbr() {
        assert_eq!(detect_format(Path::new("test.cbr")), Some(BookFormat::Cbr));
    }

    #[test]
    fn detect_format_pdf() {
        assert_eq!(detect_format(Path::new("test.pdf")), Some(BookFormat::Pdf));
    }

    #[test]
    fn detect_format_epub() {
        assert_eq!(detect_format(Path::new("test.epub")), Some(BookFormat::Epub));
    }

    #[test]
    fn detect_format_unknown() {
        assert_eq!(detect_format(Path::new("test.txt")), None);
        assert_eq!(detect_format(Path::new("no_extension")), None);
    }

    #[test]
    fn is_image_name_accepts_valid() {
        assert!(is_image_name("page01.jpg"));
        assert!(is_image_name("cover.png"));
        assert!(is_image_name("image.webp"));
    }

    #[test]
    fn is_image_name_rejects_non_images() {
        assert!(!is_image_name("readme.txt"));
        assert!(!is_image_name("metadata.xml"));
        assert!(!is_image_name(""));
    }

    #[test]
    fn is_image_name_rejects_macos_metadata() {
        assert!(!is_image_name("__macosx/page01.jpg"));
        assert!(!is_image_name("folder/._cover.png"));
        assert!(!is_image_name("._hidden.jpg"));
    }

    #[test]
    fn is_image_name_accepts_all_formats() {
        assert!(is_image_name("page.jpeg"));
        assert!(is_image_name("page.avif"));
        assert!(is_image_name("page.gif"));
        assert!(is_image_name("page.bmp"));
        assert!(is_image_name("page.tif"));
        assert!(is_image_name("page.tiff"));
    }

    // ─── extract_volume ──────────────────────────────────────────────────

    #[test]
    fn extract_volume_t_prefix() {
        assert_eq!(extract_volume("One Piece T01"), Some(1));
        assert_eq!(extract_volume("Naruto T12"), Some(12));
        assert_eq!(extract_volume("Series T100"), Some(100));
    }

    #[test]
    fn extract_volume_tome_prefix() {
        assert_eq!(extract_volume("Naruto Tome 3"), Some(3));
        // "Tome" matches via T pattern first (T followed by digits after "ome")
        // but the T pattern matches "Tome" → the 'T' in "Tome" triggers T(\d+) only
        // if followed by digits directly. Let's verify actual behavior:
        assert_eq!(extract_volume("Asterix Tome 12"), Some(12));
    }

    #[test]
    fn extract_volume_vol_prefix() {
        assert_eq!(extract_volume("Vol.12"), Some(12));
        assert_eq!(extract_volume("Vol 5"), Some(5));
        assert_eq!(extract_volume("Volume 3"), Some(3));
        assert_eq!(extract_volume("Volume3"), Some(3));
    }

    #[test]
    fn extract_volume_hash_prefix() {
        assert_eq!(extract_volume("Issue #42"), Some(42));
        assert_eq!(extract_volume("#007"), Some(7));
    }

    #[test]
    fn extract_volume_trailing_dash_number() {
        assert_eq!(extract_volume("Series - 05"), Some(5));
    }

    #[test]
    fn extract_volume_zero_padded() {
        assert_eq!(extract_volume("T007"), Some(7));
        assert_eq!(extract_volume("T001"), Some(1));
    }

    #[test]
    fn extract_volume_no_match() {
        assert_eq!(extract_volume("Just a title"), None);
        assert_eq!(extract_volume("No numbers here"), None);
        assert_eq!(extract_volume(""), None);
    }

    // ─── extract_series ──────────────────────────────────────────────────

    #[test]
    fn extract_series_simple() {
        let path = Path::new("/libraries/manga/One Piece/T01.cbz");
        let root = Path::new("/libraries/manga");
        assert_eq!(extract_series(path, root), Some("One Piece".to_string()));
    }

    #[test]
    fn extract_series_nested() {
        let path = Path::new("/libraries/bd/Asterix/subfolder/file.cbz");
        let root = Path::new("/libraries/bd");
        // Should return the first directory component after root
        assert_eq!(extract_series(path, root), Some("Asterix".to_string()));
    }

    #[test]
    fn extract_series_file_at_root() {
        let path = Path::new("/libraries/manga/standalone.cbz");
        let root = Path::new("/libraries/manga");
        // File directly in root → no series directory
        assert_eq!(extract_series(path, root), None);
    }

    #[test]
    fn extract_series_unrelated_path() {
        let path = Path::new("/other/path/file.cbz");
        let root = Path::new("/libraries/manga");
        // Path doesn't start with root
        assert_eq!(extract_series(path, root), None);
    }

    // ─── parse_metadata_fast ─────────────────────────────────────────────

    #[test]
    fn parse_metadata_fast_extracts_all() {
        let path = Path::new("/libraries/manga/One Piece/One Piece T05.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.title, "One Piece T05");
        assert_eq!(meta.series, Some("One Piece".to_string()));
        assert_eq!(meta.volume, Some(5));
        assert_eq!(meta.page_count, None);
    }

    #[test]
    fn parse_metadata_fast_no_volume() {
        let path = Path::new("/libraries/bd/Asterix/Asterix le Gaulois.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.title, "Asterix le Gaulois");
        assert_eq!(meta.series, Some("Asterix".to_string()));
        assert_eq!(meta.volume, None);
    }

    #[test]
    fn parse_metadata_fast_no_extension() {
        let path = Path::new("/libraries/manga/Series/Untitled");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.title, "Untitled");
        assert_eq!(meta.series, Some("Series".to_string()));
    }

    // ─── detect_format ───────────────────────────────────────────────────

    #[test]
    fn detect_format_case_insensitive() {
        assert_eq!(detect_format(Path::new("test.PDF")), Some(BookFormat::Pdf));
        assert_eq!(detect_format(Path::new("test.Epub")), Some(BookFormat::Epub));
        assert_eq!(detect_format(Path::new("test.CbR")), Some(BookFormat::Cbr));
    }

    // ─── BookFormat::as_str ──────────────────────────────────────────────

    #[test]
    fn book_format_as_str() {
        assert_eq!(BookFormat::Cbz.as_str(), "cbz");
        assert_eq!(BookFormat::Cbr.as_str(), "cbr");
        assert_eq!(BookFormat::Pdf.as_str(), "pdf");
        assert_eq!(BookFormat::Epub.as_str(), "epub");
    }

    // ─── clean_title ─────────────────────────────────────────────────────

    #[test]
    fn clean_title_removes_volume_patterns() {
        assert_eq!(clean_title("One Piece T05"), "One Piece");
        assert_eq!(clean_title("Naruto Vol.12"), "Naruto");
        assert_eq!(clean_title("Series Volume 3"), "Series");
        assert_eq!(clean_title("Issue #42"), "Issue");
        assert_eq!(clean_title("Series - 05"), "Series");
    }

    #[test]
    fn clean_title_no_volume() {
        assert_eq!(clean_title("Just a title"), "Just a title");
    }
}
