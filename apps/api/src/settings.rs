use axum::{
    extract::{Path as AxumPath, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{error::ApiError, state::{AppState, load_dynamic_settings}};

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
        .route("/settings/:key", get(get_setting).post(update_setting))
        .route("/settings/cache/clear", post(clear_cache))
        .route("/settings/cache/stats", get(get_cache_stats))
        .route("/settings/thumbnail/stats", get(get_thumbnail_stats))
        .route(
            "/settings/status-mappings",
            get(list_status_mappings).post(upsert_status_mapping),
        )
        .route(
            "/settings/status-mappings/:id",
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
        let value: Value = row.get("value");
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
    let row = sqlx::query(r#"SELECT value FROM app_settings WHERE key = $1"#)
        .bind(&key)
        .fetch_optional(&state.pool)
        .await?;

    match row {
        Some(row) => {
            let value: Value = row.get("value");
            Ok(Json(value))
        }
        None => Ok(Json(Value::Null)),
    }
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
    .bind(&body.value)
    .fetch_one(&state.pool)
    .await?;

    let value: Value = row.get("value");

    // Rechargement des settings dynamiques si la clé affecte le comportement runtime
    if key == "limits" || key == "image_processing" || key == "cache" {
        let new_settings = load_dynamic_settings(&state.pool).await;
        *state.settings.write().await = new_settings;
    }

    Ok(Json(value))
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
pub async fn clear_cache(State(state): State<AppState>) -> Result<Json<ClearCacheResponse>, ApiError> {
    let cache_dir = state.settings.read().await.cache_directory.clone();

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
                message: format!("Cache directory '{}' does not exist, nothing to clear", cache_dir),
            }
        }
    })
    .await
    .map_err(|e| ApiError::internal(format!("cache clear failed: {}", e)))?;

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

    let cache_dir_clone = cache_dir.clone();
    let stats = tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&cache_dir_clone);
        if !path.exists() {
            return CacheStats {
                total_size_mb: 0.0,
                file_count: 0,
                directory: cache_dir_clone,
            };
        }

        let mut total_size: u64 = 0;
        let mut file_count: u64 = 0;

        fn visit_dirs(
            dir: &std::path::Path,
            total_size: &mut u64,
            file_count: &mut u64,
        ) -> std::io::Result<()> {
            if dir.is_dir() {
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        visit_dirs(&path, total_size, file_count)?;
                    } else {
                        *total_size += entry.metadata()?.len();
                        *file_count += 1;
                    }
                }
            }
            Ok(())
        }

        let _ = visit_dirs(path, &mut total_size, &mut file_count);

        CacheStats {
            total_size_mb: total_size as f64 / 1024.0 / 1024.0,
            file_count,
            directory: cache_dir_clone,
        }
    })
    .await
    .map_err(|e| ApiError::internal(format!("cache stats failed: {}", e)))?;

    Ok(Json(stats))
}

fn compute_dir_stats(path: &std::path::Path) -> (u64, u64) {
    let mut total_size: u64 = 0;
    let mut file_count: u64 = 0;

    fn visit_dirs(
        dir: &std::path::Path,
        total_size: &mut u64,
        file_count: &mut u64,
    ) -> std::io::Result<()> {
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, total_size, file_count)?;
                } else {
                    *total_size += entry.metadata()?.len();
                    *file_count += 1;
                }
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
pub async fn get_thumbnail_stats(State(_state): State<AppState>) -> Result<Json<ThumbnailStats>, ApiError> {
    let settings = sqlx::query(r#"SELECT value FROM app_settings WHERE key = 'thumbnail'"#)
        .fetch_optional(&_state.pool)
        .await?;

    let directory = match settings {
        Some(row) => {
            let value: serde_json::Value = row.get("value");
            value.get("directory")
                .and_then(|v| v.as_str())
                .unwrap_or("/data/thumbnails")
                .to_string()
        }
        None => "/data/thumbnails".to_string(),
    };

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
