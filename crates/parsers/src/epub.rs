use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use anyhow::{Context, Result};

use crate::{is_image_name, ArchiveIndexCache};

// ============================================================
// EPUB support — spine-aware image index with cache
// ============================================================

/// Cache of ordered image paths per EPUB file. Avoids re-parsing OPF/XHTML on every page request.
/// Keyed by (path, mtime) so the cache invalidates automatically when the file is replaced.
static EPUB_INDEX_CACHE: ArchiveIndexCache = OnceLock::new();

pub fn epub_index_cache() -> &'static Mutex<HashMap<PathBuf, (SystemTime, Vec<String>)>> {
    EPUB_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

// Pre-compiled regex patterns for EPUB XML parsing (compiled once on first use)
static RE_EPUB_ROOTFILE: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ITEM: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ITEMREF: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_IMG_SRC: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_SVG_HREF: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ATTR_ID: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ATTR_HREF: OnceLock<regex::Regex> = OnceLock::new();
static RE_EPUB_ATTR_MEDIA: OnceLock<regex::Regex> = OnceLock::new();

pub struct EpubManifestItem {
    href: String,
    media_type: String,
}

/// Build the ordered list of image paths for an EPUB file.
/// Walks the OPF spine to determine reading order, parses XHTML/SVG pages
/// for image references, and falls back to CBZ-style listing if no
/// images are found through the spine.
pub fn build_epub_image_index(path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open epub: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("invalid epub zip: {}", path.display()))?;

    // 1. Find OPF path from META-INF/container.xml
    let opf_path = {
        let mut entry = archive
            .by_name("META-INF/container.xml")
            .context("missing META-INF/container.xml — not a valid EPUB")?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let xml = String::from_utf8_lossy(&buf);
        let re = RE_EPUB_ROOTFILE.get_or_init(|| {
            regex::Regex::new(r#"<(?:\w+:)?rootfile[^>]+full-path="([^"]+)""#).unwrap()
        });
        re.captures(&xml)
            .and_then(|c| c.get(1))
            .map(|m| decode_xml_entities(m.as_str()))
            .context("no rootfile found in container.xml")?
    };

    let opf_dir = std::path::Path::new(&opf_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    // 2. Parse OPF manifest + spine
    let (manifest, spine_idrefs) = {
        let mut entry = archive
            .by_name(&opf_path)
            .with_context(|| format!("missing OPF file: {}", opf_path))?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let xml = String::from_utf8_lossy(&buf);
        parse_epub_opf(&xml, &opf_dir)?
    };

    // 3. Walk spine entries to build ordered image list
    let re_img = RE_EPUB_IMG_SRC
        .get_or_init(|| regex::Regex::new(r#"(?i)<img\s[^>]*src=["']([^"']+)["']"#).unwrap());
    let re_svg = RE_EPUB_SVG_HREF.get_or_init(|| {
        regex::Regex::new(r#"(?i)<image\s[^>]*(?:xlink:)?href=["']([^"']+)["']"#).unwrap()
    });

    let mut images: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for idref in &spine_idrefs {
        let item = match manifest.get(idref.as_str()) {
            Some(item) => item,
            None => continue,
        };

        // Direct raster image in spine (rare but possible)
        if item.media_type.starts_with("image/") && !item.media_type.contains("svg") {
            if seen.insert(item.href.clone()) {
                images.push(item.href.clone());
            }
            continue;
        }

        // Read XHTML/SVG content — entry is dropped at end of match arm, releasing archive borrow
        let content = match archive.by_name(&item.href) {
            Ok(mut entry) => {
                let mut buf = Vec::new();
                match entry.read_to_end(&mut buf) {
                    Ok(_) => String::from_utf8_lossy(&buf).to_string(),
                    Err(_) => continue,
                }
            }
            Err(_) => continue,
        };

        let content_dir = std::path::Path::new(&item.href)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // Extract <img src="..."> and <image [xlink:]href="...">
        for re in [re_img, re_svg] {
            for cap in re.captures_iter(&content) {
                if let Some(src) = cap.get(1) {
                    let src_str = src.as_str();
                    if src_str.starts_with("data:") {
                        continue;
                    }
                    let decoded = decode_xml_entities(&percent_decode_epub(src_str));
                    let resolved = resolve_epub_path(&content_dir, &decoded);
                    if seen.insert(resolved.clone()) {
                        images.push(resolved);
                    }
                }
            }
        }
    }

    // 4. Fallback: no images from spine → list all images in ZIP (CBZ-style)
    if images.is_empty() {
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                let name = entry.name().to_string();
                if is_image_name(&name.to_ascii_lowercase()) && seen.insert(name.clone()) {
                    images.push(name);
                }
            }
        }
        images.sort_by(|a, b| natord::compare(a, b));
    }

    if images.is_empty() {
        return Err(anyhow::anyhow!(
            "no images found in epub: {}",
            path.display()
        ));
    }

    Ok(images)
}

pub fn parse_epub_opf(
    xml: &str,
    opf_dir: &str,
) -> Result<(HashMap<String, EpubManifestItem>, Vec<String>)> {
    let re_item = RE_EPUB_ITEM
        .get_or_init(|| regex::Regex::new(r#"(?s)<(?:\w+:)?item\s([^>]+?)/?>"#).unwrap());
    let re_itemref = RE_EPUB_ITEMREF
        .get_or_init(|| regex::Regex::new(r#"<(?:\w+:)?itemref\s[^>]*idref="([^"]+)""#).unwrap());
    let re_id =
        RE_EPUB_ATTR_ID.get_or_init(|| regex::Regex::new(r#"(?:^|\s)id="([^"]+)""#).unwrap());
    let re_href =
        RE_EPUB_ATTR_HREF.get_or_init(|| regex::Regex::new(r#"(?:^|\s)href="([^"]+)""#).unwrap());
    let re_media =
        RE_EPUB_ATTR_MEDIA.get_or_init(|| regex::Regex::new(r#"media-type="([^"]+)""#).unwrap());

    let mut manifest: HashMap<String, EpubManifestItem> = HashMap::new();
    for cap in re_item.captures_iter(xml) {
        if let Some(attrs) = cap.get(1) {
            let a = attrs.as_str();
            let id = re_id.captures(a).and_then(|c| c.get(1));
            let href = re_href.captures(a).and_then(|c| c.get(1));
            let media = re_media.captures(a).and_then(|c| c.get(1));

            if let (Some(id), Some(href), Some(media)) = (id, href, media) {
                let decoded_href = decode_xml_entities(&percent_decode_epub(href.as_str()));
                let resolved = resolve_epub_path(opf_dir, &decoded_href);
                manifest.insert(
                    id.as_str().to_string(),
                    EpubManifestItem {
                        href: resolved,
                        media_type: media.as_str().to_string(),
                    },
                );
            }
        }
    }

    let spine_idrefs: Vec<String> = re_itemref
        .captures_iter(xml)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();

    Ok((manifest, spine_idrefs))
}

/// Get the cached image index for an EPUB, building it on first access.
pub fn get_epub_image_index(path: &Path) -> Result<Vec<String>> {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    {
        let cache = epub_index_cache().lock().unwrap();
        if let Some((cached_mtime, names)) = cache.get(path) {
            if mtime == Some(*cached_mtime) {
                return Ok(names.clone());
            }
        }
    }
    let images = build_epub_image_index(path)?;
    if let Some(mtime) = mtime {
        let mut cache = epub_index_cache().lock().unwrap();
        cache.insert(path.to_path_buf(), (mtime, images.clone()));
    }
    Ok(images)
}

pub fn parse_epub_page_count(path: &Path) -> Result<i32> {
    let images = build_epub_image_index(path)?;
    Ok(images.len() as i32)
}

pub fn analyze_epub(path: &Path) -> Result<(i32, Vec<u8>)> {
    let images = get_epub_image_index(path)?;
    let count = images.len() as i32;

    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open epub: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;

    for img_path in &images {
        if let Ok(mut entry) = archive.by_name(img_path) {
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                return Ok((count, buf));
            }
        }
    }

    Err(anyhow::anyhow!(
        "no readable images in epub: {}",
        path.display()
    ))
}

pub fn extract_epub_page(path: &Path, page_number: u32) -> Result<Vec<u8>> {
    let images = get_epub_image_index(path)?;
    let index = page_number as usize - 1;
    let img_path = images.get(index).with_context(|| {
        format!(
            "page {} out of range (total: {})",
            page_number,
            images.len()
        )
    })?;

    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open epub: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive
        .by_name(img_path)
        .with_context(|| format!("image '{}' not found in epub", img_path))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

// --- EPUB path/encoding helpers ---

pub fn resolve_epub_path(base_dir: &str, href: &str) -> String {
    if let Some(stripped) = href.strip_prefix('/') {
        return normalize_epub_path(stripped);
    }
    if base_dir.is_empty() {
        return normalize_epub_path(href);
    }
    normalize_epub_path(&format!("{}/{}", base_dir, href))
}

pub fn normalize_epub_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

pub fn percent_decode_epub(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (epub_hex_val(bytes[i + 1]), epub_hex_val(bytes[i + 2])) {
                result.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

pub fn epub_hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn decode_xml_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}
