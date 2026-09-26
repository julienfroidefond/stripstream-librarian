use sqlx::{PgPool, Row};
use tracing::{info, warn};
use uuid::Uuid;

use parsers::{detect_format, extract_volumes, fold_accents, parse_metadata_fast, VolumeType};
use stripstream_core::paths::{remap_libraries_path, unmap_libraries_path};

use crate::books::rename::{
    load_rename_max_volume, load_rename_templates, render_rename_filename, RenameTemplateBook,
};

use super::torrent_import::{ImportResult, ImportedFile, SkippedFile};

pub(super) async fn do_import(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    expected_volumes: &[i32],
    content_path: &str,
    replace_existing: bool,
) -> anyhow::Result<ImportResult> {
    let physical_content = remap_downloads_path(content_path);

    // Find the target directory and a naming reference from existing book_files.
    // First find ANY existing book to determine the target directory, then pick a
    // reference file (preferring one outside expected_volumes for naming consistency).
    let any_row = sqlx::query(
        "SELECT bf.abs_path, b.volume \
         FROM book_files bf \
         JOIN books b ON b.id = bf.book_id \
         LEFT JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 \
           AND norm_text(s.name) = norm_text($2) \
           AND b.volume IS NOT NULL \
         ORDER BY b.volume DESC LIMIT 1",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await?;

    let lib_row = sqlx::query("SELECT root_path FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_one(pool)
        .await?;
    let root_path: String = lib_row.get("root_path");
    let physical_root = remap_libraries_path(&root_path);

    let (target_dir, reference) = if let Some(r) = any_row {
        let abs_path: String = r.get("abs_path");
        let volume: i32 = r.get("volume");
        let physical = remap_libraries_path(&abs_path);
        let parent = std::path::Path::new(&physical)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(physical);
        info!(
            "[IMPORT] DB reference found: {} (volume {}), target_dir={}",
            abs_path, volume, parent
        );
        (parent, Some((abs_path, volume)))
    } else {
        // No existing files in DB: look for an existing directory (case-insensitive)
        // inside the library root, then fall back to creating one.
        info!(
            "[IMPORT] No DB reference for series '{}' in library {}",
            series_name, library_id
        );
        let dir = find_existing_series_dir(&physical_root, series_name)
            .unwrap_or_else(|| format!("{}/{}", physical_root.trim_end_matches('/'), series_name));
        info!("[IMPORT] Target directory: {}", dir);
        (dir, None)
    };

    std::fs::create_dir_all(&target_dir)?;

    let mut expected_set: std::collections::HashSet<i32> =
        expected_volumes.iter().copied().collect();

    // If DB didn't give us a reference, try to find one from existing files on disk
    let reference = if reference.is_some() {
        reference
    } else {
        info!("[IMPORT] Trying disk fallback in {}", target_dir);
        let disk_ref = find_reference_from_disk(&target_dir, &expected_set);
        if disk_ref.is_none() {
            info!("[IMPORT] No disk reference found either, using default naming");
        }
        disk_ref
    };

    info!("[IMPORT] Final reference: {:?}", reference);
    info!(
        "[IMPORT] Expected volumes (from download): {:?}",
        expected_set
    );
    info!("[IMPORT] Physical content path: {}", physical_content);

    // Collect all candidate files, then deduplicate by volume keeping the best format.
    // Priority: cbz > cbr > pdf > epub
    let all_source_files = collect_book_files(&physical_content)?;
    info!(
        "[IMPORT] Found {} source files: {:?}",
        all_source_files.len(),
        all_source_files
    );
    for f in &all_source_files {
        let fname = std::path::Path::new(f)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let extracted = extract_volumes(fname);
        info!(
            "[IMPORT]   '{}' => extracted volumes: {:?}",
            fname, extracted
        );
    }

    // Expand expected_set: also import volumes found in the torrent that are missing
    // from the library (not just the volumes originally expected by the download detection)
    if !replace_existing && !expected_set.is_empty() {
        let all_torrent_volumes: std::collections::HashSet<i32> = all_source_files
            .iter()
            .flat_map(|f| {
                let fname = std::path::Path::new(f)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                extract_volumes(fname)
            })
            .collect();

        if !all_torrent_volumes.is_empty() {
            let existing_volumes: Vec<i32> = sqlx::query_scalar(
                "SELECT DISTINCT b.volume FROM books b \
                 LEFT JOIN series s ON s.id = b.series_id \
                 WHERE b.library_id = $1 AND norm_text(s.name) = norm_text($2) AND b.volume IS NOT NULL"
            )
            .bind(library_id)
            .bind(series_name)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let existing_set: std::collections::HashSet<i32> =
                existing_volumes.into_iter().collect();
            let missing_in_library: Vec<i32> = all_torrent_volumes
                .iter()
                .filter(|v| !existing_set.contains(v))
                .copied()
                .collect();

            if !missing_in_library.is_empty() {
                info!(
                    "[IMPORT] Expanding expected_set with {} additional missing volumes: {:?}",
                    missing_in_library.len(),
                    missing_in_library
                );
                expected_set.extend(missing_in_library);
            }
        }
    }
    info!("[IMPORT] Final expected volumes: {:?}", expected_set);

    // In replace mode, don't filter by expected volumes — import all files
    let dedup_set = if replace_existing {
        std::collections::HashSet::new()
    } else {
        expected_set.clone()
    };
    let source_files = deduplicate_by_format(&all_source_files, &dedup_set);
    info!("[IMPORT] After dedup: {} files kept", source_files.len());

    let total_source_files = all_source_files.len();
    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    let mut used_destinations: std::collections::HashSet<String> = std::collections::HashSet::new();
    let rename_templates = load_rename_templates(pool).await?;
    let max_template_volume = expected_set
        .iter()
        .copied()
        .chain(
            all_source_files
                .iter()
                .flat_map(|f| extract_volumes(filename_from_path(f))),
        )
        .max()
        .unwrap_or(0);
    let max_template_volume =
        load_rename_max_volume(pool, library_id, series_name, max_template_volume).await?;

    // Existing files per volume, so re-downloading an already-owned volume replaces the
    // older file (keep the most recent) instead of creating a duplicate.
    let existing_by_volume = load_existing_files_by_volume(pool, library_id, series_name).await?;

    for source_path in &source_files {
        let filename = std::path::Path::new(&source_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let ext = std::path::Path::new(&source_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let all_extracted = extract_volumes(filename);
        let matched: Vec<i32> = if expected_set.is_empty() || replace_existing {
            all_extracted.clone()
        } else {
            all_extracted
                .iter()
                .copied()
                .filter(|v| expected_set.contains(v))
                .collect()
        };

        if matched.is_empty() && !expected_set.is_empty() && !replace_existing {
            info!(
                "[IMPORT] Skipping '{}' (extracted volumes {:?}, none in expected set)",
                filename, all_extracted
            );
            skipped.push(SkippedFile {
                filename: filename.to_string(),
                reason: "no matching expected volume".to_string(),
                extracted_volumes: all_extracted,
            });
            continue;
        }
        if matched.is_empty() && expected_set.is_empty() && all_extracted.is_empty() {
            // One-shot / standalone book (no volume number, no expected volumes)
            // Import it as-is without renaming — keep original filename
            info!(
                "[IMPORT] No volume detected for '{}', importing as one-shot",
                filename
            );
        }

        let target_filename = if matched.is_empty() {
            // One-shot / standalone: keep original filename
            filename.to_string()
        } else if matched.len() == 1 {
            // Single volume: apply naming pattern from reference
            let vol = matched[0];
            let generated = if let Some((ref ref_path, ref_vol)) = reference {
                let built = build_target_filename(ref_path, ref_vol, vol, ext);
                info!("[IMPORT] build_target_filename(ref={}, ref_vol={}, new_vol={}, ext={}) => {:?} (source='{}')",
                    ref_path, ref_vol, vol, ext, built, filename);
                built.unwrap_or_else(|| filename.to_string())
            } else {
                let built = build_target_filename_from_template(
                    &rename_templates,
                    series_name,
                    source_path,
                    vol,
                    ext,
                    max_template_volume,
                );
                info!(
                    "[IMPORT] No reference, template filename for '{}' vol {} => {:?}",
                    filename, vol, built
                );
                built.unwrap_or_else(|| filename.to_string())
            };

            // If this destination was already used in this batch, keep original filename
            if used_destinations.contains(&generated) {
                info!("[IMPORT] Destination '{}' already used in this batch, keeping original filename '{}'", generated, filename);
                filename.to_string()
            } else {
                generated
            }
        } else {
            // Multi-volume pack: keep original filename (scanner handles ranges)
            filename.to_string()
        };

        let dest = format!("{}/{}", target_dir, target_filename);

        // Re-download guard: if the library already owns this volume, keep the most recent
        // file. Only single-volume regular books participate (HS/one-shots/integral do not
        // share the numbering). Replace-all mode bypasses the comparison so an explicit
        // replace always wins.
        let conflict = match matched.as_slice() {
            [vol] if is_incoming_regular(source_path, std::path::Path::new(&physical_root)) => {
                existing_by_volume
                    .get(vol)
                    .map(|paths| resolve_volume_conflict(source_path, paths, replace_existing))
                    .unwrap_or(VolumeConflict::None)
            }
            _ => VolumeConflict::None,
        };

        if conflict == VolumeConflict::KeepExisting {
            info!(
                "[IMPORT] Skipping '{}' (volume {:?}): a more recent library file already exists",
                filename, matched
            );
            skipped.push(SkippedFile {
                filename: filename.to_string(),
                reason: "existing volume is more recent".to_string(),
                extracted_volumes: matched.clone(),
            });
            continue;
        }

        // Untracked filename collision (e.g. a one-shot, or a file the scanner has not yet
        // indexed): leave the existing file untouched and count it as already imported.
        if std::path::Path::new(&dest).exists()
            && !replace_existing
            && conflict == VolumeConflict::None
        {
            info!(
                "[IMPORT] Already exists '{}' → '{}', counting as imported",
                filename, dest
            );
            imported.push(ImportedFile {
                volume: matched.iter().min().copied().unwrap_or(0),
                source: source_path.clone(),
                destination: unmap_libraries_path(&dest),
                already_existed: true,
            });
            continue;
        }

        move_file(source_path, &dest)?;
        used_destinations.insert(target_filename);
        if let VolumeConflict::ReplaceExisting(paths) = &conflict {
            remove_superseded_files(paths, &dest);
        }
        info!(
            "[IMPORT] Imported '{}' [{:?}] → {}",
            filename, matched, dest
        );

        imported.push(ImportedFile {
            volume: matched.iter().min().copied().unwrap_or(0),
            source: source_path.clone(),
            destination: unmap_libraries_path(&dest),
            already_existed: false,
        });
    }

    // Sanity check: warn if many source files collapsed into few volumes
    // (symptom of a volume extraction bug)
    let source_count = collect_book_files(&physical_content)
        .map(|f| f.len())
        .unwrap_or(0);
    let unique_volumes: std::collections::HashSet<i32> =
        imported.iter().map(|f| f.volume).collect();
    if source_count > 5 && !unique_volumes.is_empty() && source_count > unique_volumes.len() * 3 {
        warn!(
            "[IMPORT] Suspicious: {} source files mapped to only {} unique volumes ({:?}). \
             Possible volume extraction issue for series '{}'",
            source_count,
            unique_volumes.len(),
            {
                let mut v: Vec<i32> = unique_volumes.into_iter().collect();
                v.sort();
                v
            },
            series_name,
        );
    }

    // Also track dedup-filtered files (in all_source_files but not in source_files)
    let source_set: std::collections::HashSet<&String> = source_files.iter().collect();
    for f in &all_source_files {
        if !source_set.contains(f) {
            let fname = std::path::Path::new(f)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let vols = extract_volumes(fname);
            skipped.push(SkippedFile {
                filename: fname.to_string(),
                reason: "duplicate format (lower priority)".to_string(),
                extracted_volumes: vols,
            });
        }
    }

    // Cleanup: remove old single-volume files in target_dir for volumes we just imported
    // under a different filename (e.g. old naming convention left behind when dest path differed).
    // Only in replace mode — the user explicitly asked to replace existing files.
    if replace_existing {
        let imported_volume_map: std::collections::HashMap<i32, String> = imported
            .iter()
            .filter(|f| f.volume > 0)
            .map(|f| {
                let dest_filename = std::path::Path::new(&f.destination)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                (f.volume, dest_filename)
            })
            .collect();

        if !imported_volume_map.is_empty() {
            if let Ok(entries) = std::fs::read_dir(&target_dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if !entry_path.is_file() {
                        continue;
                    }
                    let fname = entry_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    let vols = extract_volumes(&fname);
                    if vols.len() == 1 {
                        let vol = vols[0];
                        if let Some(new_fname) = imported_volume_map.get(&vol) {
                            if &fname != new_fname {
                                info!(
                                    "[IMPORT] Removing old file for volume {}: {:?}",
                                    vol, entry_path
                                );
                                if let Err(e) = std::fs::remove_file(&entry_path) {
                                    warn!(
                                        "[IMPORT] Failed to remove old file {:?}: {}",
                                        entry_path, e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(ImportResult {
        imported,
        skipped,
        total_source_files,
    })
}

// ─── Volume replacement ───────────────────────────────────────────────────────

/// What to do when an incoming file targets a volume already present in the library.
#[derive(Debug, PartialEq)]
pub(super) enum VolumeConflict {
    /// No library file for this volume — import normally.
    None,
    /// A library file for this volume is at least as recent — keep it, skip the incoming file.
    KeepExisting,
    /// The incoming file is more recent — import it and remove these superseded physical paths.
    ReplaceExisting(Vec<String>),
}

fn file_mtime(path: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// Decide whether an incoming volume file should replace the library file for the same volume.
/// The most recent file wins; `existing_abs_paths` are DB paths, remapped before stat-ing.
/// `force_replace` (explicit replace-all import) makes the incoming file win unconditionally.
pub(super) fn resolve_volume_conflict(
    incoming: &str,
    existing_abs_paths: &[String],
    force_replace: bool,
) -> VolumeConflict {
    let existing: Vec<String> = existing_abs_paths
        .iter()
        .map(|p| remap_libraries_path(p))
        .collect();

    if existing.is_empty() {
        return VolumeConflict::None;
    }

    if force_replace {
        return VolumeConflict::ReplaceExisting(existing);
    }

    let newest_existing = existing.iter().filter_map(|p| file_mtime(p)).max();

    match (file_mtime(incoming), newest_existing) {
        (Some(incoming_mtime), Some(existing_mtime)) if incoming_mtime <= existing_mtime => {
            VolumeConflict::KeepExisting
        }
        // Incoming wins when strictly newer, or when either file can't be stat-ed
        // (missing library file, unreadable source) — the stale row gets cleaned up.
        _ => VolumeConflict::ReplaceExisting(existing),
    }
}

/// Whether the incoming file is a regular numbered volume (HS/one-shots/integral excluded).
fn is_incoming_regular(source_path: &str, library_root: &std::path::Path) -> bool {
    let path = std::path::Path::new(source_path);
    let Some(format) = detect_format(path) else {
        return false;
    };
    parse_metadata_fast(path, format, library_root).volume_type == VolumeType::Regular
}

fn remove_superseded_files(paths: &[String], dest: &str) {
    for path in paths {
        if path == dest || is_same_file(path, dest) {
            continue;
        }
        if !std::path::Path::new(path).exists() {
            continue;
        }
        info!("[IMPORT] Removing superseded file: {}", path);
        if let Err(e) = std::fs::remove_file(path) {
            warn!("[IMPORT] Failed to remove superseded file {}: {}", path, e);
        }
    }
}

/// Same physical file, so a DB path that differs only by case (case-insensitive filesystems)
/// or via a symlink never causes the file we just wrote to be deleted.
#[cfg(unix)]
fn is_same_file(a: &str, b: &str) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(ma), Ok(mb)) => ma.dev() == mb.dev() && ma.ino() == mb.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn is_same_file(a: &str, b: &str) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(pa), Ok(pb)) => pa == pb,
        _ => false,
    }
}

/// Physical paths of regular books already present for a series, grouped by volume.
async fn load_existing_files_by_volume(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
) -> anyhow::Result<std::collections::HashMap<i32, Vec<String>>> {
    let rows = sqlx::query(
        "SELECT b.volume, bf.abs_path \
         FROM books b \
         JOIN book_files bf ON bf.book_id = b.id \
         JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 \
           AND norm_text(s.name) = norm_text($2) \
           AND b.volume IS NOT NULL \
           AND b.volume_type = 'regular'",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_all(pool)
    .await?;

    let mut by_volume: std::collections::HashMap<i32, Vec<String>> =
        std::collections::HashMap::new();
    for row in rows {
        by_volume
            .entry(row.get("volume"))
            .or_default()
            .push(row.get("abs_path"));
    }
    Ok(by_volume)
}

// ─── Directory matching ───────────────────────────────────────────────────────

/// Find an existing directory in `root` whose name matches `series_name`
/// case-insensitively and accent-insensitively (e.g. "les géants" matches "les geants").
pub(super) fn find_existing_series_dir(root: &str, series_name: &str) -> Option<String> {
    let target_norm = fold_accents(&series_name.to_lowercase());
    let entries = std::fs::read_dir(root).ok()?;
    let mut best: Option<(String, bool)> = None; // (path, is_exact_case_match)
    for entry in entries.flatten() {
        if !entry.file_type().ok().is_some_and(|t| t.is_dir()) {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let name_lower = name_str.to_lowercase();
        let name_norm = fold_accents(&name_lower);
        if name_norm == target_norm {
            let path = entry.path().to_string_lossy().into_owned();
            let exact = name_lower == series_name.to_lowercase();
            info!(
                "[IMPORT] Found existing directory (normalized match): {} (exact={})",
                path, exact
            );
            // Prefer exact case match over accent-stripped match
            if exact || best.is_none() {
                best = Some((path, exact));
            }
        }
    }
    best.map(|(p, _)| p)
}

// ─── Format deduplication ─────────────────────────────────────────────────────

/// When a download contains the same volume in multiple formats (e.g. T01.cbz and T01.pdf),
/// keep only the best format per volume. Priority: cbz > cbr > pdf > epub.
pub(super) fn format_priority(ext: &str) -> u8 {
    match ext.to_ascii_lowercase().as_str() {
        "cbz" => 0,
        "cbr" => 1,
        "pdf" => 2,
        "epub" => 3,
        _ => 4,
    }
}

pub(super) fn deduplicate_by_format(
    files: &[String],
    expected_set: &std::collections::HashSet<i32>,
) -> Vec<String> {
    // Map: volume -> (priority, file_path)
    let mut best_per_vol: std::collections::HashMap<i32, (u8, &str)> =
        std::collections::HashMap::new();
    let mut multi_volume_files: Vec<&str> = Vec::new();

    for path in files {
        let filename = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let all_volumes = extract_volumes(filename);
        // If expected_set is empty, keep all files (no filtering)
        let volumes: Vec<i32> = if expected_set.is_empty() {
            all_volumes
        } else {
            all_volumes
                .into_iter()
                .filter(|v| expected_set.contains(v))
                .collect()
        };

        if volumes.is_empty() && !expected_set.is_empty() {
            continue;
        }
        // For files with no extracted volume and empty expected_set, keep them
        if volumes.is_empty() && expected_set.is_empty() {
            multi_volume_files.push(path);
            continue;
        }

        if volumes.len() > 1 {
            // Multi-volume packs are always kept (no dedup possible)
            multi_volume_files.push(path);
            continue;
        }

        let vol = volumes[0];
        let prio = format_priority(ext);
        if best_per_vol.get(&vol).is_none_or(|(p, _)| prio < *p) {
            best_per_vol.insert(vol, (prio, path));
        }
    }

    let mut result: Vec<String> = best_per_vol
        .into_values()
        .map(|(_, path)| path.to_string())
        .collect();
    result.extend(multi_volume_files.into_iter().map(|s| s.to_string()));
    result
}

// ─── Reference from disk ──────────────────────────────────────────────────────

/// Scan a directory for book files and pick the one with the highest extracted volume
/// as a naming reference, excluding certain volumes. Returns (abs_path, volume).
fn find_reference_from_disk(
    dir: &str,
    exclude_volumes: &std::collections::HashSet<i32>,
) -> Option<(String, i32)> {
    let extensions = ["cbz", "cbr", "pdf", "epub"];
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(String, i32)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !extensions.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
            continue;
        }
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let volumes = extract_volumes(filename);
        if let Some(&vol) = volumes.iter().max() {
            if exclude_volumes.contains(&vol) {
                continue;
            }
            if best.as_ref().is_none_or(|(_, v)| vol > *v) {
                best = Some((path.to_string_lossy().into_owned(), vol));
            }
        }
    }

    if let Some((ref path, vol)) = best {
        info!("[IMPORT] Found disk reference: {} (volume {})", path, vol);
    }
    best
}

// ─── Filesystem helpers ───────────────────────────────────────────────────────

fn collect_book_files(root: &str) -> anyhow::Result<Vec<String>> {
    let extensions = ["cbz", "cbr", "pdf", "epub"];
    let mut files = Vec::new();
    collect_recursive(root, &extensions, &mut files)?;
    Ok(files)
}

fn collect_recursive(path: &str, exts: &[&str], out: &mut Vec<String>) -> anyhow::Result<()> {
    let p = std::path::Path::new(path);
    if p.is_file() {
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if exts.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
                out.push(path.to_string());
            }
        }
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path().to_string_lossy().into_owned();
        if entry.path().is_dir() {
            collect_recursive(&child, exts, out)?;
        } else if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            if exts.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
                out.push(child);
            }
        }
    }
    Ok(())
}

fn filename_from_path(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
}

pub(super) fn move_file(src: &str, dst: &str) -> anyhow::Result<()> {
    if std::fs::rename(src, dst).is_err() {
        // Cross-device link: copy then remove
        std::fs::copy(src, dst)?;
        std::fs::remove_file(src)?;
    }
    Ok(())
}

// ─── Path remapping ───────────────────────────────────────────────────────────

pub(super) fn remap_downloads_path(path: &str) -> String {
    if let Ok(root) = std::env::var("DOWNLOADS_PATH") {
        if path.starts_with("/downloads") {
            return path.replacen("/downloads", &root, 1);
        }
    }
    path.to_string()
}

// ─── Naming helpers ───────────────────────────────────────────────────────────

/// Infer the target filename for `new_volume` by reusing the naming pattern from
/// `reference_abs_path` (which stores `reference_volume`).
///
/// Strategy: find the last digit run in the reference stem that parses to `reference_volume`,
/// replace it with `new_volume` formatted to the same width (preserves leading zeros).
///
/// Example:
///   reference = "/libraries/bd/One Piece/One Piece - T104.cbz", reference_volume = 104
///   new_volume = 105, source_ext = "cbz"
///   → "One Piece - T105.cbz"
pub(super) fn build_target_filename(
    reference_abs_path: &str,
    reference_volume: i32,
    new_volume: i32,
    source_ext: &str,
) -> Option<String> {
    let path = std::path::Path::new(reference_abs_path);
    let stem = path.file_stem()?.to_str()?;
    let ref_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("cbz");
    let target_ext = if source_ext.is_empty() {
        ref_ext
    } else {
        source_ext
    };

    // Iterate over raw bytes to find ASCII digit runs (safe: continuation bytes of
    // multi-byte UTF-8 sequences are never in the ASCII digit range 0x30–0x39).
    let bytes = stem.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut last_match: Option<(usize, usize)> = None; // byte offsets into `stem`

    while i < len {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let digit_str = &stem[start..i]; // valid UTF-8: all ASCII digits
            if let Ok(n) = digit_str.parse::<i32>() {
                if n == reference_volume {
                    last_match = Some((start, i));
                }
            }
        } else {
            i += 1;
        }
    }

    let (start, end) = last_match?;
    let digit_width = end - start;
    let new_digits = format!("{:0>width$}", new_volume, width = digit_width);
    // Truncate after the volume number (remove suffixes like ".FR-NoFace696")
    let new_stem = format!("{}{}", &stem[..start], new_digits);
    Some(format!("{}.{}", new_stem, target_ext))
}

fn build_target_filename_from_template(
    templates: &crate::books::rename::RenameTemplates,
    series_name: &str,
    source_path: &str,
    volume: i32,
    source_ext: &str,
    max_volume: i64,
) -> Option<String> {
    let path = std::path::Path::new(source_path);
    let format = detect_format(path)?;
    let library_root = path.parent().unwrap_or_else(|| std::path::Path::new(""));
    let parsed = parse_metadata_fast(path, format, library_root);
    let book = RenameTemplateBook {
        title: parsed.title,
        authors: Vec::new(),
        volume: Some(volume),
        volume_type: parsed.volume_type.as_str().to_string(),
        publish_date: None,
        isbn: None,
        abs_path: source_path.to_string(),
    };

    render_rename_filename(templates, series_name, &book, max_volume, source_ext)
}

#[cfg(test)]
#[path = "tests/torrent_import.rs"]
mod tests;
