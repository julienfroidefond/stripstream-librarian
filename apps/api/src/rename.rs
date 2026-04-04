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
mod tests {
    use super::*;

    fn make_book(
        title: &str,
        volume: Option<i32>,
        authors: Vec<&str>,
        abs_path: &str,
    ) -> BookFileData {
        BookFileData {
            book_id: Uuid::new_v4(),
            title: title.to_string(),
            authors: authors.into_iter().map(String::from).collect(),
            volume,
            publish_date: None,
            isbn: None,
            abs_path: abs_path.to_string(),
            file_id: Uuid::new_v4(),
        }
    }

    // ── apply_template ──────────────────────────────────────────────────

    #[test]
    fn basic_template() {
        let book = make_book("Son Goku et ses amis", Some(1), vec!["Akira Toriyama"], "/libraries/BD/old.cbz");
        let result = apply_template("{series_name} - T{volume_padded} - {title}", "Dragon Ball", &book, 42);
        assert_eq!(result, "Dragon Ball - T01 - Son Goku et ses amis");
    }

    #[test]
    fn volume_padding_two_digits() {
        let book = make_book("Title", Some(3), vec![], "/libraries/BD/old.cbz");
        let result = apply_template("{volume_padded}", "S", &book, 25);
        assert_eq!(result, "03");
    }

    #[test]
    fn volume_padding_three_digits() {
        let book = make_book("Title", Some(5), vec![], "/libraries/BD/old.cbz");
        let result = apply_template("{volume_padded}", "S", &book, 150);
        assert_eq!(result, "005");
    }

    #[test]
    fn volume_padding_single_digit_series() {
        let book = make_book("Title", Some(3), vec![], "/libraries/BD/old.cbz");
        let result = apply_template("{volume_padded}", "S", &book, 5);
        assert_eq!(result, "3");
    }

    #[test]
    fn volume_padding_four_digits() {
        let book = make_book("Title", Some(5), vec![], "/libraries/BD/old.cbz");
        let result = apply_template("{volume_padded}", "S", &book, 1200);
        assert_eq!(result, "0005");
    }

    #[test]
    fn null_volume_removes_entire_segment() {
        let book = make_book("Title", None, vec!["Author"], "/libraries/BD/old.cbz");
        let result = apply_template("{series_name} - T{volume_padded} - {title}", "Dragon Ball", &book, 10);
        assert_eq!(result, "Dragon Ball - Title");
    }

    #[test]
    fn null_volume_removes_segment_without_prefix() {
        let book = make_book("Title", None, vec!["Author"], "/libraries/BD/old.cbz");
        let result = apply_template("{series_name} - {volume_padded} - {title}", "Dragon Ball", &book, 10);
        assert_eq!(result, "Dragon Ball - Title");
    }

    #[test]
    fn empty_authors_fallback_to_unknown() {
        let book = make_book("Title", Some(1), vec![], "/libraries/BD/old.cbz");
        let result = apply_template("{authors}", "S", &book, 1);
        assert_eq!(result, "Unknown");
    }

    #[test]
    fn multiple_authors_joined() {
        let book = make_book("Title", Some(1), vec!["Author A", "Author B"], "/libraries/BD/old.cbz");
        let result = apply_template("{authors}", "S", &book, 1);
        assert_eq!(result, "Author A, Author B");
    }

    #[test]
    fn unknown_variable_removed() {
        let book = make_book("Title", Some(1), vec![], "/libraries/BD/old.cbz");
        let result = apply_template("{series_name} - {unknown_var} - {title}", "S", &book, 1);
        assert_eq!(result, "S - Title");
    }

    #[test]
    fn publish_date_and_isbn() {
        let mut book = make_book("Title", Some(1), vec![], "/libraries/BD/old.cbz");
        book.publish_date = Some("2023-01-15".to_string());
        book.isbn = Some("978-123".to_string());
        let result = apply_template("{title} ({publish_date}) [{isbn}]", "S", &book, 1);
        assert_eq!(result, "Title (2023-01-15) [978-123]");
    }

    #[test]
    fn null_publish_date_cleaned_up() {
        let book = make_book("Title", Some(1), vec![], "/libraries/BD/old.cbz");
        let result = apply_template("{title} - {publish_date}", "S", &book, 1);
        assert_eq!(result, "Title");
    }

    // ── sanitize_filename ───────────────────────────────────────────────

    #[test]
    fn sanitize_replaces_forbidden_chars() {
        assert_eq!(sanitize_filename("a/b\\c:d*e?f\"g<h>i|j"), "a_b_c_d_e_f_g_h_i_j");
    }

    #[test]
    fn sanitize_trims_whitespace_and_dots() {
        assert_eq!(sanitize_filename("  ..hello world..  "), "hello world");
    }

    #[test]
    fn sanitize_truncates_long_names() {
        let long_name = "a".repeat(250);
        let result = sanitize_filename(&long_name);
        assert_eq!(result.len(), 200);
    }

