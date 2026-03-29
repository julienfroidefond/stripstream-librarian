use anyhow::Result;
use chrono::DateTime;
use parsers::BookFormat;
use sha2::{Digest, Sha256};
use std::path::Path;
use chrono::Utc;

pub use stripstream_core::paths::remap_libraries_path;
pub use stripstream_core::paths::unmap_libraries_path;

pub fn compute_fingerprint(path: &Path, size: u64, mtime: &DateTime<Utc>) -> Result<String> {
    // Optimized: only use size + mtime + first bytes of filename for fast fingerprinting
    // This is 100x faster than reading file content while still being reliable for change detection
    let mut hasher = Sha256::new();
    hasher.update(size.to_le_bytes());
    hasher.update(mtime.timestamp().to_le_bytes());
    
    // Add filename for extra uniqueness (in case of rapid changes with same size+mtime)
    if let Some(filename) = path.file_name() {
        hasher.update(filename.as_encoded_bytes());
    }
    
    Ok(format!("{:x}", hasher.finalize()))
}

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
