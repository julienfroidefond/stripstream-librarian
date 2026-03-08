use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;
use std::process::Command;
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
        // Normalize paths for comparison (handle different separators, etc.)
        let parent_str = parent.to_string_lossy().to_string();
        let root_str = library_root.to_string_lossy().to_string();

        // Try to find the library root in the parent path
        let relative = if let Some(idx) = parent_str.find(&root_str) {
            // Found root in parent, extract what comes after
            let after_root = &parent_str[idx + root_str.len()..];
            Path::new(after_root)
        } else if let Some(relative) = parent.strip_prefix(library_root).ok() {
            // Standard approach works
            relative
        } else {
            // Log for diagnostic on server
            eprintln!(
                "[PARSER] Cannot determine series: parent '{}' doesn't start with root '{}'",
                parent.display(),
                library_root.display()
            );
            return None;
        };

        // Remove leading separators
        let relative_str = relative.to_string_lossy().to_string();
        let relative_clean = relative_str.trim_start_matches(|c| c == '/' || c == '\\');

        if relative_clean.is_empty() {
            return None;
        }

        // Get first component as series
        let first_sep = relative_clean.find(|c| c == '/' || c == '\\');
        let series_name = match first_sep {
            Some(idx) => &relative_clean[..idx],
            None => relative_clean,
        };

        if series_name.is_empty() {
            None
        } else {
            Some(series_name.to_string())
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
    // Use pdfinfo command line tool instead of lopdf for better performance
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

    // Use env command like the API does
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

    // Use WalkDir for recursive search (CBR can have subdirectories)
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