    #[test]
    fn sanitize_normal_name_unchanged() {
        assert_eq!(sanitize_filename("Dragon Ball - T01 - Son Goku"), "Dragon Ball - T01 - Son Goku");
    }

    // ── deduplicate_filenames ───────────────────────────────────────────

    fn make_entry(old: &str, new: &str, new_path: &str) -> RenameEntry {
        RenameEntry {
            book_id: Uuid::new_v4(),
            old_filename: old.to_string(),
            new_filename: new.to_string(),
            old_path: format!("/libraries/BD/{}", old),
            new_path: new_path.to_string(),
            changed: old != new,
        }
    }

    #[test]
    fn no_duplicates_unchanged() {
        let mut entries = vec![
            make_entry("old1.cbz", "new1.cbz", "/libraries/BD/new1.cbz"),
            make_entry("old2.cbz", "new2.cbz", "/libraries/BD/new2.cbz"),
        ];
        deduplicate_filenames(&mut entries);
        assert_eq!(entries[0].new_filename, "new1.cbz");
        assert_eq!(entries[1].new_filename, "new2.cbz");
    }

    #[test]
    fn duplicates_get_suffix() {
        let mut entries = vec![
            make_entry("old1.cbz", "same.cbz", "/libraries/BD/same.cbz"),
            make_entry("old2.cbz", "same.cbz", "/libraries/BD/same.cbz"),
            make_entry("old3.cbz", "same.cbz", "/libraries/BD/same.cbz"),
        ];
        deduplicate_filenames(&mut entries);
        assert_eq!(entries[0].new_filename, "same.cbz");
        assert_eq!(entries[1].new_filename, "same (2).cbz");
        assert_eq!(entries[2].new_filename, "same (3).cbz");
    }

    #[test]
    fn duplicate_without_extension() {
        let mut entries = vec![
            make_entry("old1", "same", "/libraries/BD/same"),
            make_entry("old2", "same", "/libraries/BD/same"),
        ];
        deduplicate_filenames(&mut entries);
        assert_eq!(entries[0].new_filename, "same");
        assert_eq!(entries[1].new_filename, "same (2)");
    }

    #[test]
    fn dedup_updates_new_path() {
        let mut entries = vec![
            make_entry("old1.cbz", "same.cbz", "/libraries/BD/same.cbz"),
            make_entry("old2.cbz", "same.cbz", "/libraries/BD/same.cbz"),
        ];
        deduplicate_filenames(&mut entries);
        assert!(entries[1].new_path.ends_with("same (2).cbz"));
    }

    #[test]
    fn mixed_duplicates_and_unique() {
        let mut entries = vec![
            make_entry("a.cbz", "dup.cbz", "/libraries/BD/dup.cbz"),
            make_entry("b.cbz", "unique.cbz", "/libraries/BD/unique.cbz"),
            make_entry("c.cbz", "dup.cbz", "/libraries/BD/dup.cbz"),
        ];
        deduplicate_filenames(&mut entries);
        assert_eq!(entries[0].new_filename, "dup.cbz");
        assert_eq!(entries[1].new_filename, "unique.cbz");
        assert_eq!(entries[2].new_filename, "dup (2).cbz");
    }

    // ── full flow simulation ────────────────────────────────────────────

    #[test]
    fn full_flow_42_books_with_mixed_volumes() {
        let template = "{series_name} - T{volume_padded} - {title}";
        let series_name = "dragon ball";
        let max_volume = 42i64;

        // Simulate books with various edge cases
        let test_cases: Vec<(Option<i32>, &str)> = vec![
            (Some(1), "Le secret du pouvoir surhumain"),
            (Some(2), "Kamehameha"),
            (Some(10), "Le miracle"),
            (Some(42), "La victoire"),
            (None, "Hors série spécial"),
            (Some(3), "L'épreuve"),
        ];

        for (vol, title) in &test_cases {
            let book = make_book(title, *vol, vec!["Akira Toriyama"], "/libraries/BD/old.cbz");
            let result = apply_template(template, series_name, &book, max_volume);
            let result = sanitize_filename(&result);
            // Should never be empty
            assert!(!result.is_empty(), "empty result for vol={:?} title={}", vol, title);
            // Should not contain null bytes
            assert!(!result.contains('\x00'), "null byte in result: {}", result);
        }

        // Check specific outputs
        let book1 = make_book("Le secret", Some(1), vec![], "/libraries/BD/old.cbz");
        assert_eq!(
            apply_template(template, series_name, &book1, max_volume),
            "dragon ball - T01 - Le secret"
        );

        let book42 = make_book("La victoire", Some(42), vec![], "/libraries/BD/old.cbz");
        assert_eq!(
            apply_template(template, series_name, &book42, max_volume),
            "dragon ball - T42 - La victoire"
        );

        let book_hs = make_book("Hors série", None, vec![], "/libraries/BD/old.cbz");
        assert_eq!(
            apply_template(template, series_name, &book_hs, max_volume),
            "dragon ball - Hors série"
        );
    }

