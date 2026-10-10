use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::is_image_name;
use crate::zip::{analyze_cbz, extract_cbz_page, list_cbz_images, parse_cbz_page_count};

/// Returns true if the error indicates the file is not a RAR archive (likely a ZIP with .cbr extension).
pub fn is_not_rar_error(err_str: &str) -> bool {
    err_str.contains("Not a RAR archive") || err_str.contains("bad archive")
}

/// Try to open a CBR file for listing. Returns the archive or an error string.
/// If the error indicates the file is not actually a RAR archive, the caller can fall back to CBZ.
pub fn open_cbr_listing(
    path: &Path,
) -> std::result::Result<unrar::OpenArchive<unrar::List, unrar::CursorBeforeHeader>, String> {
    unrar::Archive::new(path)
        .open_for_listing()
        .map_err(|e| format!("unrar listing failed for {}: {}", path.display(), e))
}

pub fn analyze_cbr(path: &Path, allow_fallback: bool) -> Result<(i32, Vec<u8>)> {
    // Pass 1: list all image names via unrar (in-process, no subprocess)
    let mut image_names: Vec<String> = {
        // Some .cbr files are actually ZIP archives with wrong extension — fallback to CBZ parser
        let archive = match open_cbr_listing(path) {
            Ok(a) => a,
            Err(e_str) => {
                if allow_fallback && is_not_rar_error(&e_str) {
                    return analyze_cbz(path, false).map_err(|zip_err| {
                        anyhow::anyhow!(
                            "not a RAR archive and ZIP fallback also failed for {}: RAR={}, ZIP={}",
                            path.display(),
                            e_str,
                            zip_err
                        )
                    });
                }
                return Err(anyhow::anyhow!("{}", e_str));
            }
        };
        let mut names = Vec::new();
        for entry in archive {
            let entry = entry.map_err(|e| anyhow::anyhow!("unrar entry error: {}", e))?;
            let name = entry.filename.to_string_lossy().to_string();
            if is_image_name(&name.to_ascii_lowercase()) {
                names.push(name);
            }
        }
        names
    };

    if image_names.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in cbr: {}",
            path.display()
        ));
    }

    image_names.sort_by(|a, b| natord::compare(a, b));
    let count = image_names.len() as i32;
    let first_name = image_names[0].clone();

    // Pass 2: extract first image to memory
    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| {
            anyhow::anyhow!(
                "unrar open for processing failed for {}: {}",
                path.display(),
                e
            )
        })?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        if entry_name == first_name {
            let (data, _) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read data: {}", e))?;
            return Ok((count, data));
        }
        archive = header
            .skip()
            .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
    }

    Err(anyhow::anyhow!(
        "could not find '{}' in {}",
        first_name,
        path.display()
    ))
}

pub fn parse_cbr_page_count(path: &Path) -> Result<i32> {
    let archive = match open_cbr_listing(path) {
        Ok(a) => a,
        Err(e_str) => {
            if is_not_rar_error(&e_str) {
                return parse_cbz_page_count(path);
            }
            return Err(anyhow::anyhow!("{}", e_str));
        }
    };
    let count = archive
        .filter(|r| {
            r.as_ref()
                .map(|e| is_image_name(&e.filename.to_string_lossy().to_ascii_lowercase()))
                .unwrap_or(false)
        })
        .count() as i32;
    Ok(count)
}

pub fn list_cbr_images(path: &Path) -> Result<Vec<String>> {
    let archive = match open_cbr_listing(path) {
        Ok(a) => a,
        Err(e_str) => {
            if is_not_rar_error(&e_str) {
                return list_cbz_images(path);
            }
            return Err(anyhow::anyhow!("{}", e_str));
        }
    };
    let mut names: Vec<String> = Vec::new();
    for entry in archive {
        let entry = entry.map_err(|e| anyhow::anyhow!("unrar entry error: {}", e))?;
        let name = entry.filename.to_string_lossy().to_string();
        if is_image_name(&name.to_ascii_lowercase()) {
            names.push(name);
        }
    }
    names.sort_by(|a, b| natord::compare(a, b));
    Ok(names)
}

pub fn extract_cbr_by_name(path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| {
            anyhow::anyhow!(
                "unrar open for processing failed for {}: {}",
                path.display(),
                e
            )
        })?;
    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        if entry_name == image_name {
            let (data, _) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read data: {}", e))?;
            return Ok(data);
        }
        archive = header
            .skip()
            .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
    }
    Err(anyhow::anyhow!(
        "entry '{}' not found in cbr: {}",
        image_name,
        path.display()
    ))
}

