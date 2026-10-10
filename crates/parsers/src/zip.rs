use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use anyhow::{Context, Result};

use crate::cbr::{analyze_cbr, extract_cbr_page, list_cbr_images};
use crate::{is_image_name, ArchiveIndexCache};

pub fn analyze_cbz(path: &Path, allow_fallback: bool) -> Result<(i32, Vec<u8>)> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(zip_err) => {
            if allow_fallback {
                tracing::debug!(target: "extraction", "[EXTRACTION] ZipArchive::new failed for {}: {} — trying fallbacks", path.display(), zip_err);

                // Check magic bytes to avoid expensive RAR probe on ZIP files
                let is_zip_magic = std::fs::File::open(path)
                    .and_then(|mut f| {
                        let mut magic = [0u8; 4];
                        std::io::Read::read_exact(&mut f, &mut magic)?;
                        Ok(magic[0] == b'P' && magic[1] == b'K')
                    })
                    .unwrap_or(false);

                if !is_zip_magic {
                    // Try RAR fallback (file might be a RAR with .cbz extension)
                    if let Ok(result) = analyze_cbr(path, false) {
                        tracing::debug!(target: "extraction", "[EXTRACTION] RAR fallback succeeded for {}", path.display());
                        return Ok(result);
                    }
                }

                // Try streaming fallback: read local file headers without central directory
                // (handles ZIP files with NTFS extra fields that confuse the central dir parser)
                let t0 = std::time::Instant::now();
                if let Ok(result) = analyze_cbz_streaming(path) {
                    tracing::debug!(target: "extraction", "[EXTRACTION] Streaming fallback succeeded for {} — {} pages in {:.0}ms", path.display(), result.0, t0.elapsed().as_secs_f64() * 1000.0);
                    return Ok(result);
                }
            }
            return Err(anyhow::anyhow!(
                "invalid cbz archive for {}: {}",
                path.display(),
                zip_err
            ));
        }
    };

    let mut image_names: Vec<String> = archive
        .file_names()
        .filter(|name| is_image_name(&name.to_ascii_lowercase()))
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    image_names.sort_by(|a, b| natord::compare(a, b));

    if image_names.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in cbz: {}",
            path.display()
        ));
    }

    // Try images in order until one reads successfully (first pages can be corrupted too)
    let count = image_names.len() as i32;
    for first_image in &image_names {
        if let Ok(mut entry) = archive.by_name(first_image) {
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                return Ok((count, buf));
            }
        }
    }

    // zip v8 is stricter about local-header extra fields and CRC validation — fall back to
    // the raw streaming reader which bypasses those checks (method 0/8 only).
    if allow_fallback {
        if let Ok(result) = analyze_cbz_streaming(path) {
            tracing::debug!(target: "extraction", "[EXTRACTION] Streaming fallback succeeded for {} (by_name read failed)", path.display());
            return Ok(result);
        }
    }

    Err(anyhow::anyhow!(
        "all entries unreadable in cbz: {}",
        path.display()
    ))
}

// ---------------------------------------------------------------------------
// Raw ZIP reader — bypasses extra field validation (CRC32 on Unicode path, NTFS, etc.)
// ---------------------------------------------------------------------------

/// Information about a ZIP local file entry (parsed from raw headers).
pub struct RawZipEntry {
    name: String,
    compression: u16,
    compressed_size: u64,
    uncompressed_size: u64,
    /// File offset of the compressed data (right after name + extra field).
    data_offset: u64,
}

