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
    count_cbr_pages(path, true)
}

/// Count the images of a RAR archive. With `allow_fallback`, a file that is not a RAR is
/// counted as a ZIP; without it (used by the CBZ reader's own RAR fallback) the error is
/// returned, so the two fallbacks can never call each other in a loop.
pub(crate) fn count_cbr_pages(path: &Path, allow_fallback: bool) -> Result<i32> {
    let archive = match open_cbr_listing(path) {
        Ok(a) => a,
        Err(e_str) => {
            if allow_fallback && is_not_rar_error(&e_str) {
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
    list_cbr_images_with(path, true)
}

/// Sorted image names of a RAR archive. Without `allow_fallback` (the CBZ reader's own RAR
/// fallback), a file that is not a RAR is an error instead of going back to the CBZ reader,
/// which would recurse forever on a file that is neither.
pub(crate) fn list_cbr_images_with(path: &Path, allow_fallback: bool) -> Result<Vec<String>> {
    let archive = match open_cbr_listing(path) {
        Ok(a) => a,
        Err(e_str) => {
            if allow_fallback && is_not_rar_error(&e_str) {
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

/// Turn an archive entry name into a safe relative ZIP path: `\\` separators become `/`,
/// and root, drive prefix, `.` and `..` components are dropped so the entry can never
/// point outside the archive. `None` when nothing is left.
pub(crate) fn sanitize_entry_path(name: &str) -> Option<String> {
    let normalized = name.replace('\\', "/");
    let mut components = normalized
        .split('/')
        .filter(|c| !c.is_empty() && *c != "." && *c != "..")
        .peekable();
    // Windows drive prefix (`C:`) left by some RAR tools.
    if components
        .peek()
        .is_some_and(|c| c.len() == 2 && c.ends_with(':'))
    {
        components.next();
    }
    let path = components.collect::<Vec<_>>().join("/");
    (!path.is_empty()).then_some(path)
}

/// Prepare the images of a CBR for the CBZ: keep each entry's relative path (sanitized, so
/// `ch1/01.jpg` and `ch2/01.jpg` stay distinct and keep their order) and sort on the full path
/// with the same natural order every page reader uses. Names that still collide after
/// sanitization get a numeric suffix rather than being dropped.
pub(crate) fn cbz_entries<T>(images: Vec<(String, T)>) -> Vec<(String, T)> {
    let mut entries: Vec<(String, T)> = images
        .into_iter()
        .filter_map(|(name, data)| sanitize_entry_path(&name).map(|path| (path, data)))
        .collect();
    entries.sort_by(|(a, _), (b, _)| natord::compare(a, b));

    let mut used = std::collections::HashSet::new();
    for (name, _) in &mut entries {
        if used.insert(name.clone()) {
            continue;
        }
        let (stem, ext) = match name.rfind('.') {
            Some(dot) if dot > name.rfind('/').map_or(0, |slash| slash + 1) => {
                (name[..dot].to_string(), name[dot..].to_string())
            }
            _ => (name.clone(), String::new()),
        };
        let mut n = 2;
        while !used.insert(format!("{stem}~{n}{ext}")) {
            n += 1;
        }
        *name = format!("{stem}~{n}{ext}");
    }
    entries
}

/// Write `entries` (already in page order) to `tmp_path` and check that every image made it.
/// The temporary file is removed on any failure.
pub(crate) fn write_verified_cbz(tmp_path: &Path, entries: &[(String, Vec<u8>)]) -> Result<()> {
    let result = (|| -> Result<()> {
        let cbz_file = std::fs::File::create(tmp_path)
            .with_context(|| format!("cannot create {}", tmp_path.display()))?;
        let mut zip = zip::ZipWriter::new(cbz_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for (file_name, data) in entries {
            zip.start_file(file_name, options)
                .with_context(|| format!("cannot add file {} to zip", file_name))?;
            zip.write_all(data)
                .with_context(|| format!("cannot write {} to zip", file_name))?;
        }
        zip.finish().context("cannot finalize zip")?;

        let packed = sorted_zip_image_names(tmp_path)?;
        if packed.len() != entries.len() {
            return Err(anyhow::anyhow!(
                "CBZ verification failed: expected {} images, found {}",
                entries.len(),
                packed.len()
            ));
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(tmp_path);
    }
    result
}

/// Image entry names of a real ZIP archive (no RAR or streaming fallback), naturally sorted.
fn sorted_zip_image_names(path: &Path) -> Result<Vec<String>> {
    let file =
        std::fs::File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let archive = zip::ZipArchive::new(file).context("invalid zip archive")?;
    let mut names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.name_for_index(i).and_then(Result::ok))
        .filter(|n| is_image_name(&n.to_ascii_lowercase()))
        .map(|n| n.to_string())
        .collect();
    names.sort_by(|a, b| natord::compare(a, b));
    Ok(names)
}

/// Whether `cbz_path` is the complete conversion of `cbr_path`: a valid ZIP holding exactly
/// the images [`convert_cbr_to_cbz`] would write, under the same names. The conversion
/// publishes the CBZ with an atomic rename, so a CBZ left next to its CBR by a crash between
/// the rename and the database update passes this check and can be adopted as is.
pub fn cbz_is_conversion_of(cbr_path: &Path, cbz_path: &Path) -> Result<bool> {
    let expected: Vec<String> = cbz_entries(
        list_cbr_images(cbr_path)?
            .into_iter()
            .map(|name| (name, ()))
            .collect(),
    )
    .into_iter()
    .map(|(name, ())| name)
    .collect();
    let Ok(found) = sorted_zip_image_names(cbz_path) else {
        return Ok(false);
    };
    Ok(!expected.is_empty() && expected == found)
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

        if is_image_name(&entry_name.to_ascii_lowercase()) {
            let (data, next) = header
                .read()
                .map_err(|e| anyhow::anyhow!("unrar read: {}", e))?;
            images.push((entry_name, data));
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

    write_verified_cbz(&tmp_path, &cbz_entries(images))?;

    std::fs::rename(&tmp_path, &cbz_path).with_context(|| {
        format!(
            "cannot rename {} to {}",
            tmp_path.display(),
            cbz_path.display()
        )
    })?;

    Ok(cbz_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Minimal valid JPEG (SOI/EOI only): `is_image_name` matches on the
    /// extension and none of these paths decode the bytes.
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xD9];
    const SECOND: &[u8] = &[0x01, 0x02, 0x03, 0x04];

    /// Write a ZIP archive under a `.cbr` name. Many releases are really ZIP
    /// files with the wrong extension — the historical reason for the fallback
    /// chain these tests pin down.
    fn write_zip_named(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).expect("create archive");
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            zip.start_file(*name, opts).expect("start entry");
            zip.write_all(data).expect("write entry");
        }
        zip.finish().expect("finish archive");
    }

    #[test]
    fn zip_named_cbr_falls_back_to_cbz_listing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbr");
        write_zip_named(&path, &[("001.jpg", JPEG), ("002.jpg", SECOND)]);

        let (count, first) = analyze_cbr(&path, true).unwrap();

        assert_eq!(count, 2);
        assert_eq!(first, JPEG);
    }

    #[test]
    fn zip_named_cbr_without_fallback_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbr");
        write_zip_named(&path, &[("001.jpg", JPEG)]);

        assert!(analyze_cbr(&path, false).is_err());
    }

    #[test]
    fn page_count_and_listing_fall_back_to_cbz() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbr");
        write_zip_named(&path, &[("002.jpg", SECOND), ("001.jpg", JPEG)]);

        assert_eq!(parse_cbr_page_count(&path).unwrap(), 2);
        // Listing is naturally sorted, not archive order.
        assert_eq!(
            list_cbr_images(&path).unwrap(),
            vec!["001.jpg".to_string(), "002.jpg".to_string()]
        );
    }

    #[test]
    fn extract_page_falls_back_to_cbz() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbr");
        write_zip_named(&path, &[("001.jpg", JPEG), ("002.jpg", SECOND)]);

        assert_eq!(extract_cbr_page(&path, 2, true).unwrap(), SECOND);

        let err = extract_cbr_page(&path, 3, true).unwrap_err();
        assert!(
            err.to_string().contains("out of range"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn convert_refuses_when_the_cbz_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let cbr = dir.path().join("book.cbr");
        write_zip_named(&cbr, &[("001.jpg", JPEG)]);
        std::fs::write(dir.path().join("book.cbz"), b"already there").unwrap();

        let err = convert_cbr_to_cbz(&cbr).unwrap_err();
        assert!(
            err.to_string().contains("already exists"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn convert_reports_a_zip_input_as_an_unrar_failure() {
        let dir = tempfile::tempdir().unwrap();
        let cbr = dir.path().join("book.cbr");
        write_zip_named(&cbr, &[("001.jpg", JPEG)]);

        // `convert_cbr_to_cbz` has no ZIP fallback by design: a mislabelled ZIP
        // must fail explicitly rather than silently produce a wrong CBZ.
        let err = convert_cbr_to_cbz(&cbr).unwrap_err();
        assert!(err.to_string().contains("unrar"), "unexpected: {err}");
        assert!(!dir.path().join("book.cbz").exists());
        assert!(!dir.path().join("book.cbz.tmp").exists());
    }

    #[test]
    fn sanitize_entry_path_keeps_relative_paths_only() {
        assert_eq!(
            sanitize_entry_path("ch1/01.jpg").as_deref(),
            Some("ch1/01.jpg")
        );
        assert_eq!(
            sanitize_entry_path("ch1\\01.jpg").as_deref(),
            Some("ch1/01.jpg")
        );
        assert_eq!(
            sanitize_entry_path("/abs/01.jpg").as_deref(),
            Some("abs/01.jpg")
        );
        assert_eq!(
            sanitize_entry_path("../../etc/01.jpg").as_deref(),
            Some("etc/01.jpg")
        );
        assert_eq!(
            sanitize_entry_path("./a/./b/../01.jpg").as_deref(),
            Some("a/b/01.jpg")
        );
        assert_eq!(
            sanitize_entry_path("C:\\scans\\01.jpg").as_deref(),
            Some("scans/01.jpg")
        );
        assert_eq!(sanitize_entry_path("../"), None);
    }

    #[test]
    fn cbz_entries_keep_folders_order_and_never_drop_a_page() {
        let entries = cbz_entries(vec![
            ("ch2/01.jpg".to_string(), 3),
            ("ch10/01.jpg".to_string(), 5),
            ("ch1/02.jpg".to_string(), 2),
            ("ch1/01.jpg".to_string(), 1),
            ("ch2/02.jpg".to_string(), 4),
            ("../ch2/02.jpg".to_string(), 6),
        ]);
        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "ch1/01.jpg",
                "ch1/02.jpg",
                "ch2/01.jpg",
                "ch2/02.jpg",
                "ch2/02~2.jpg",
                "ch10/01.jpg"
            ]
        );
        assert_eq!(entries.len(), 6, "colliding names are renamed, not dropped");
    }

    /// Basenames repeated across chapter folders used to be flattened (duplicate ZIP names,
    /// page order changed). The converted CBZ must read its pages in the order the CBR
    /// reader shows them: natural order of the full path.
    #[test]
    fn converted_cbz_reads_pages_in_cbr_order() {
        let dir = tempfile::tempdir().unwrap();
        let cbz = dir.path().join("book.cbz.tmp");
        let raw = vec![
            ("Book/ch2/01.jpg".to_string(), vec![3u8]),
            ("Book/ch1/02.jpg".to_string(), vec![2u8]),
            ("Book/ch1/01.jpg".to_string(), vec![1u8]),
            ("Book/ch2/02.jpg".to_string(), vec![4u8]),
        ];
        let mut cbr_order: Vec<String> = raw.iter().map(|(n, _)| n.clone()).collect();
        cbr_order.sort_by(|a, b| natord::compare(a, b));

        write_verified_cbz(&cbz, &cbz_entries(raw)).unwrap();

        assert_eq!(list_cbz_images(&cbz).unwrap(), cbr_order);
        for page in 1..=4u32 {
            assert_eq!(
                extract_cbz_page(&cbz, page, false).unwrap(),
                vec![page as u8]
            );
        }
    }

    #[test]
    fn cbz_is_conversion_of_accepts_only_a_complete_matching_cbz() {
        let dir = tempfile::tempdir().unwrap();
        // A ZIP-named CBR: its listing goes through the CBZ fallback, which is enough to
        // exercise the comparison without a RAR writer.
        let cbr = dir.path().join("book.cbr");
        write_zip_named(&cbr, &[("ch1/001.jpg", JPEG), ("ch2/001.jpg", SECOND)]);

        let complete = dir.path().join("complete.cbz");
        write_verified_cbz(
            &complete,
            &cbz_entries(vec![
                ("ch1/001.jpg".to_string(), JPEG.to_vec()),
                ("ch2/001.jpg".to_string(), SECOND.to_vec()),
            ]),
        )
        .unwrap();
        assert!(cbz_is_conversion_of(&cbr, &complete).unwrap());

        let partial = dir.path().join("partial.cbz");
        write_verified_cbz(&partial, &[("ch1/001.jpg".to_string(), JPEG.to_vec())]).unwrap();
        assert!(!cbz_is_conversion_of(&cbr, &partial).unwrap());

        let garbage = dir.path().join("garbage.cbz");
        std::fs::write(&garbage, b"truncated").unwrap();
        assert!(!cbz_is_conversion_of(&cbr, &garbage).unwrap());
    }

    #[test]
    fn handmade_rar_is_readable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbr");
        crate::test_support::write_rar4(&path, &[("002.jpg", SECOND), ("001.jpg", JPEG)]);

        assert_eq!(parse_cbr_page_count(&path).unwrap(), 2);
        assert_eq!(
            list_cbr_images(&path).unwrap(),
            vec!["001.jpg".to_string(), "002.jpg".to_string()]
        );
        assert_eq!(extract_cbr_page(&path, 2, false).unwrap(), SECOND);
    }

    /// End to end: chapter folders sharing page names survive the conversion, and every page
    /// of the CBZ is the page the CBR reader showed at the same position.
    #[test]
    fn convert_keeps_chapter_folders_and_page_order() {
        let dir = tempfile::tempdir().unwrap();
        let cbr = dir.path().join("book.cbr");
        let pages: [(&str, &[u8]); 4] = [
            ("Book/ch2/01.jpg", &[3]),
            ("Book/ch1/02.jpg", &[2]),
            ("Book/ch10/01.jpg", &[4]),
            ("Book/ch1/01.jpg", &[1]),
        ];
        crate::test_support::write_rar4(&cbr, &pages);

        let cbz = convert_cbr_to_cbz(&cbr).unwrap();

        assert_eq!(
            list_cbz_images(&cbz).unwrap(),
            list_cbr_images(&cbr).unwrap()
        );
        for page in 1..=4 {
            assert_eq!(
                extract_cbz_page(&cbz, page, false).unwrap(),
                extract_cbr_page(&cbr, page, false).unwrap(),
                "page {page}"
            );
        }
        assert!(cbz_is_conversion_of(&cbr, &cbz).unwrap());
    }
}