pub fn extract_cbr_page(path: &Path, page_number: u32, allow_fallback: bool) -> Result<Vec<u8>> {
    let index = page_number as usize - 1;

    let mut image_names: Vec<String> = {
        let archive = match open_cbr_listing(path) {
            Ok(a) => a,
            Err(e_str) => {
                if allow_fallback && is_not_rar_error(&e_str) {
                    return extract_cbz_page(path, page_number, false);
                }
                return Err(anyhow::anyhow!("{}", e_str));
            }
        };
        let mut names = Vec::new();
        for entry in archive {
            let entry = entry.map_err(|e| anyhow::anyhow!("unrar entry error: {}", e))?;
            let name = entry.filename.to_string_lossy().to_string();
            if is_image_name(&name.to_ascii_lowercase()) {
                names.push(name);
            }
        }
        names
    };

    image_names.sort_by(|a, b| natord::compare(a, b));
    let target = image_names
        .get(index)
        .with_context(|| {
            format!(
                "page {} out of range (total: {})",
                page_number,
                image_names.len()
            )
        })?
        .clone();

    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| anyhow::anyhow!("unrar open for processing failed: {}", e))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        if entry_name == target {
            let (data, _) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read data: {}", e))?;
            return Ok(data);
        }
        archive = header
            .skip()
            .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
    }

    Err(anyhow::anyhow!(
        "page '{}' not found in {}",
        target,
        path.display()
    ))
}

/// Convert a CBR file to CBZ in-place (same directory, same stem).
///
/// The conversion is safe: a `.cbz.tmp` file is written first, verified, then
/// atomically renamed to `.cbz`. The original CBR is **not** deleted by this
/// function — the caller is responsible for removing it after a successful DB update.
///
/// Returns the path of the newly created `.cbz` file.
pub fn convert_cbr_to_cbz(cbr_path: &Path) -> Result<PathBuf> {
    let parent = cbr_path
        .parent()
        .with_context(|| format!("no parent directory for {}", cbr_path.display()))?;
    let stem = cbr_path
        .file_stem()
        .with_context(|| format!("no file stem for {}", cbr_path.display()))?;

    let cbz_path = parent.join(format!("{}.cbz", stem.to_string_lossy()));
    let tmp_path = parent.join(format!("{}.cbz.tmp", stem.to_string_lossy()));

    if cbz_path.exists() {
        return Err(anyhow::anyhow!(
            "CBZ file already exists: {}",
            cbz_path.display()
        ));
    }

    // Extract all images from CBR into memory using unrar crate (no subprocess)
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();
    let mut archive = unrar::Archive::new(cbr_path)
        .open_for_processing()
        .map_err(|e| anyhow::anyhow!("unrar open failed for {}: {}", cbr_path.display(), e))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("unrar read header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();
        let file_name = Path::new(&entry_name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| entry_name.clone());

        if is_image_name(&entry_name.to_ascii_lowercase()) {
            let (data, next) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read: {}", e))?;
            images.push((file_name, data));
            archive = next;
        } else {
            archive = header
                .skip()
                .map_err(|e| anyhow::anyhow!("unrar skip: {}", e))?;
        }
    }

    if images.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in CBR: {}",
            cbr_path.display()
        ));
    }

    images.sort_by(|(a, _), (b, _)| natord::compare(a, b));
    let image_count = images.len();

    // Pack images into the .cbz.tmp file
    let pack_result = (|| -> Result<()> {
        let cbz_file = std::fs::File::create(&tmp_path)
            .with_context(|| format!("cannot create {}", tmp_path.display()))?;
        let mut zip = zip::ZipWriter::new(cbz_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for (file_name, data) in &images {
            zip.start_file(file_name, options)
                .with_context(|| format!("cannot add file {} to zip", file_name))?;
            zip.write_all(data)
                .with_context(|| format!("cannot write {} to zip", file_name))?;
        }
        zip.finish().context("cannot finalize zip")?;
        Ok(())
    })();

    if let Err(err) = pack_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    // Verify the CBZ contains the expected number of images
    let verify_result = (|| -> Result<()> {
        let file = std::fs::File::open(&tmp_path)
            .with_context(|| format!("cannot open {}", tmp_path.display()))?;
        let archive = zip::ZipArchive::new(file).context("invalid zip archive")?;
        let packed_count = (0..archive.len())
            .filter(|&i| {
                archive
                    .name_for_index(i)
                    .map(|n| is_image_name(&n.to_ascii_lowercase()))
                    .unwrap_or(false)
            })
            .count();
        if packed_count != image_count {
            return Err(anyhow::anyhow!(
                "CBZ verification failed: expected {} images, found {}",
                image_count,
                packed_count
            ));
        }
        Ok(())
    })();

    if let Err(err) = verify_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    std::fs::rename(&tmp_path, &cbz_path).with_context(|| {
        format!(
            "cannot rename {} to {}",
            tmp_path.display(),
            cbz_path.display()
        )
    })?;

    Ok(cbz_path)
}
