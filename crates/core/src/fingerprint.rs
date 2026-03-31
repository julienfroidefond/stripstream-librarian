use anyhow::Result;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Compute a fast fingerprint based on file size, mtime, and filename.
/// Used for change detection during scanning. 100x faster than content hashing.
pub fn compute_fingerprint(path: &Path, size: u64, mtime: &DateTime<Utc>) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(size.to_le_bytes());
    hasher.update(mtime.timestamp().to_le_bytes());

    if let Some(filename) = path.file_name() {
        hasher.update(filename.as_encoded_bytes());
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn deterministic_output() {
        let path = Path::new("/libraries/BD/book.cbz");
        let mtime = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
        let fp1 = compute_fingerprint(path, 1024, &mtime).unwrap();
        let fp2 = compute_fingerprint(path, 1024, &mtime).unwrap();
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn different_size_produces_different_fingerprint() {
        let path = Path::new("/libraries/BD/book.cbz");
        let mtime = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
        let fp1 = compute_fingerprint(path, 1024, &mtime).unwrap();
        let fp2 = compute_fingerprint(path, 2048, &mtime).unwrap();
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn different_mtime_produces_different_fingerprint() {
        let path = Path::new("/libraries/BD/book.cbz");
        let mtime1 = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
        let mtime2 = Utc.with_ymd_and_hms(2025, 1, 15, 10, 31, 0).unwrap();
        let fp1 = compute_fingerprint(path, 1024, &mtime1).unwrap();
        let fp2 = compute_fingerprint(path, 1024, &mtime2).unwrap();
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn different_filename_produces_different_fingerprint() {
        let mtime = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
        let fp1 = compute_fingerprint(Path::new("/dir/book_a.cbz"), 1024, &mtime).unwrap();
        let fp2 = compute_fingerprint(Path::new("/dir/book_b.cbz"), 1024, &mtime).unwrap();
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn same_filename_different_directory_same_fingerprint() {
        let mtime = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
        let fp1 = compute_fingerprint(Path::new("/dir1/book.cbz"), 1024, &mtime).unwrap();
        let fp2 = compute_fingerprint(Path::new("/dir2/book.cbz"), 1024, &mtime).unwrap();
        assert_eq!(fp1, fp2, "fingerprint uses only filename, not full path");
    }

    #[test]
    fn output_is_hex_sha256() {
        let path = Path::new("/libraries/BD/book.cbz");
        let mtime = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
        let fp = compute_fingerprint(path, 1024, &mtime).unwrap();
        assert_eq!(fp.len(), 64, "SHA-256 hex should be 64 chars");
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
