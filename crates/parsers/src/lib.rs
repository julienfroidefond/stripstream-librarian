use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use uuid::Uuid;
use walkdir::WalkDir;

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
/// This is more efficient than calling parse_metadata + extract_first_page separately.
pub fn analyze_book(path: &Path, format: BookFormat) -> Result<(i32, Vec<u8>)> {
    match format {
        BookFormat::Cbz => analyze_cbz(path),
        BookFormat::Cbr => analyze_cbr(path),
        BookFormat::Pdf => analyze_pdf(path),
    }
}

fn analyze_cbz(path: &Path) -> Result<(i32, Vec<u8>)> {
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
    image_names.sort();

    let count = image_names.len() as i32;
    let first_image = image_names.first().context("no images found in cbz")?;

    let mut entry = archive
        .by_name(first_image)
        .context("cannot read first image")?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;

    Ok((count, buf))
}

fn list_cbr_images(path: &Path) -> Result<Vec<String>> {
    // Try unrar lb first (fast)
    let output = std::process::Command::new("unrar")
        .arg("lb")
        .arg(path)
        .output()
        .with_context(|| format!("failed to execute unrar lb for {}", path.display()))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let images: Vec<String> = stdout
            .lines()
            .filter(|line| is_image_name(&line.to_ascii_lowercase()))
            .map(|l| l.to_string())
            .collect();
        if !images.is_empty() {
            return Ok(images);
        }
    }

    // Fallback: lsar (from unar package) handles UTF-16BE encoded filenames
    let lsar_output = std::process::Command::new("lsar")
        .arg(path)
        .output()
        .with_context(|| format!("failed to execute lsar for {}", path.display()))?;

    if !lsar_output.status.success() {
        return Err(anyhow::anyhow!(
            "both unrar lb and lsar failed for {}",
            path.display()
        ));
    }

    let stdout = String::from_utf8_lossy(&lsar_output.stdout);
    // lsar output: first line is archive info, then one file per line (indented)
    let images: Vec<String> = stdout
        .lines()
        .skip(1) // skip the archive header line
        .map(|l| l.trim().to_string())
        .filter(|line| is_image_name(&line.to_ascii_lowercase()))
        .collect();

    Ok(images)
}

fn analyze_cbr(path: &Path) -> Result<(i32, Vec<u8>)> {
    let mut image_names = list_cbr_images(path)?;
    image_names.sort();

    let count = image_names.len() as i32;
    if count == 0 {
        return Err(anyhow::anyhow!("no images found in cbr: {}", path.display()));
    }

    let first_name = &image_names[0];

    // Try unrar p to extract first image to stdout (faster — no temp dir)
    let p_output = std::process::Command::new("unrar")
        .args(["p", "-inul"])
        .arg(path)
        .arg(first_name)
        .output();

    match p_output {
        Ok(out) if out.status.success() && looks_like_image(&out.stdout) => Ok((count, out.stdout)),
        _ => {
            // Fallback: full extraction with unar (handles special chars, encoding issues)
            let image_bytes = extract_cbr_first_page(path)?;
            Ok((count, image_bytes))
        }
    }
}

/// Check image magic bytes to validate that bytes are a real image before decoding.
fn looks_like_image(bytes: &[u8]) -> bool {
    if bytes.len() < 12 {
        return false;
    }
    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return true;
    }
    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return true;
    }
    // WebP: RIFF....WEBP
    if &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return true;
    }
    false
}

fn analyze_pdf(path: &Path) -> Result<(i32, Vec<u8>)> {
    let count = parse_pdf_page_count(path)?;
    let image_bytes = extract_pdf_first_page(path)?;
    Ok((count, image_bytes))
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
    let images = list_cbr_images(path)?;
    Ok(images.len() as i32)
}

fn parse_pdf_page_count(path: &Path) -> Result<i32> {
    let output = std::process::Command::new("pdfinfo")
        .arg(path)
        .output()
        .with_context(|| format!("failed to execute pdfinfo for {}", path.display()))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("pdfinfo failed for {}", path.display()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.starts_with("Pages:") {
            if let Some(pages_str) = line.split_whitespace().nth(1) {
                return pages_str
                    .parse::<i32>()
                    .with_context(|| format!("cannot parse page count: {}", pages_str));
            }
        }
    }

    Err(anyhow::anyhow!(
        "could not find page count in pdfinfo output"
    ))
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
        BookFormat::Cbr => extract_cbr_first_page(path),
        BookFormat::Pdf => extract_pdf_first_page(path),
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
    image_names.sort();

    let first_image = image_names.first().context("no images found in cbz")?;

    let mut entry = archive
        .by_name(first_image)
        .context("cannot read first image")?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

fn extract_cbr_first_page(path: &Path) -> Result<Vec<u8>> {
    let tmp_dir = std::env::temp_dir().join(format!("stripstream-cbr-thumb-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_dir).context("cannot create temp dir")?;

    let output = std::process::Command::new("env")
        .args(["LC_ALL=en_US.UTF-8", "LANG=en_US.UTF-8", "unar", "-o"])
        .arg(&tmp_dir)
        .arg(path)
        .output()
        .context("unar failed")?;

    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(anyhow::anyhow!(
            "unar extract failed: {:?}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let mut image_files: Vec<_> = WalkDir::new(&tmp_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            is_image_name(&name)
        })
        .collect();

    image_files.sort_by_key(|e| e.path().to_string_lossy().to_lowercase());

    let first_image = image_files.first().context("no images found in cbr")?;

    let data = std::fs::read(first_image.path())?;
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(data)
}

fn extract_pdf_first_page(path: &Path) -> Result<Vec<u8>> {
    let tmp_dir = std::env::temp_dir().join(format!("stripstream-pdf-thumb-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_dir)?;
    let output_prefix = tmp_dir.join("page");

    let output = Command::new("pdftoppm")
        .args([
            "-f",
            "1",
            "-singlefile",
            "-png",
            "-scale-to",
            "800",
            path.to_str().unwrap(),
            output_prefix.to_str().unwrap(),
        ])
        .output()
        .context("pdftoppm failed")?;

    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(anyhow::anyhow!("pdftoppm failed"));
    }

    let image_path = output_prefix.with_extension("png");
    let data = std::fs::read(&image_path)?;
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(data)
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
