use std::path::Path;

use anyhow::Result;

use crate::cbr::{analyze_cbr, parse_cbr_page_count};
use crate::epub::{analyze_epub, parse_epub_page_count};
use crate::pdf::{analyze_pdf, parse_pdf_page_count};
use crate::volume::{
    extract_hs_info, extract_int_info, extract_series, extract_volume, is_oneshot_folder,
};
use crate::zip::{analyze_cbz, parse_cbz_page_count};
use crate::{BookFormat, ParsedMetadata, VolumeType};

pub fn parse_metadata_fast(
    path: &Path,
    _format: BookFormat,
    library_root: &Path,
) -> ParsedMetadata {
    let filename = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    // Detect oneshot folder: file is exactly one level below library root in a known oneshot dir.
    let in_oneshot_folder = path
        .parent()
        .and_then(|p| p.strip_prefix(library_root).ok())
        .map(|rel| {
            let mut comps = rel.components();
            match (comps.next(), comps.next()) {
                (Some(first), None) => is_oneshot_folder(&first.as_os_str().to_string_lossy()),
                _ => false,
            }
        })
        .unwrap_or(false);

    if in_oneshot_folder {
        return ParsedMetadata {
            title: filename.clone(),
            series: Some(filename),
            volume: None,
            volume_type: VolumeType::Oneshot,
            page_count: None,
        };
    }

    // Check for INT patterns first (before HS, since "INTHS" contains "HS")
    let (volume, volume_type) = if let Some((int_number, _)) = extract_int_info(&filename) {
        (int_number, VolumeType::Integral)
    } else if let Some((hs_number, _)) = extract_hs_info(&filename) {
        (hs_number, VolumeType::Hs)
    } else {
        (extract_volume(&filename), VolumeType::Regular)
    };

    let title = filename;
    let series = extract_series(path, library_root);

    ParsedMetadata {
        title,
        series,
        volume,
        volume_type,
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
pub fn analyze_book(
    path: &Path,
    format: BookFormat,
    pdf_render_scale: u32,
) -> Result<(i32, Vec<u8>)> {
    match format {
        BookFormat::Cbz => analyze_cbz(path, true),
        BookFormat::Cbr => analyze_cbr(path, true),
        BookFormat::Pdf => analyze_pdf(path, pdf_render_scale),
        BookFormat::Epub => analyze_epub(path),
    }
}
