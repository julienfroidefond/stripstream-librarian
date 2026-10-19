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
        BookFormat::Pdf => {
            let (page_count, first_page) = analyze_pdf(path, pdf_render_scale)?;
            let first_page = first_page.ok_or_else(|| {
                anyhow::anyhow!(
                    "PDF first page could not be rendered (page count is {})",
                    page_count
                )
            })?;
            Ok((page_count, first_page))
        }
        BookFormat::Epub => analyze_epub(path),
    }
}

/// Opens a book only to count its pages, without extracting any image.
///
/// Used as a recovery path when `analyze_book` fails: a book whose first page
/// cannot be rendered (damaged page, rendering timeout, ...) still gets a
/// correct `page_count` and stays readable in the UI.
pub fn page_count_only(path: &Path, format: BookFormat) -> Result<i32> {
    match format {
        BookFormat::Cbz => crate::zip::parse_cbz_page_count(path),
        BookFormat::Cbr => crate::cbr::parse_cbr_page_count(path),
        BookFormat::Pdf => crate::pdf::pdf_page_count(path),
        BookFormat::Epub => crate::epub::parse_epub_page_count(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Minimal valid JPEG (SOI/EOI only): enough to be recognised as an image by
    /// `is_image_name`; nothing here is ever decoded.
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xD9];

    fn write_cbz(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).expect("create cbz");
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            zip.start_file(*name, opts).expect("start entry");
            zip.write_all(data).expect("write entry");
        }
        zip.finish().expect("finish cbz");
    }

    fn write_epub(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).expect("create epub");
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            zip.start_file(*name, opts).expect("start entry");
            zip.write_all(data).expect("write entry");
        }
        zip.finish().expect("finish epub");
    }

    /// Two-image EPUB whose spine lists both pages in order.
    fn epub_with_two_images() -> Vec<(&'static str, &'static [u8])> {
        vec![
            (
                "META-INF/container.xml",
                br#"<container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#,
            ),
            (
                "OEBPS/content.opf",
                br#"<package><manifest>
                    <item id="p1" href="page1.xhtml" media-type="application/xhtml+xml"/>
                    <item id="p2" href="page2.xhtml" media-type="application/xhtml+xml"/>
                </manifest><spine><itemref idref="p1"/><itemref idref="p2"/></spine></package>"#,
            ),
            (
                "OEBPS/page1.xhtml",
                br#"<html><body><img src="img/001.jpg"/></body></html>"#,
            ),
            (
                "OEBPS/page2.xhtml",
                br#"<html><body><img src="img/002.jpg"/></body></html>"#,
            ),
            ("OEBPS/img/001.jpg", JPEG),
            ("OEBPS/img/002.jpg", JPEG),
        ]
    }

    #[test]
    fn parse_metadata_fills_cbz_page_count_and_series() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("bd");
        std::fs::create_dir_all(root.join("One Piece")).unwrap();
        let path = root.join("One Piece").join("T01.cbz");
        write_cbz(&path, &[("001.jpg", JPEG), ("002.jpg", JPEG)]);

        let meta = parse_metadata(&path, BookFormat::Cbz, &root).unwrap();

        assert_eq!(meta.title, "T01");
        assert_eq!(meta.series, Some("One Piece".to_string()));
        assert_eq!(meta.volume, Some(1));
        assert_eq!(meta.volume_type, VolumeType::Regular);
        assert_eq!(meta.page_count, Some(2));
    }

    #[test]
    fn parse_metadata_fills_epub_page_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.epub");
        write_epub(&path, &epub_with_two_images());

        let meta = parse_metadata(&path, BookFormat::Epub, dir.path()).unwrap();

        assert_eq!(meta.page_count, Some(2));
    }

    #[test]
    fn parse_metadata_keeps_file_metadata_when_the_archive_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("bd");
        // The file is not on disk: the fast (filename-only) metadata must survive
        // and `page_count` stays `None` instead of failing the whole call.
        let path = root.join("Broken").join("T03.cbz");

        let meta = parse_metadata(&path, BookFormat::Cbz, &root).unwrap();

        assert_eq!(meta.title, "T03");
        assert_eq!(meta.series, Some("Broken".to_string()));
        assert_eq!(meta.volume, Some(3));
        assert_eq!(meta.page_count, None);
    }

    #[test]
    fn parse_metadata_reports_zero_pages_for_a_corrupt_archive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("T03.cbz");
        // A file that is not a ZIP at all: the streaming counter cannot find any
        // image, so the book is reported with 0 pages. That is the signal the
        // diagnostics integrity check (`books_zero_pages`) surfaces to admins.
        std::fs::write(&path, b"garbage, not an archive").unwrap();

        let meta = parse_metadata(&path, BookFormat::Cbz, dir.path()).unwrap();

        assert_eq!(meta.page_count, Some(0));
    }

    #[test]
    fn analyze_book_returns_page_count_and_first_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbz");
        write_cbz(&path, &[("001.jpg", JPEG), ("002.jpg", b"second")]);

        let (count, first) = analyze_book(&path, BookFormat::Cbz, 400).unwrap();

        assert_eq!(count, 2);
        assert_eq!(first, JPEG);
    }

    #[test]
    fn analyze_book_errors_on_a_cbz_without_images() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.cbz");
        write_cbz(&path, &[("readme.txt", b"nothing to see")]);

        let err = analyze_book(&path, BookFormat::Cbz, 400).unwrap_err();
        assert!(
            err.to_string().contains("no images found"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn page_count_only_dispatches_per_format() {
        let dir = tempfile::tempdir().unwrap();

        let cbz = dir.path().join("book.cbz");
        write_cbz(&cbz, &[("001.jpg", JPEG), ("002.jpg", JPEG)]);
        assert_eq!(page_count_only(&cbz, BookFormat::Cbz).unwrap(), 2);

        let epub = dir.path().join("book.epub");
        write_epub(&epub, &epub_with_two_images());
        assert_eq!(page_count_only(&epub, BookFormat::Epub).unwrap(), 2);

        assert!(page_count_only(&dir.path().join("missing.cbz"), BookFormat::Cbz).is_err());
    }
}