/// Scan local file headers and return metadata for all entries.
/// Does NOT read file data — only collects names and offsets.
pub fn raw_zip_list_entries(path: &Path) -> Result<Vec<RawZipEntry>> {
    use std::io::{BufReader, Seek, SeekFrom};

    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open zip: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut entries = Vec::new();

    loop {
        let mut sig = [0u8; 4];
        if reader.read_exact(&mut sig).is_err() {
            break;
        }
        if u32::from_le_bytes(sig) != 0x04034b50 {
            break;
        }

        let mut hdr = [0u8; 26];
        reader
            .read_exact(&mut hdr)
            .context("truncated local file header")?;

        let compression = u16::from_le_bytes([hdr[4], hdr[5]]);
        let compressed_size = u32::from_le_bytes([hdr[14], hdr[15], hdr[16], hdr[17]]) as u64;
        let uncompressed_size = u32::from_le_bytes([hdr[18], hdr[19], hdr[20], hdr[21]]) as u64;
        let name_len = u16::from_le_bytes([hdr[22], hdr[23]]) as u64;
        let extra_len = u16::from_le_bytes([hdr[24], hdr[25]]) as u64;

        let mut name_buf = vec![0u8; name_len as usize];
        reader.read_exact(&mut name_buf)?;
        let name = String::from_utf8_lossy(&name_buf).to_string();

        // Skip extra field entirely
        if extra_len > 0 {
            reader.seek(SeekFrom::Current(extra_len as i64))?;
        }

        let data_offset = reader.stream_position()?;

        entries.push(RawZipEntry {
            name,
            compression,
            compressed_size,
            uncompressed_size,
            data_offset,
        });

        // Skip file data
        if compressed_size > 0 {
            reader.seek(SeekFrom::Current(compressed_size as i64))?;
        }
    }

    Ok(entries)
}

/// Read and decompress the data for a single entry.
pub fn raw_zip_read_entry(path: &Path, entry: &RawZipEntry) -> Result<Vec<u8>> {
    use std::io::{BufReader, Seek, SeekFrom};

    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(entry.data_offset))?;

    let mut compressed = vec![0u8; entry.compressed_size as usize];
    reader.read_exact(&mut compressed)?;

    match entry.compression {
        0 => Ok(compressed),
        8 => {
            let mut decoder = flate2::read::DeflateDecoder::new(&compressed[..]);
            let mut decompressed = Vec::with_capacity(entry.uncompressed_size as usize);
            decoder.read_to_end(&mut decompressed)?;
            Ok(decompressed)
        }
        other => Err(anyhow::anyhow!(
            "unsupported zip compression method: {}",
            other
        )),
    }
}

/// Fallback: list image names + extract all images (for analyze_book which needs first page + count).
pub fn analyze_cbz_streaming(path: &Path) -> Result<(i32, Vec<u8>)> {
    let entries = raw_zip_list_entries(path)?;
    let mut image_entries: Vec<&RawZipEntry> = entries
        .iter()
        .filter(|e| is_image_name(&e.name.to_ascii_lowercase()))
        .collect();

    if image_entries.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in streaming cbz: {}",
            path.display()
        ));
    }

    image_entries.sort_by(|a, b| natord::compare(&a.name, &b.name));
    let count = image_entries.len() as i32;
    let first_bytes = raw_zip_read_entry(path, image_entries[0])?;
    Ok((count, first_bytes))
}

pub fn parse_cbz_page_count(path: &Path) -> Result<i32> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    match zip::ZipArchive::new(file) {
        Ok(mut archive) => {
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
        Err(_) => {
            // Fallback: streaming count (bypasses extra field validation)
            parse_cbz_page_count_streaming(path)
        }
    }
}

pub fn parse_cbz_page_count_streaming(path: &Path) -> Result<i32> {
    let entries = raw_zip_list_entries(path)?;
    let count = entries
        .iter()
        .filter(|e| is_image_name(&e.name.to_ascii_lowercase()))
        .count() as i32;
    Ok(count)
}

pub fn list_cbz_images(path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(zip_err) => {
            // Try RAR fallback
            if let Ok(names) = list_cbr_images(path) {
                return Ok(names);
            }
            // Try streaming fallback
            return list_cbz_images_streaming(path)
                .map_err(|_| anyhow::anyhow!("invalid cbz for {}: {}", path.display(), zip_err));
        }
    };

    let mut names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let lower = entry.name().to_ascii_lowercase();
        if is_image_name(&lower) {
            names.push(entry.name().to_string());
        }
    }
    names.sort_by(|a, b| natord::compare(a, b));
    Ok(names)
}

