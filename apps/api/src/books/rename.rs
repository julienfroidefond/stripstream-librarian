use axum::{extract::{Path, State}, Json};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use stripstream_core::fingerprint::compute_fingerprint;
use stripstream_core::paths::remap_libraries_path;

use crate::error::ApiError;
use crate::state::AppState;

// ─── Request / Response types ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RenameRequest {
    /// Template pattern. If null, uses the saved `rename_format` setting.
    pub format: Option<String>,
    /// "preview" for dry-run, "execute" to perform renames.
    pub mode: RenameMode,
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RenameMode {
    Preview,
    Execute,
}

#[derive(Serialize)]
pub struct RenameResponse {
    pub series_id: Uuid,
    pub renames: Vec<RenameEntry>,
    pub errors: Vec<RenameError>,
    pub executed: bool,
}

#[derive(Serialize)]
pub struct RenameEntry {
    pub book_id: Uuid,
    pub old_filename: String,
    pub new_filename: String,
    pub old_path: String,
    pub new_path: String,
    pub changed: bool,
}

#[derive(Serialize)]
pub struct RenameError {
    pub book_id: Uuid,
    pub filename: String,
    pub error: String,
}

// ─── Book data for template rendering ───────────────────────────────────────

struct BookFileData {
    book_id: Uuid,
    title: String,
    authors: Vec<String>,
    volume: Option<i32>,
    publish_date: Option<String>,
    isbn: Option<String>,
    abs_path: String,
    file_id: Uuid,
}

// ─── Template engine ────────────────────────────────────────────────────────

fn apply_template(
    template: &str,
    series_name: &str,
    book: &BookFileData,
    max_volume: i64,
) -> String {
    let re = Regex::new(r"\{(\w+)\}").expect("valid regex");

    // Build variable map
    let mut vars: HashMap<&str, Option<String>> = HashMap::new();
    vars.insert("series_name", Some(series_name.to_string()));
    vars.insert("title", Some(book.title.clone()));
    vars.insert(
        "authors",
        if book.authors.is_empty() {
            Some("Unknown".to_string())
        } else {
            Some(book.authors.join(", "))
        },
    );
    // Use DB volume if available, otherwise try to extract from filename
    let effective_volume = book.volume.or_else(|| {
        let filename = std::path::Path::new(&book.abs_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        parsers::extract_volume(filename)
    });

    vars.insert(
        "volume",
        effective_volume.map(|v| v.to_string()),
    );
    vars.insert("volume_padded", effective_volume.map(|v| {
        // Auto-pad to match the digit count of the largest volume in the series
        let width = if max_volume >= 1000 {
            4
        } else if max_volume >= 100 {
            3
        } else if max_volume >= 10 {
            2
        } else {
            1
        };
        format!("{:0>width$}", v, width = width)
    }));
    vars.insert("publish_date", book.publish_date.clone());
    vars.insert("isbn", book.isbn.clone());

    // Replace tokens, cleaning up dangling separators for null values
    let result = re.replace_all(template, |caps: &regex::Captures| {
        let var_name = &caps[1];
        match vars.get(var_name) {
            Some(Some(val)) if !val.is_empty() => val.clone(),
            _ => "\x00".to_string(), // Placeholder for removal
        }
    });

    // Clean up separator segments containing null placeholders.
    // Handles patterns like " - T\0", " - \0", "\0 - ", and lone "\0".
    let cleanup = Regex::new(r"\s*-\s*[^\x00\s]*\x00[^\x00\s]*|\s*[^\x00\s]*\x00[^\x00\s]*\s*-\s*|\s*\x00\s*|\x00").expect("valid regex");
    cleanup.replace_all(&result, "").trim().to_string()
}

fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    let trimmed = sanitized.trim().trim_matches('.').to_string();

    // Truncate to 200 chars to leave room for extension
    if trimmed.len() > 200 {
        trimmed[..200].to_string()
    } else {
        trimmed
    }
}

fn deduplicate_filenames(entries: &mut [RenameEntry]) {
    let mut counts: HashMap<String, usize> = HashMap::new();

    // First pass: count occurrences
    for entry in entries.iter() {
        *counts.entry(entry.new_filename.clone()).or_insert(0) += 1;
    }

    // Second pass: add suffixes for duplicates
    let mut seen: HashMap<String, usize> = HashMap::new();
    for entry in entries.iter_mut() {
        let count = counts.get(&entry.new_filename).copied().unwrap_or(0);
        if count > 1 {
            let idx = seen.entry(entry.new_filename.clone()).or_insert(0);
            *idx += 1;
            if *idx > 1 {
                // Add suffix before extension
                if let Some(dot_pos) = entry.new_filename.rfind('.') {
                    let (stem, ext) = entry.new_filename.split_at(dot_pos);
                    entry.new_filename = format!("{} ({}){}", stem, idx, ext);
                } else {
                    entry.new_filename = format!("{} ({})", entry.new_filename, idx);
                }
                // Rebuild new_path with updated filename
                if let Some(parent) = PathBuf::from(&entry.new_path).parent() {
                    entry.new_path = parent
                        .join(&entry.new_filename)
                        .to_string_lossy()
                        .to_string();
                }
            }
        }
    }
}

