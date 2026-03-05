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

pub fn parse_metadata(path: &Path, format: BookFormat) -> Result<ParsedMetadata> {
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    let page_count = match format {
        BookFormat::Cbz => parse_cbz_page_count(path).ok(),
        BookFormat::Cbr => parse_cbr_page_count(path).ok(),
        BookFormat::Pdf => parse_pdf_page_count(path).ok(),
    };

    Ok(ParsedMetadata { title, page_count })
}

fn parse_cbz_page_count(path: &Path) -> Result<i32> {
    let file = std::fs::File::open(path).with_context(|| format!("cannot open cbz: {}", path.display()))?;
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
    let doc = lopdf::Document::load(path).with_context(|| format!("cannot open pdf: {}", path.display()))?;
    Ok(doc.get_pages().len() as i32)
}

fn is_image_name(name: &str) -> bool {
    name.ends_with(".jpg")
        || name.ends_with(".jpeg")
        || name.ends_with(".png")
        || name.ends_with(".webp")
        || name.ends_with(".avif")
}
