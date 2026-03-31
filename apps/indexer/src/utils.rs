use parsers::BookFormat;
use std::path::Path;

pub use stripstream_core::fingerprint::compute_fingerprint;
pub use stripstream_core::paths::remap_libraries_path;
pub use stripstream_core::paths::unmap_libraries_path;

pub fn kind_from_format(format: BookFormat) -> &'static str {
    match format {
        BookFormat::Pdf | BookFormat::Epub => "ebook",
        BookFormat::Cbz | BookFormat::Cbr => "comic",
    }
}

pub fn file_display_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string())
}