// ─── Handler ────────────────────────────────────────────────────────────────

pub async fn rename_books(
    State(state): State<AppState>,
    Path(series_id): Path<Uuid>,
    Json(req): Json<RenameRequest>,
) -> Result<Json<RenameResponse>, ApiError> {
    // 1. Get the template
    let template = match req.format {
        Some(ref f) if !f.is_empty() => f.clone(),
        _ => {
            // Load from settings
            let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'rename_format'")
                .fetch_optional(&state.pool)
                .await?;
            match row {
                Some(r) => {
                    let val: serde_json::Value = r.get("value");
                    val.as_str()
                        .unwrap_or("{series_name} - T{volume_padded} - {title}")
                        .to_string()
                }
                None => "{series_name} - T{volume_padded} - {title}".to_string(),
            }
        }
    };

    // 2. Get series info
    let series_row = sqlx::query("SELECT id, name FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("series not found"))?;

    let series_name: String = series_row.get("name");

    // 3. Get all books with their files
    let rows = sqlx::query(
        r#"
        SELECT b.id AS book_id, b.title, b.authors, b.volume, b.publish_date, b.isbn,
               bf.id AS file_id, bf.abs_path, bf.format AS file_format
        FROM books b
        INNER JOIN book_files bf ON bf.book_id = b.id
        WHERE b.series_id = $1
        ORDER BY b.volume NULLS LAST, b.title
        "#,
    )
    .bind(series_id)
    .fetch_all(&state.pool)
    .await?;

    if rows.is_empty() {
        return Ok(Json(RenameResponse {
            series_id,
            renames: vec![],
            errors: vec![],
            executed: false,
        }));
    }

    // Parse book data
    let books: Vec<BookFileData> = rows
        .iter()
        .map(|row| {
            let authors_raw: Vec<String> = row.get("authors");
            BookFileData {
                book_id: row.get("book_id"),
                title: row.get("title"),
                authors: authors_raw,
                volume: row.get("volume"),
                publish_date: row.get("publish_date"),
                isbn: row.get("isbn"),
                abs_path: row.get("abs_path"),
                file_id: row.get("file_id"),
            }
        })
        .collect();

    // Compute max volume for padding
    let max_volume: i64 = books
        .iter()
        .filter_map(|b| b.volume.map(|v| v as i64))
        .max()
        .unwrap_or(0);

    // 4. Generate rename entries
    let mut entries: Vec<RenameEntry> = books
        .iter()
        .map(|book| {
            let old_path = PathBuf::from(&book.abs_path);
            let old_filename = old_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();

            let extension = old_path
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();

            let new_stem = apply_template(&template, &series_name, book, max_volume);
            let new_stem = sanitize_filename(&new_stem);
            // Avoid double extension (e.g. "Title.cbr" + ".cbr" → "Title.cbr")
            let new_filename = if !extension.is_empty()
                && new_stem.to_lowercase().ends_with(&extension.to_lowercase())
            {
                new_stem
            } else {
                format!("{}{}", new_stem, extension)
            };

            let new_path = old_path
                .parent()
                .map(|p| p.join(&new_filename).to_string_lossy().to_string())
                .unwrap_or_else(|| new_filename.clone());

            let changed = old_filename != new_filename;

            RenameEntry {
                book_id: book.book_id,
                old_filename,
                new_filename,
                old_path: book.abs_path.clone(),
                new_path,
                changed,
            }
        })
        .collect();

    // 5. Deduplicate
    deduplicate_filenames(&mut entries);

    // Recheck changed status after dedup
    for entry in &mut entries {
        entry.changed = entry.old_filename != entry.new_filename;
    }

    // 6. Preview mode: return early
    if req.mode == RenameMode::Preview {
        return Ok(Json(RenameResponse {
            series_id,
            renames: entries,
            errors: vec![],
            executed: false,
        }));
    }

    // 7. Execute mode
    let changed_entries: Vec<&RenameEntry> = entries.iter().filter(|e| e.changed).collect();

    if changed_entries.is_empty() {
        return Ok(Json(RenameResponse {
            series_id,
            renames: entries,
            errors: vec![],
            executed: true,
        }));
    }

    // Check for conflicts with existing files on disk (outside the batch)
    let batch_old_paths: Vec<String> = entries.iter().map(|e| e.old_path.clone()).collect();
    for entry in &changed_entries {
        let physical_new = remap_libraries_path(&entry.new_path);
        let physical_new_path = std::path::Path::new(&physical_new);
        if physical_new_path.exists() && !batch_old_paths.contains(&entry.new_path) {
            return Err(ApiError::bad_request(format!(
                "target file already exists: {}",
                entry.new_filename
            )));
        }
    }

    // Build a lookup from book_id to file_id
    let file_id_map: HashMap<Uuid, Uuid> = books
        .iter()
        .map(|b| (b.book_id, b.file_id))
        .collect();

    // Perform renames in a transaction
    let mut tx = state.pool.begin().await?;
    let mut renamed_files: Vec<(String, String)> = Vec::new(); // (physical_old, physical_new) for rollback
    let errors: Vec<RenameError> = Vec::new();

    for entry in &changed_entries {
        let physical_old = remap_libraries_path(&entry.old_path);
        let physical_new = remap_libraries_path(&entry.new_path);

        // Rename on filesystem
        let old_clone = physical_old.clone();
        let new_clone = physical_new.clone();
        let rename_result =
            tokio::task::spawn_blocking(move || std::fs::rename(&old_clone, &new_clone)).await;

        match rename_result {
            Ok(Ok(())) => {
                renamed_files.push((physical_old.clone(), physical_new.clone()));

                // Recompute fingerprint
                let new_path = std::path::Path::new(&physical_new);
                let meta_result =
                    tokio::task::spawn_blocking({
                        let new_physical = physical_new.clone();
                        move || std::fs::metadata(&new_physical)
                    })
                    .await;

                let (new_fingerprint, new_mtime) = match meta_result {
                    Ok(Ok(meta)) => {
                        let mtime: DateTime<Utc> = meta
                            .modified()
                            .map(DateTime::<Utc>::from)
                            .unwrap_or_else(|_| Utc::now());
                        let fp = compute_fingerprint(new_path, meta.len(), &mtime)
                            .unwrap_or_default();
                        (fp, mtime)
                    }
                    _ => (String::new(), Utc::now()),
                };

                // Update book_files
                let file_id = file_id_map.get(&entry.book_id);
                if let Some(fid) = file_id {
                    sqlx::query(
                        "UPDATE book_files SET abs_path = $1, fingerprint = $2, mtime = $3 WHERE id = $4",
                    )
                    .bind(&entry.new_path)
                    .bind(&new_fingerprint)
                    .bind(new_mtime)
                    .bind(fid)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        tracing::error!("[RENAME] DB update failed for {}: {}", entry.new_filename, e);
                        e
                    })?;
                }

                // Update book title and volume from the new filename
                let new_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let new_volume = parsers::extract_volume(new_stem);
                sqlx::query(
                    "UPDATE books SET title = $1, volume = $2, updated_at = NOW() WHERE id = $3",
                )
                .bind(new_stem)
                .bind(new_volume)
                .bind(entry.book_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    tracing::error!("[RENAME] Book update failed for {}: {}", entry.new_filename, e);
                    e
                })?;
            }
            Ok(Err(e)) => {
                tracing::error!(
                    "[RENAME] Failed to rename {} -> {}: {}",
                    physical_old,
                    physical_new,
                    e
                );
                // Rollback already renamed files
                for (old, new) in renamed_files.iter().rev() {
                    let _ = std::fs::rename(new, old);
                }
                return Err(ApiError::internal(format!(
                    "failed to rename {}: {}. All renames rolled back.",
                    entry.old_filename, e
                )));
            }
            Err(e) => {
                // spawn_blocking panicked
                for (old, new) in renamed_files.iter().rev() {
                    let _ = std::fs::rename(new, old);
                }
                return Err(ApiError::internal(format!(
                    "rename task panicked for {}: {}",
                    entry.old_filename, e
                )));
            }
        }
    }

    // Commit transaction
    tx.commit().await?;

    tracing::info!(
        "[RENAME] Successfully renamed {} files in series '{}'",
        renamed_files.len(),
        series_name
    );

    Ok(Json(RenameResponse {
        series_id,
        renames: entries,
        errors,
        executed: true,
    }))
}

#[cfg(test)]
#[path = "rename_tests.rs"]
mod tests;
