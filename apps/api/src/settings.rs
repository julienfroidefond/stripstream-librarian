use axum::{
    extract::{Path as AxumPath, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use std::path::Path as FsPath;
use std::time::Duration;
use stripstream_core::paths::normalize_lexically;
use stripstream_core::settings::load_setting;
use stripstream_thumbnails::{ThumbnailSettings, DEFAULT_DIRECTORY as DEFAULT_THUMBNAIL_DIRECTORY};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    downloads::settings::DownloadSettings,
    error::ApiError,
    state::{load_dynamic_settings, AppState, DiskCacheStatsSnapshot},
};

pub mod limits;

use limits::{Limits, LIMITS_KEY};

const DISK_CACHE_STATS_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSettingRequest {
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClearCacheResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CacheStats {
    pub total_size_mb: f64,
    pub file_count: u64,
    pub directory: String,
    pub memory_size_mb: f64,
    pub memory_page_count: usize,
    pub memory_max_size_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ThumbnailStats {
    pub total_size_mb: f64,
    pub file_count: u64,
    pub directory: String,
}

pub fn settings_routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(get_settings))
        .route("/settings/{key}", get(get_setting).post(update_setting))
        .route(
            "/settings/ai_tagging/test",
            post(crate::ai_tagging::test_connection),
        )
        .route("/settings/cache/clear", post(clear_cache))
        .route("/settings/cache/stats", get(get_cache_stats))
        .route("/settings/thumbnail/stats", get(get_thumbnail_stats))
        .route(
            "/settings/status-mappings",
            get(list_status_mappings).post(upsert_status_mapping),
        )
        .route(
            "/settings/status-mappings/{id}",
            delete(delete_status_mapping),
        )
}

/// List all settings
#[utoipa::path(
    get,
    path = "/settings",
    tag = "settings",
    responses(
        (status = 200, description = "All settings as key/value object"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_settings(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query(r#"SELECT key, value FROM app_settings"#)
        .fetch_all(&state.pool)
        .await?;

    let mut settings = serde_json::Map::new();
    for row in rows {
        let key: String = row.get("key");
        let mut value: Value = row.get("value");
        redact_secrets(&key, &mut value);
        settings.insert(key, value);
    }

    Ok(Json(Value::Object(settings)))
}

/// Get a single setting by key
#[utoipa::path(
    get,
    path = "/settings/{key}",
    tag = "settings",
    params(("key" = String, Path, description = "Setting key")),
    responses(
        (status = 200, description = "Setting value"),
        (status = 404, description = "Setting not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_setting(
    State(state): State<AppState>,
    axum::extract::Path(key): axum::extract::Path<String>,
) -> Result<Json<Value>, ApiError> {
    match load_setting::<Value>(&state.pool, &key).await? {
        Some(mut value) => {
            redact_secrets(&key, &mut value);
            Ok(Json(value))
        }
        None => Err(ApiError::not_found("setting not found")),
    }
}

/// Setting keys holding a credential, and the fields inside each value that
/// must never be echoed back by the settings endpoints.
const SECRET_FIELDS: &[(&str, &[&str])] = &[("alldebrid", &["api_key"])];

/// Placeholder returned in place of a stored secret.
const REDACTED_SECRET: &str = "********";

fn secret_fields(key: &str) -> Option<&'static [&'static str]> {
    SECRET_FIELDS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, fields)| *fields)
}

/// Replace stored secret fields with [`REDACTED_SECRET`] so responses never
/// leak credentials. Only non-empty values are masked, keeping "not yet
/// configured" visible to the backoffice.
fn redact_secrets(key: &str, value: &mut Value) {
    let Some(fields) = secret_fields(key) else {
        return;
    };
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    for field in fields {
        if let Some(slot) = obj.get_mut(*field) {
            if slot.as_str().map(|s| !s.is_empty()).unwrap_or(false) {
                *slot = Value::String(REDACTED_SECRET.to_string());
            }
        }
    }
}

/// Restore secrets the client left untouched: an update carrying an empty or
/// redacted secret keeps the value already stored, so re-saving the settings
/// form never wipes the API key.
async fn preserve_secrets(
    pool: &sqlx::PgPool,
    key: &str,
    incoming: &mut Value,
) -> Result<(), ApiError> {
    let Some(fields) = secret_fields(key) else {
        return Ok(());
    };
    let Some(stored) = load_setting::<Value>(pool, key).await? else {
        return Ok(());
    };
    let Some(obj) = incoming.as_object_mut() else {
        return Ok(());
    };
    for field in fields {
        // Untouched = field absent, or left empty / at the redacted placeholder.
        let untouched = match obj.get(*field) {
            None => true,
            Some(v) => v
                .as_str()
                .map(|s| s.is_empty() || s == REDACTED_SECRET)
                .unwrap_or(false),
        };
        if untouched {
            if let Some(stored_value) = stored.get(*field) {
                obj.insert((*field).to_string(), stored_value.clone());
            }
        }
    }
    Ok(())
}

/// Validate a setting value before it is persisted.
///
/// Only the runtime keys whose values directly control resource allocation,
/// filesystem access or request admission are validated; unknown keys are left
/// untouched so the Backoffice can store arbitrary configuration. Only `cache`
/// needs the thumbnail settings, so no other key depends on reading them.
async fn validate_setting(state: &AppState, key: &str, value: &Value) -> Result<(), ApiError> {
    match key {
        LIMITS_KEY => validate_limits(value),
        "cache" => {
            // Fail closed: without the configured thumbnail directory, a cache
            // directory covering it could not be refused.
            let thumbnail_directory = ThumbnailSettings::load(&state.pool).await?.directory;
            validate_cache(value, &state.library_root, &thumbnail_directory)
        }
        _ => Ok(()),
    }
}

/// `limits` is validated by its typed schema: every bad field is reported.
fn validate_limits(value: &Value) -> Result<(), ApiError> {
    Limits::parse(value).map(drop).map_err(|errors| {
        let messages: Vec<String> = errors.iter().map(ToString::to_string).collect();
        ApiError::bad_request(messages.join("; "))
    })
}

fn validate_cache(
    value: &Value,
    library_root: &stripstream_core::paths::LibraryRoot,
    thumbnail_directory: &FsPath,
) -> Result<(), ApiError> {
    if let Some(max_mb) = value.get("memory_max_size_mb") {
        let max_mb = max_mb
            .as_u64()
            .ok_or_else(|| ApiError::bad_request("cache.memory_max_size_mb must be an integer"))?;
        if max_mb == 0 {
            return Err(ApiError::bad_request(
                "cache.memory_max_size_mb must be greater than 0",
            ));
        }
    }
    if let Some(dir) = value.get("directory") {
        let dir = dir
            .as_str()
            .ok_or_else(|| ApiError::bad_request("cache.directory must be a string"))?;
        if !is_safe_cache_directory(dir, library_root, thumbnail_directory) {
            return Err(ApiError::bad_request(
                "cache.directory must be an absolute path outside protected directories",
            ));
        }
    }
    Ok(())
}

/// Reject cache directories that would make `clear_cache` destructive if pointed
/// at a system or data root (`remove_dir_all` runs on this path). The thumbnail
/// store (configured and default directories) is protected too.
fn is_safe_cache_directory(
    path: &str,
    library_root: &stripstream_core::paths::LibraryRoot,
    thumbnail_directory: &FsPath,
) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }

    let path = FsPath::new(trimmed);
    if !path.is_absolute() {
        return false;
    }

    let normalized = normalize_lexically(path);
    if normalized.parent().is_none() {
        return false;
    }

    let protected = [
        library_root.local_root(),
        thumbnail_directory,
        FsPath::new(DEFAULT_THUMBNAIL_DIRECTORY),
    ];

    const SYSTEM_ROOTS: &[&str] = &[
        "/bin", "/boot", "/dev", "/etc", "/lib", "/lib64", "/proc", "/sbin", "/sys", "/usr",
    ];

    !SYSTEM_ROOTS
        .iter()
        .map(FsPath::new)
        .chain(protected)
        .any(|root| normalized.starts_with(root))
}

