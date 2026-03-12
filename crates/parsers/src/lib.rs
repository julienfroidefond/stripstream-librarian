use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
        BookFormat::Cbz => analyze_cbz(path),
        BookFormat::Cbr => analyze_cbr(path),
        BookFormat::Pdf => analyze_pdf(path, pdf_render_scale),
    }
}

fn analyze_cbz(path: &Path) -> Result<(i32, Vec<u8>)> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            // Some .cbz files are actually RAR archives with the wrong extension — fallback to CBR parser
            return analyze_cbr(path).map_err(|rar_err| {
                anyhow::anyhow!(
                    "invalid cbz archive and RAR fallback also failed for {}: ZIP={}, RAR={}",
                    path.display(),
                    e,
                    rar_err
                )
            });
        }
    };

    let mut image_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).context("cannot read cbz entry")?;
        let name = entry.name().to_ascii_lowercase();
        if is_image_name(&name) {
            image_names.push(entry.name().to_string());
        }
    }
    image_names.sort_by(|a, b| natord::compare(a, b));

    let count = image_names.len() as i32;
    let first_image = image_names.first().context("no images found in cbz")?;

    let mut entry = archive
        .by_name(first_image)
        .context("cannot read first image")?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;

    Ok((count, buf))
}

fn analyze_cbr(path: &Path) -> Result<(i32, Vec<u8>)> {
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
                if e_str.contains("Not a RAR archive") || e_str.contains("bad archive") {
                    return analyze_cbz(path).map_err(|zip_err| {
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
    let mut archive = zip::ZipArchive::new(file).context("invalid cbz archive")?;
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

fn is_image_name(name: &str) -> bool {
    // Skip macOS metadata entries (__MACOSX/ prefix or AppleDouble ._* files)
    if name.starts_with("__macosx/") || name.contains("/._") || name.starts_with("._") {
        return false;
    }
    name.ends_with(".jpg")
        || name.ends_with(".jpeg")
        || name.ends_with(".png")
        || name.ends_with(".webp")
        || name.ends_with(".avif")
}

pub fn extract_first_page(path: &Path, format: BookFormat) -> Result<Vec<u8>> {
    match format {
        BookFormat::Cbz => extract_cbz_first_page(path),
        BookFormat::Cbr => analyze_cbr(path).map(|(_, bytes)| bytes),
        BookFormat::Pdf => analyze_pdf(path, 0).map(|(_, bytes)| bytes),
    }
}

fn extract_cbz_first_page(path: &Path) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("invalid cbz archive")?;

    let mut image_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).context("cannot read cbz entry")?;
        let name = entry.name().to_ascii_lowercase();
        if is_image_name(&name) {
            image_names.push(entry.name().to_string());
        }
    }
    image_names.sort_by(|a, b| natord::compare(a, b));

    let first_image = image_names.first().context("no images found in cbz")?;

    let mut entry = archive
        .by_name(first_image)
        .context("cannot read first image")?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
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
