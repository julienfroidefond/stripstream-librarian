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
}

impl BookFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Pdf => "pdf",
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
        _ => None,
    }
}

// Cache compiled regex patterns — compiled once on first use
static VOLUME_PATTERNS: OnceLock<Vec<(regex::Regex, usize)>> = OnceLock::new();

fn get_volume_patterns() -> &'static Vec<(regex::Regex, usize)> {
    VOLUME_PATTERNS.get_or_init(|| {
        [
            // T01, T02 pattern (most common for manga/comics)
            (r"(?i)T(\d+)", 1usize),
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
            eprintln!(
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

fn analyze_cbr(path: &Path, allow_fallback: bool) -> Result<(i32, Vec<u8>)> {
    // Pass 1: list all image names via unrar (in-process, no subprocess)
    let mut image_names: Vec<String> = {
        let archive = unrar::Archive::new(path)
            .open_for_listing()
            .map_err(|e| anyhow::anyhow!("unrar listing failed for {}: {}", path.display(), e));
        // Some .cbr files are actually ZIP archives with wrong extension — fallback to CBZ parser
        let archive = match archive {
            Ok(a) => a,
            Err(e) => {
                let e_str = e.to_string();
                if allow_fallback && (e_str.contains("Not a RAR archive") || e_str.contains("bad archive")) {
                    return analyze_cbz(path, false).map_err(|zip_err| {
                        anyhow::anyhow!(
                            "not a RAR archive and ZIP fallback also failed for {}: RAR={}, ZIP={}",
                            path.display(),
                            e_str,
                            zip_err
                        )
                    });
                }
                return Err(e);
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
    let archive = unrar::Archive::new(path)
        .open_for_listing()
        .map_err(|e| anyhow::anyhow!("unrar listing failed for {}: {}", path.display(), e));
    // Some .cbr files are actually ZIP archives with wrong extension — fallback to CBZ parser
    let archive = match archive {
        Ok(a) => a,
        Err(e) => {
            let e_str = e.to_string();
            if e_str.contains("Not a RAR archive") || e_str.contains("bad archive") {
                return parse_cbz_page_count(path);
            }
            return Err(e);
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
    let archive = unrar::Archive::new(path)
        .open_for_listing()
        .map_err(|e| anyhow::anyhow!("unrar listing failed for {}: {}", path.display(), e));
    let archive = match archive {
        Ok(a) => a,
        Err(e) => {
            let e_str = e.to_string();
            if e_str.contains("Not a RAR archive") || e_str.contains("bad archive") {
                return list_cbz_images(path);
            }
            return Err(e);
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
        let archive = match unrar::Archive::new(path).open_for_listing() {
            Ok(a) => a,
            Err(e) => {
                if allow_fallback {
                    return extract_cbz_page(path, page_number, false);
                }
                return Err(anyhow::anyhow!("unrar listing failed for {}: {}", path.display(), e));
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
