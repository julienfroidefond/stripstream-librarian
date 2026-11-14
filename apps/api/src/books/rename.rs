use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use stripstream_core::fingerprint::compute_fingerprint;
use stripstream_core::settings::load_setting;

use crate::error::ApiError;
use crate::state::AppState;

// ─── Request / Response types ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RenameRequest {
    /// Template pattern. If null, uses the saved `rename_format` setting.
    pub format: Option<String>,
    /// Template pattern for hors-série books. If null, HS books use the main format.
    pub format_hs: Option<String>,
    /// Template pattern for integral books. If null, uses the saved `rename_format_int` setting.
    pub format_int: Option<String>,
    /// Template pattern for oneshot books. If null, uses the saved `rename_format_oneshot` setting.
    pub format_oneshot: Option<String>,
    /// "preview" for dry-run, "execute" to perform renames.
    pub mode: RenameMode,
    /// Per-book metadata overrides applied before rendering and executing.
    #[serde(default)]
    pub overrides: Vec<RenameOverride>,
}

#[derive(Deserialize)]
pub struct RenameOverride {
    pub book_id: Uuid,
    /// Corrected volume number. `None` leaves the stored volume untouched.
    pub volume: Option<i32>,
    /// Corrected volume type (`regular`, `hs`, `integral`, `oneshot`).
    pub volume_type: Option<String>,
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
    /// Effective volume after overrides (used by the UI to prefill the editor).
    pub volume: Option<i32>,
    /// Effective volume type after overrides.
    pub volume_type: String,
}

#[derive(Serialize)]
pub struct RenameError {
    pub book_id: Uuid,
    pub filename: String,
    pub error: String,
}

// ─── Book data for template rendering ───────────────────────────────────────

pub(crate) struct RenameTemplates {
    pub(crate) regular: String,
    pub(crate) hs: String,
    pub(crate) integral: String,
    pub(crate) oneshot: String,
}

pub(crate) struct RenameTemplateBook {
    pub(crate) title: String,
    pub(crate) authors: Vec<String>,
    pub(crate) volume: Option<i32>,
    pub(crate) volume_type: String,
    pub(crate) publish_date: Option<String>,
    pub(crate) isbn: Option<String>,
    pub(crate) abs_path: String,
}

struct BookFileData {
    book_id: Uuid,
    template_book: RenameTemplateBook,
    file_id: Uuid,
}

// ─── Template engine ────────────────────────────────────────────────────────

pub(crate) const DEFAULT_RENAME_TEMPLATE: &str = "{series_name} - T{volume_padded} - {title}";
pub(crate) const DEFAULT_RENAME_TEMPLATE_HS: &str = "{series_name} - HS {volume_padded}";
pub(crate) const DEFAULT_RENAME_TEMPLATE_INT: &str = "{series_name} - INT {volume_padded}";
pub(crate) const DEFAULT_RENAME_TEMPLATE_ONESHOT: &str = "{series_name}";

pub(crate) async fn load_rename_templates(pool: &PgPool) -> anyhow::Result<RenameTemplates> {
    let regular = load_rename_template(pool, "rename_format", DEFAULT_RENAME_TEMPLATE).await?;
    let hs = load_rename_template(pool, "rename_format_hs", DEFAULT_RENAME_TEMPLATE_HS).await?;
    let integral =
        load_rename_template(pool, "rename_format_int", DEFAULT_RENAME_TEMPLATE_INT).await?;
    let oneshot = load_rename_template(
        pool,
        "rename_format_oneshot",
        DEFAULT_RENAME_TEMPLATE_ONESHOT,
    )
    .await?;

    Ok(RenameTemplates {
        regular,
        hs,
        integral,
        oneshot,
    })
}

pub(crate) async fn load_rename_max_volume(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    fallback_max_volume: i32,
) -> Result<i64, sqlx::Error> {
    let max_volume: i32 = sqlx::query_scalar(
        "SELECT GREATEST(
            COALESCE(MAX(b.volume), 0),
            COALESCE(MAX(s.total_volumes), 0),
            $3::int4
         )
         FROM series s
         LEFT JOIN books b ON b.series_id = s.id AND b.volume IS NOT NULL
         WHERE s.library_id = $1
           AND norm_text(s.name) = norm_text($2)",
    )
    .bind(library_id)
    .bind(series_name)
    .bind(fallback_max_volume)
    .fetch_one(pool)
    .await?;

    Ok(max_volume as i64)
}