    #[test]
    fn no_double_extension_when_series_name_contains_ext() {
        // Simulate the full rename entry generation: template produces a stem
        // that already ends with .cbr, and original file is .cbr
        let book = make_book("La Séparation", Some(1), vec![], "/libraries/BD/Avengers.cbr/old.cbr");
        let template = "{series_name} - T{volume_padded} - {title}";
        let series_name = "Avengers - La Séparation.cbr";
        let new_stem = apply_template(template, series_name, &book, 1);
        let new_stem = sanitize_filename(&new_stem);
        let extension = ".cbr";
        // The stem already ends with .cbr, so we should NOT add another .cbr
        let new_filename = if new_stem.to_lowercase().ends_with(&extension.to_lowercase()) {
            new_stem.clone()
        } else {
            format!("{}{}", new_stem, extension)
        };
        assert!(
            !new_filename.ends_with(".cbr.cbr"),
            "double extension detected: {}",
            new_filename
        );
        assert!(new_filename.ends_with(".cbr"));
    }

    #[test]
    fn template_with_special_chars_in_title() {
        let book = make_book(
            "L'épreuve: le retour! (2ème édition)",
            Some(5),
            vec!["Auteur"],
            "/libraries/BD/old.cbz",
        );
        let result = apply_template("{series_name} - T{volume_padded} - {title}", "Série à accents", &book, 10);
        let sanitized = sanitize_filename(&result);
        assert_eq!(sanitized, "Série à accents - T05 - L'épreuve_ le retour! (2ème édition)");
    }

    // ─── Volume extraction fallback from filename ───────────────────────

    #[test]
    fn volume_extracted_from_filename_when_db_null_tome() {
        // "Tome 05.cbz" has no volume in DB but filename contains "Tome 05"
        let book = make_book("Tome 05", None, vec![], "/libraries/BD/Frieren/Tome 05.cbz");
        let result = apply_template("{series_name} - T{volume_padded}", "Frieren", &book, 14);
        assert_eq!(result, "Frieren - T05");
    }

    #[test]
    fn volume_extracted_from_filename_when_db_null_t_prefix() {
        // "Frieren – T10.cbz" — should work with T prefix too
        let book = make_book("Frieren – T10", None, vec![], "/libraries/BD/Frieren/Frieren – T10.cbz");
        let result = apply_template("{series_name} - T{volume_padded}", "Frieren", &book, 14);
        assert_eq!(result, "Frieren - T10");
    }

    #[test]
    fn volume_db_takes_precedence_over_filename() {
        // DB has volume=3 but filename says "Tome 05" → use DB
        let book = make_book("Tome 05", Some(3), vec![], "/libraries/BD/Frieren/Tome 05.cbz");
        let result = apply_template("{series_name} - T{volume_padded}", "Frieren", &book, 14);
        assert_eq!(result, "Frieren - T03");
    }

    #[test]
    fn volume_none_and_no_volume_in_filename() {
        // No volume anywhere → template degrades to series_name only
        let book = make_book("Special Edition", None, vec![], "/libraries/BD/Frieren/Special Edition.cbz");
        let result = apply_template("{series_name} - T{volume_padded}", "Frieren", &book, 10);
        // volume_padded is None → entire segment with missing var is removed
        assert_eq!(result, "Frieren");
    }

    // ─── Post-rename DB update: title + volume extraction ───────────────

    #[test]
    fn post_rename_extracts_title_and_volume() {
        // After rename, the new filename stem becomes the book title
        let new_path = std::path::Path::new("/libraries/BD/Frieren/Frieren - T05.cbz");
        let new_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap();
        let new_volume = parsers::extract_volume(new_stem);
        assert_eq!(new_stem, "Frieren - T05");
        assert_eq!(new_volume, Some(5));
    }

    #[test]
    fn post_rename_extracts_volume_from_tome_pattern() {
        let new_path = std::path::Path::new("/libraries/BD/Series/Series - Tome 12.cbz");
        let new_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap();
        let new_volume = parsers::extract_volume(new_stem);
        assert_eq!(new_volume, Some(12));
    }

    #[test]
    fn post_rename_no_volume_in_special_edition() {
        let new_path = std::path::Path::new("/libraries/BD/Series/Series - Special.cbz");
        let new_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap();
        let new_volume = parsers::extract_volume(new_stem);
        assert_eq!(new_volume, None);
    }

    #[test]
    fn post_rename_padded_volume() {
        let new_path = std::path::Path::new("/libraries/BD/One Piece/One Piece - T001.cbz");
        let new_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap();
        let new_volume = parsers::extract_volume(new_stem);
        assert_eq!(new_volume, Some(1));
    }
}
