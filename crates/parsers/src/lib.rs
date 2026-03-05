use anyhow::{Context, Result};
use std::path::Path;

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

pub fn parse_metadata(
    path: &Path,
    format: BookFormat,
    library_root: &Path,
) -> Result<ParsedMetadata> {
    let filename = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    // Extract volume from filename (patterns: T01, T02, Vol 1, Volume 1, #1, - 01, etc.)
    let volume = extract_volume(&filename);

    // Keep original filename as title (don't clean it)
    let title = filename;

    // Determine series from parent folder relative to library root
    let series = path.parent().and_then(|parent| {
        // Get the relative path from library root to parent
        let relative = parent.strip_prefix(library_root).ok()?;
        // If relative path is not empty, use first component as series
        let first_component = relative.components().next()?;
        let series_name = first_component.as_os_str().to_string_lossy().to_string();
        // Only if series_name is not empty
        if series_name.is_empty() {
            None
        } else {
            Some(series_name)
        }
    });

    let page_count = match format {
        BookFormat::Cbz => parse_cbz_page_count(path).ok(),
        BookFormat::Cbr => parse_cbr_page_count(path).ok(),
        BookFormat::Pdf => parse_pdf_page_count(path).ok(),
    };

    Ok(ParsedMetadata {
        title,
        series,
        volume,
        page_count,
    })
}

fn extract_volume(filename: &str) -> Option<i32> {
    // Common volume patterns: T01, T02, T1, T2, Vol 1, Vol. 1, Volume 1, #1, #01, - 1, - 01
    let patterns = [
        // T01, T02 pattern (most common for manga/comics)
        (r"(?i)T(\d+)", 1),
        // Vol 1, Vol. 1, Volume 1
        (r"(?i)Vol\.?\s*(\d+)", 1),
        (r"(?i)Volume\s*(\d+)", 1),
        // #1, #01
        (r"#(\d+)", 1),
        // - 1, - 01 at the end
        (r"-\s*(\d+)\s*$", 1),
    ];

    for (pattern, group) in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(filename) {
                if let Some(mat) = caps.get(*group) {
                    // Parse as integer to remove leading zeros
                    return mat.as_str().parse::<i32>().ok();
                }
            }
        }
    }

    None
}

#[allow(dead_code)]
fn clean_title(filename: &str) -> String {
    // Remove volume patterns from title to clean it up
    let cleaned = regex::Regex::new(r"(?i)\s*T\d+\s*")
        .ok()
        .and_then(|re| Some(re.replace_all(filename, " ").to_string()))
        .unwrap_or_else(|| filename.to_string());

    let cleaned = regex::Regex::new(r"(?i)\s*Vol\.?\s*\d+\s*")
        .ok()
        .and_then(|re| Some(re.replace_all(&cleaned, " ").to_string()))
        .unwrap_or_else(|| cleaned);

    let cleaned = regex::Regex::new(r"(?i)\s*Volume\s*\d+\s*")
        .ok()
        .and_then(|re| Some(re.replace_all(&cleaned, " ").to_string()))
        .unwrap_or_else(|| cleaned);

    let cleaned = regex::Regex::new(r"#\d+")
        .ok()
        .and_then(|re| Some(re.replace_all(&cleaned, " ").to_string()))
        .unwrap_or_else(|| cleaned);

    let cleaned = regex::Regex::new(r"-\s*\d+\s*$")
        .ok()
        .and_then(|re| Some(re.replace_all(&cleaned, " ").to_string()))
        .unwrap_or_else(|| cleaned);

    // Clean up extra spaces
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
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
    let output = std::process::Command::new("unrar")
        .arg("lb")
        .arg(path)
        .output()
        .with_context(|| format!("failed to execute unrar for {}", path.display()))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("unrar failed for {}", path.display()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let count = stdout
        .lines()
        .filter(|line| is_image_name(&line.to_ascii_lowercase()))
        .count() as i32;
    Ok(count)
}

fn parse_pdf_page_count(path: &Path) -> Result<i32> {
    let doc = lopdf::Document::load(path)
        .with_context(|| format!("cannot open pdf: {}", path.display()))?;
    Ok(doc.get_pages().len() as i32)
}

fn is_image_name(name: &str) -> bool {
    name.ends_with(".jpg")
        || name.ends_with(".jpeg")
        || name.ends_with(".png")
        || name.ends_with(".webp")
        || name.ends_with(".avif")
}