pub fn list_cbz_images_streaming(path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz for streaming: {}", path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    let mut names: Vec<String> = Vec::new();

    loop {
        match zip::read::read_zipfile_from_stream(&mut reader) {
            Ok(Some(mut entry)) => {
                let name = entry.name().to_string();
                if is_image_name(&name.to_ascii_lowercase()) {
                    names.push(name);
                }
                std::io::copy(&mut entry, &mut std::io::sink())?;
            }
            Ok(None) => break,
            Err(_) => {
                if !names.is_empty() {
                    break;
                }
                return Err(anyhow::anyhow!(
                    "streaming ZIP listing failed for {}",
                    path.display()
                ));
            }
        }
    }
    names.sort_by(|a, b| natord::compare(a, b));
    Ok(names)
}

pub fn extract_cbz_by_name(path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return extract_cbz_by_name_streaming(path, image_name),
    };
    let mut entry = archive
        .by_name(image_name)
        .with_context(|| format!("entry '{}' not found in {}", image_name, path.display()))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn extract_cbz_by_name_streaming(path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz for streaming: {}", path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    loop {
        match zip::read::read_zipfile_from_stream(&mut reader) {
            Ok(Some(mut entry)) => {
                if entry.name() == image_name {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf)?;
                    return Ok(buf);
                }
                std::io::copy(&mut entry, &mut std::io::sink())?;
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    Err(anyhow::anyhow!(
        "entry '{}' not found in streaming cbz: {}",
        image_name,
        path.display()
    ))
}

/// Cache of sorted image names per archive path. Avoids re-listing and sorting on every page request.
/// Keyed by (path, mtime) so the cache invalidates automatically when the file is replaced.
static CBZ_INDEX_CACHE: ArchiveIndexCache = OnceLock::new();

pub fn cbz_index_cache() -> &'static Mutex<HashMap<PathBuf, (SystemTime, Vec<String>)>> {
    CBZ_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Get sorted image names from cache, or list + sort + cache them.
pub fn get_cbz_image_index(
    path: &Path,
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> Vec<String> {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    {
        let cache = cbz_index_cache().lock().unwrap();
        if let Some((cached_mtime, names)) = cache.get(path) {
            if mtime == Some(*cached_mtime) {
                return names.clone();
            }
        }
    }
    let mut image_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.name().to_ascii_lowercase();
        if is_image_name(&name) {
            image_names.push(entry.name().to_string());
        }
    }
    image_names.sort_by(|a, b| natord::compare(a, b));
    if let Some(mtime) = mtime {
        let mut cache = cbz_index_cache().lock().unwrap();
        cache.insert(path.to_path_buf(), (mtime, image_names.clone()));
    }
    image_names
}

pub fn extract_cbz_page(path: &Path, page_number: u32, allow_fallback: bool) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open cbz: {}", path.display()))?;
    let index = page_number as usize - 1;

    match zip::ZipArchive::new(file) {
        Ok(mut archive) => {
            let image_names = get_cbz_image_index(path, &mut archive);

            let selected = image_names.get(index).with_context(|| {
                format!(
                    "page {} out of range (total: {})",
                    page_number,
                    image_names.len()
                )
            })?;

            let mut entry = archive
                .by_name(selected)
                .with_context(|| format!("cannot read page {}", selected))?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            Ok(buf)
        }
        Err(zip_err) => {
            if allow_fallback {
                // Try RAR fallback (file might be a RAR with .cbz extension)
                if let Ok(data) = extract_cbr_page(path, page_number, false) {
                    return Ok(data);
                }
                // Raw ZIP fallback (bypasses extra field validation)
                return extract_cbz_page_raw(path, page_number);
            }
            Err(anyhow::anyhow!(
                "invalid cbz archive for {}: {}",
                path.display(),
                zip_err
            ))
        }
    }
}

pub fn extract_cbz_page_raw(path: &Path, page_number: u32) -> Result<Vec<u8>> {
    let entries = raw_zip_list_entries(path)?;
    let mut image_entries: Vec<&RawZipEntry> = entries
        .iter()
        .filter(|e| is_image_name(&e.name.to_ascii_lowercase()))
        .collect();
    image_entries.sort_by(|a, b| natord::compare(&a.name, &b.name));

    let index = page_number as usize - 1;
    let entry = image_entries.get(index).with_context(|| {
        format!(
            "page {} out of range (total: {})",
            page_number,
            image_entries.len()
        )
    })?;

    raw_zip_read_entry(path, entry)
}