/// Create or update a setting
#[utoipa::path(
    post,
    path = "/settings/{key}",
    tag = "settings",
    params(("key" = String, Path, description = "Setting key")),
    request_body = UpdateSettingRequest,
    responses(
        (status = 200, description = "Updated setting value"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_setting(
    State(state): State<AppState>,
    axum::extract::Path(key): axum::extract::Path<String>,
    Json(body): Json<UpdateSettingRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut value = body.value;
    // Empty/redacted secrets keep the stored value (the form never receives it).
    preserve_secrets(&state.pool, &key, &mut value).await?;
    validate_setting(&state, &key, &value).await?;

    let row = sqlx::query(
        r#"
        INSERT INTO app_settings (key, value, updated_at)
        VALUES ($1, $2, CURRENT_TIMESTAMP)
        ON CONFLICT (key)
        DO UPDATE SET value = $2, updated_at = CURRENT_TIMESTAMP
        RETURNING value
        "#,
    )
    .bind(&key)
    .bind(&value)
    .fetch_one(&state.pool)
    .await?;

    let mut value: Value = row.get("value");
    redact_secrets(&key, &mut value);

    apply(&state, &key).await;

    Ok(Json(value))
}

/// The single hot-reload point: pushes a freshly saved setting into the
/// running process, so a Backoffice change never needs an API restart.
/// Keys without a runtime effect are ignored.
pub(crate) async fn apply(state: &AppState, key: &str) {
    match key {
        LIMITS_KEY => apply_limits(state, &Limits::load(&state.pool).await).await,
        "image_processing" | "cache" => {
            let new_settings = load_dynamic_settings(&state.pool).await;
            state
                .page_cache
                .lock()
                .await
                .set_max_size_mb(new_settings.page_cache_max_size_mb);
            *state.settings.write().await = new_settings;
            if key == "cache" {
                *state.disk_cache_stats.lock().await = None;
            }
        }
        _ => {}
    }
}

/// Applies every runtime consumer of `limits`: request admission and render
/// timeout, the render ceiling and the download queues. Running renders and
/// transfers are never interrupted; a lower ceiling only stops admitting new
/// ones until enough of them finish.
pub(crate) async fn apply_limits(state: &AppState, limits: &Limits) {
    state.settings.write().await.apply_limits(limits);
    state.page_render_limit.set_limit(limits.concurrent_renders);

    let downloads = DownloadSettings::from(limits);
    state
        .telegram_downloads
        .set_limit(downloads.telegram_concurrency);
    state
        .ddl_provider_downloads
        .set_limit(downloads.ddl_concurrency);
    *state.download_settings.write().await = downloads;
}

/// Clear the image page cache
#[utoipa::path(
    post,
    path = "/settings/cache/clear",
    tag = "settings",
    responses(
        (status = 200, body = ClearCacheResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn clear_cache(
    State(state): State<AppState>,
) -> Result<Json<ClearCacheResponse>, ApiError> {
    state.page_cache.lock().await.clear();
    let cache_dir = state.settings.read().await.cache_directory.clone();

    // The cache directory was checked against the configured thumbnail
    // directory when it was saved; an unreadable thumbnail row must not make
    // clearing the cache fail (the default directory stays protected).
    let thumbnail_directory = ThumbnailSettings::load_or_default(&state.pool)
        .await
        .directory;
    if !is_safe_cache_directory(&cache_dir, &state.library_root, &thumbnail_directory) {
        return Err(ApiError::bad_request(format!(
            "refusing to clear unsafe cache directory '{cache_dir}'"
        )));
    }

    let result = tokio::task::spawn_blocking(move || {
        if std::path::Path::new(&cache_dir).exists() {
            match std::fs::remove_dir_all(&cache_dir) {
                Ok(_) => ClearCacheResponse {
                    success: true,
                    message: format!("Cache directory '{}' cleared successfully", cache_dir),
                },
                Err(e) => ClearCacheResponse {
                    success: false,
                    message: format!("Failed to clear cache: {}", e),
                },
            }
        } else {
            ClearCacheResponse {
                success: true,
                message: format!(
                    "Cache directory '{}' does not exist, nothing to clear",
                    cache_dir
                ),
            }
        }
    })
    .await
    .map_err(|e| ApiError::internal(format!("cache clear failed: {}", e)))?;

    *state.disk_cache_stats.lock().await = None;

    Ok(Json(result))
}

/// Get image page cache statistics
#[utoipa::path(
    get,
    path = "/settings/cache/stats",
    tag = "settings",
    responses(
        (status = 200, body = CacheStats),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_cache_stats(State(state): State<AppState>) -> Result<Json<CacheStats>, ApiError> {
    let cache_dir = state.settings.read().await.cache_directory.clone();
    let (memory_size_mb, memory_page_count, memory_max_size_mb) = {
        let cache = state.page_cache.lock().await;
        (
            cache.current_size_bytes() as f64 / 1024.0 / 1024.0,
            cache.len(),
            cache.max_size_bytes() as f64 / 1024.0 / 1024.0,
        )
    };

    let cached_stats = state
        .disk_cache_stats
        .lock()
        .await
        .as_ref()
        .filter(|stats| {
            stats.directory == cache_dir && stats.collected_at.elapsed() < DISK_CACHE_STATS_TTL
        })
        .cloned();

    let disk_stats = if let Some(stats) = cached_stats {
        stats
    } else {
        let cache_dir_clone = cache_dir.clone();
        let stats = tokio::task::spawn_blocking(move || {
            let path = std::path::Path::new(&cache_dir_clone);
            if !path.exists() {
                return DiskCacheStatsSnapshot {
                    total_size_bytes: 0,
                    file_count: 0,
                    directory: cache_dir_clone,
                    collected_at: std::time::Instant::now(),
                };
            }

            let (total_size, file_count) = compute_dir_stats(path);

            DiskCacheStatsSnapshot {
                total_size_bytes: total_size,
                file_count,
                directory: cache_dir_clone,
                collected_at: std::time::Instant::now(),
            }
        })
        .await
        .map_err(|e| ApiError::internal(format!("cache stats failed: {}", e)))?;
        *state.disk_cache_stats.lock().await = Some(stats.clone());
        stats
    };

    Ok(Json(CacheStats {
        total_size_mb: disk_stats.total_size_bytes as f64 / 1024.0 / 1024.0,
        file_count: disk_stats.file_count,
        directory: disk_stats.directory,
        memory_size_mb,
        memory_page_count,
        memory_max_size_mb,
    }))
}

/// Totals the size and file count under `path`, ignoring symlinks.
///
/// Symlinked directories are skipped rather than followed: the cache and
/// thumbnail directories are user-configurable, and a symlink loop (or a link
/// to `/`) would otherwise make this walk recurse without bound.
pub(crate) fn compute_dir_stats(path: &std::path::Path) -> (u64, u64) {
    let mut total_size: u64 = 0;
    let mut file_count: u64 = 0;

    fn visit_dirs(
        dir: &std::path::Path,
        total_size: &mut u64,
        file_count: &mut u64,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                visit_dirs(&entry.path(), total_size, file_count)?;
            } else if file_type.is_file() {
                *total_size += entry.metadata()?.len();
                *file_count += 1;
            }
        }
        Ok(())
    }

    let _ = visit_dirs(path, &mut total_size, &mut file_count);
    (total_size, file_count)
}

/// Get thumbnail storage statistics
#[utoipa::path(
    get,
    path = "/settings/thumbnail/stats",
    tag = "settings",
    responses(
        (status = 200, body = ThumbnailStats),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_thumbnail_stats(
    State(state): State<AppState>,
) -> Result<Json<ThumbnailStats>, ApiError> {
    let directory = ThumbnailSettings::load_or_default(&state.pool)
        .await
        .directory
        .to_string_lossy()
        .into_owned();

    let directory_clone = directory.clone();
    let stats = tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&directory_clone);
        if !path.exists() {
            return ThumbnailStats {
                total_size_mb: 0.0,
                file_count: 0,
                directory: directory_clone,
            };
        }

        let (total_size, file_count) = compute_dir_stats(path);

        ThumbnailStats {
            total_size_mb: total_size as f64 / 1024.0 / 1024.0,
            file_count,
            directory: directory_clone,
        }
    })
    .await
    .map_err(|e| ApiError::internal(format!("thumbnail stats failed: {}", e)))?;

    Ok(Json(stats))
}

// ---------------------------------------------------------------------------
// Status Mappings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StatusMappingDto {
    pub id: String,
    pub provider_status: String,
    pub mapped_status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpsertStatusMappingRequest {
    pub provider_status: String,
    pub mapped_status: String,
}

/// List all status mappings
#[utoipa::path(
    get,
    path = "/settings/status-mappings",
    tag = "settings",
    responses(
        (status = 200, body = Vec<StatusMappingDto>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_status_mappings(
    State(state): State<AppState>,
) -> Result<Json<Vec<StatusMappingDto>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, provider_status, mapped_status FROM status_mappings ORDER BY mapped_status NULLS LAST, provider_status",
    )
    .fetch_all(&state.pool)
    .await?;

    let mappings = rows
        .iter()
        .map(|row| StatusMappingDto {
            id: row.get::<Uuid, _>("id").to_string(),
            provider_status: row.get("provider_status"),
            mapped_status: row.get::<Option<String>, _>("mapped_status"),
        })
        .collect();

    Ok(Json(mappings))
}

/// Create or update a status mapping
#[utoipa::path(
    post,
    path = "/settings/status-mappings",
    tag = "settings",
    request_body = UpsertStatusMappingRequest,
    responses(
        (status = 200, body = StatusMappingDto),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn upsert_status_mapping(
    State(state): State<AppState>,
    Json(body): Json<UpsertStatusMappingRequest>,
) -> Result<Json<StatusMappingDto>, ApiError> {
    let provider_status = body.provider_status.to_lowercase();

    let row = sqlx::query(
        r#"
        INSERT INTO status_mappings (provider_status, mapped_status)
        VALUES ($1, $2)
        ON CONFLICT (provider_status)
        DO UPDATE SET mapped_status = $2, updated_at = NOW()
        RETURNING id, provider_status, mapped_status
        "#,
    )
    .bind(&provider_status)
    .bind(&body.mapped_status)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(StatusMappingDto {
        id: row.get::<Uuid, _>("id").to_string(),
        provider_status: row.get("provider_status"),
        mapped_status: row.get::<Option<String>, _>("mapped_status"),
    }))
}

/// Unmap a status mapping (sets mapped_status to NULL, keeps the provider status known)
#[utoipa::path(
    delete,
    path = "/settings/status-mappings/{id}",
    tag = "settings",
    params(("id" = String, Path, description = "Mapping UUID")),
    responses(
        (status = 200, body = StatusMappingDto),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_status_mapping(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<StatusMappingDto>, ApiError> {
    let row = sqlx::query(
        "UPDATE status_mappings SET mapped_status = NULL, updated_at = NOW() WHERE id = $1 RETURNING id, provider_status, mapped_status",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    match row {
        Some(row) => Ok(Json(StatusMappingDto {
            id: row.get::<Uuid, _>("id").to_string(),
            provider_status: row.get("provider_status"),
            mapped_status: row.get::<Option<String>, _>("mapped_status"),
        })),
        None => Err(ApiError::not_found("status mapping not found")),
    }
}

#[cfg(test)]
#[path = "tests/settings.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/core_settings.rs"]
mod core_settings_tests;