async fn load_rename_template(
    pool: &PgPool,
    key: &str,
    default_template: &str,
) -> anyhow::Result<String> {
    let value = load_setting::<serde_json::Value>(pool, key).await?;

    Ok(value
        .and_then(|val| val.as_str().map(ToOwned::to_owned))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default_template.to_string()))
}

fn apply_template(
    template: &str,
    series_name: &str,
    book: &RenameTemplateBook,
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

    vars.insert("volume", effective_volume.map(|v| v.to_string()));
    vars.insert(
        "volume_padded",
        effective_volume.map(|v| {
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
        }),
    );
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
    let cleanup = Regex::new(
        r"\s*-\s*[^\x00\s]*\x00[^\x00\s]*|\s*[^\x00\s]*\x00[^\x00\s]*\s*-\s*|\s*\x00\s*|\x00",
    )
    .expect("valid regex");
    cleanup.replace_all(&result, "").trim().to_string()
}

pub(crate) fn render_rename_filename(
    templates: &RenameTemplates,
    series_name: &str,
    book: &RenameTemplateBook,
    max_volume: i64,
    extension: &str,
) -> Option<String> {
    let effective_template = match book.volume_type.as_str() {
        "hs" => &templates.hs,
        "integral" => &templates.integral,
        "oneshot" => &templates.oneshot,
        _ => &templates.regular,
    };
    let new_stem = apply_template(effective_template, series_name, book, max_volume);
    let new_stem = sanitize_filename(&new_stem);
    if new_stem.is_empty() {
        return None;
    }

    if extension.is_empty()
        || new_stem
            .to_lowercase()
            .ends_with(&format!(".{}", extension.to_lowercase()))
    {
        Some(new_stem)
    } else {
        Some(format!("{}.{}", new_stem, extension))
    }
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
    if trimmed.chars().count() > 200 {
        trimmed.chars().take(200).collect()
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
    // 1. Get the templates
    let mut templates = load_rename_templates(&state.pool).await?;
    if let Some(ref f) = req.format {
        if !f.is_empty() {
            templates.regular = f.clone();
        }
    }
    if let Some(ref f) = req.format_hs {
        if !f.is_empty() {
            templates.hs = f.clone();
        }
    }
    if let Some(ref f) = req.format_int {
        if !f.is_empty() {
            templates.integral = f.clone();
        }
    }
    if let Some(ref f) = req.format_oneshot {
        if !f.is_empty() {
            templates.oneshot = f.clone();
        }
    }

    // Validate per-book overrides before doing any work.
    for ov in &req.overrides {
        if let Some(ref vt) = ov.volume_type {
            if !matches!(vt.as_str(), "regular" | "hs" | "integral" | "oneshot") {
                return Err(ApiError::bad_request(format!(
                    "invalid volume_type override: {}",
                    vt
                )));
            }
        }
    }
    let overrides: HashMap<Uuid, &RenameOverride> =
        req.overrides.iter().map(|o| (o.book_id, o)).collect();

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
        SELECT b.id AS book_id, b.title, b.authors, b.volume, b.volume_type, b.publish_date, b.isbn,
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

    // Parse book data, applying per-book overrides.
    let books: Vec<BookFileData> = rows
        .iter()
        .map(|row| {
            let authors_raw: Vec<String> = row.get("authors");
            let book_id: Uuid = row.get("book_id");
            let ov = overrides.get(&book_id);
            let volume = ov.and_then(|o| o.volume).or_else(|| row.get("volume"));
            let volume_type = ov
                .and_then(|o| o.volume_type.clone())
                .unwrap_or_else(|| row.get("volume_type"));
            BookFileData {
                book_id,
                template_book: RenameTemplateBook {
                    title: row.get("title"),
                    authors: authors_raw,
                    volume,
                    volume_type,
                    publish_date: row.get("publish_date"),
                    isbn: row.get("isbn"),
                    abs_path: row.get("abs_path"),
                },
                file_id: row.get("file_id"),
            }
        })
        .collect();

    // Compute max volume for padding
    let max_volume: i64 = books
        .iter()
        .filter_map(|b| b.template_book.volume.map(|v| v as i64))
        .max()
        .unwrap_or(0);

    // 4. Generate rename entries
    let mut entries: Vec<RenameEntry> = books
        .iter()
        .map(|book| {
            let old_path = PathBuf::from(&book.template_book.abs_path);
            let old_filename = old_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();

            let extension = old_path
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();

            let new_filename = render_rename_filename(
                &templates,
                &series_name,
                &book.template_book,
                max_volume,
                &extension,
            )
            .unwrap_or_else(|| old_filename.clone());

            let new_path = old_path
                .parent()
                .map(|p| p.join(&new_filename).to_string_lossy().to_string())
                .unwrap_or_else(|| new_filename.clone());

            let changed = old_filename != new_filename;

            RenameEntry {
                book_id: book.book_id,
                old_filename,
                new_filename,
                old_path: book.template_book.abs_path.clone(),
                new_path,
                changed,
                volume: book.template_book.volume,
                volume_type: book.template_book.volume_type.clone(),
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

    // Resolve every physical path before touching the disk, so an invalid
    // database path aborts the batch instead of leaving it half-renamed.
    let physical_paths: Vec<(String, String)> = changed_entries
        .iter()
        .map(|entry| {
            Ok((
                state
                    .library_root
                    .resolve(&entry.old_path)?
                    .to_local()
                    .to_string(),
                state
                    .library_root
                    .resolve(&entry.new_path)?
                    .to_local()
                    .to_string(),
            ))
        })
        .collect::<Result<_, ApiError>>()?;

    // Build a lookup from book_id to book data
    let book_map: HashMap<Uuid, &BookFileData> = books.iter().map(|b| (b.book_id, b)).collect();

    let plan: Vec<PlannedRename> = changed_entries
        .iter()
        .zip(physical_paths)
        .filter_map(|(entry, (physical_old, physical_new))| {
            book_map.get(&entry.book_id).map(|book| PlannedRename {
                book_id: entry.book_id,
                file_id: book.file_id,
                db_new: entry.new_path.clone(),
                physical_old,
                physical_new,
                volume: book.template_book.volume,
                volume_type: book.template_book.volume_type.clone(),
            })
        })
        .collect();

    let renamed = execute_rename_plan(&state.pool, plan).await?;

    tracing::info!(
        "[RENAME] Successfully renamed {} files in series '{}'",
        renamed,
        series_name
    );

    Ok(Json(RenameResponse {
        series_id,
        renames: entries,
        errors: vec![],
        executed: true,
    }))
}

// ─── Execution ──────────────────────────────────────────────────────────────

/// One file move of an executed rename batch.
pub(crate) struct PlannedRename {
    pub book_id: Uuid,
    pub file_id: Uuid,
    /// New `book_files.abs_path` (database form).
    pub db_new: String,
    pub physical_old: String,
    pub physical_new: String,
    pub volume: Option<i32>,
    pub volume_type: String,
}

/// Disk moves already performed, undone in reverse order on failure.
#[derive(Default)]
struct DiskJournal {
    moves: Vec<(String, String)>,
}

impl DiskJournal {
    async fn rename(&mut self, from: &str, to: &str) -> std::io::Result<()> {
        let (f, t) = (from.to_string(), to.to_string());
        tokio::task::spawn_blocking(move || std::fs::rename(&f, &t))
            .await
            .map_err(std::io::Error::other)??;
        self.moves.push((from.to_string(), to.to_string()));
        Ok(())
    }

    /// Undo every move in reverse order. Reversing the whole sequence restores
    /// the original layout even for chains and swaps (A→tmp, B→A, tmp→B).
    async fn rollback(self) {
        let moves = self.moves;
        let _ = tokio::task::spawn_blocking(move || {
            for (from, to) in moves.iter().rev() {
                if let Err(e) = std::fs::rename(to, from) {
                    tracing::error!("[RENAME] Rollback failed for {} -> {}: {}", to, from, e);
                }
            }
        })
        .await;
    }
}

/// Unique temporary sibling of `physical` (same directory, so renames stay atomic).
fn temp_sibling(physical: &str, token: &str) -> String {
    let path = std::path::Path::new(physical);
    let name = format!(".stl-rename-{token}.tmp");
    path.parent()
        .map(|p| p.join(&name).to_string_lossy().to_string())
        .unwrap_or(name)
}

/// Execute a rename batch safely:
/// - every target must be free or be the source of another file of the batch;
/// - phase 1 moves every source to a unique temporary sibling, phase 2 moves
///   the temporaries to their targets, so chains and swaps never overwrite;
/// - the database is updated in one transaction, first to unique placeholder
///   paths then to the targets, so the `UNIQUE (abs_path)` constraint holds;
/// - any disk or SQL error undoes every disk move already performed.
///
/// Returns the number of renamed files.
pub(crate) async fn execute_rename_plan(
    pool: &PgPool,
    plan: Vec<PlannedRename>,
) -> Result<usize, ApiError> {
    if plan.is_empty() {
        return Ok(0);
    }

    let sources: std::collections::HashSet<&str> =
        plan.iter().map(|p| p.physical_old.as_str()).collect();
    let mut targets = std::collections::HashSet::new();
    for p in &plan {
        if !targets.insert(p.physical_new.as_str()) {
            return Err(ApiError::bad_request(format!(
                "two books would be renamed to the same file: {}",
                p.physical_new
            )));
        }
        if !sources.contains(p.physical_new.as_str())
            && std::path::Path::new(&p.physical_new).exists()
        {
            return Err(ApiError::bad_request(format!(
                "target file already exists: {}",
                p.physical_new
            )));
        }
    }

    let batch_token = Uuid::new_v4().simple().to_string();
    let temps: Vec<String> = plan
        .iter()
        .enumerate()
        .map(|(i, p)| temp_sibling(&p.physical_old, &format!("{batch_token}-{i}")))
        .collect();

    let mut journal = DiskJournal::default();

    // Phase 1: sources → temporaries.
    for (p, temp) in plan.iter().zip(&temps) {
        if let Err(e) = journal.rename(&p.physical_old, temp).await {
            tracing::error!("[RENAME] Failed to move {} aside: {}", p.physical_old, e);
            journal.rollback().await;
            return Err(ApiError::internal(format!(
                "failed to rename {}: {}. All renames rolled back.",
                p.physical_old, e
            )));
        }
    }

    // Phase 2: temporaries → targets (a target appearing meanwhile aborts).
    for (p, temp) in plan.iter().zip(&temps) {
        let result = if std::path::Path::new(&p.physical_new).exists() {
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "target file already exists",
            ))
        } else {
            journal.rename(temp, &p.physical_new).await
        };
        if let Err(e) = result {
            tracing::error!(
                "[RENAME] Failed to rename {} -> {}: {}",
                p.physical_old,
                p.physical_new,
                e
            );
            journal.rollback().await;
            return Err(ApiError::internal(format!(
                "failed to rename {}: {}. All renames rolled back.",
                p.physical_old, e
            )));
        }
    }

    match update_renamed_files(pool, &plan, &batch_token).await {
        Ok(()) => Ok(plan.len()),
        Err(e) => {
            tracing::error!(
                "[RENAME] DB update failed, rolling back disk: {}",
                e.message
            );
            journal.rollback().await;
            Err(e)
        }
    }
}

/// Database side of [`execute_rename_plan`], all-or-nothing.
async fn update_renamed_files(
    pool: &PgPool,
    plan: &[PlannedRename],
    batch_token: &str,
) -> Result<(), ApiError> {
    let mut fingerprints = Vec::with_capacity(plan.len());
    for p in plan {
        let physical = p.physical_new.clone();
        let (fp, mtime) = tokio::task::spawn_blocking(move || match std::fs::metadata(&physical) {
            Ok(meta) => {
                let mtime: DateTime<Utc> = meta
                    .modified()
                    .map(DateTime::<Utc>::from)
                    .unwrap_or_else(|_| Utc::now());
                let fp = compute_fingerprint(std::path::Path::new(&physical), meta.len(), &mtime)
                    .unwrap_or_default();
                (fp, mtime)
            }
            Err(_) => (String::new(), Utc::now()),
        })
        .await
        .map_err(|e| ApiError::internal(format!("fingerprint task failed: {e}")))?;
        fingerprints.push((fp, mtime));
    }

    let file_ids: Vec<Uuid> = plan.iter().map(|p| p.file_id).collect();
    let mut tx = pool.begin().await?;

    // Step 1: unique placeholders, so swapped paths never collide mid-update.
    sqlx::query(
        "UPDATE book_files SET abs_path = abs_path || '.stl-renaming-' || $2 || '-' || id::text \
         WHERE id = ANY($1)",
    )
    .bind(&file_ids)
    .bind(batch_token)
    .execute(&mut *tx)
    .await?;

    // Step 2: final paths.
    for (p, (fp, mtime)) in plan.iter().zip(&fingerprints) {
        sqlx::query(
            "UPDATE book_files SET abs_path = $1, fingerprint = $2, mtime = $3 WHERE id = $4",
        )
        .bind(&p.db_new)
        .bind(fp)
        .bind(mtime)
        .bind(p.file_id)
        .execute(&mut *tx)
        .await?;

        // Preserve book metadata: only volume / volume_type are synced
        // (from explicit overrides or the stored values), never the title.
        sqlx::query(
            "UPDATE books SET volume = $1, volume_type = $2, updated_at = NOW() WHERE id = $3",
        )
        .bind(p.volume)
        .bind(&p.volume_type)
        .bind(p.book_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/rename.rs"]
mod tests;
