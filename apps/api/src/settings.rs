use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;

use crate::{error::ApiError, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageProcessingSettings {
    pub format: String,
    pub quality: u8,
    pub filter: String,
    pub max_width: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSettings {
    pub enabled: bool,
    pub directory: String,
    pub max_size_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsSettings {
    pub concurrent_renders: u8,
    pub timeout_seconds: u8,
    pub rate_limit_per_second: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub image_processing: ImageProcessingSettings,
    pub cache: CacheSettings,
    pub limits: LimitsSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingRequest {
    pub value: Value,
}

pub fn settings_routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(get_settings))
        .route("/settings/:key", get(get_setting).post(update_setting))
        .route("/settings/cache/clear", post(clear_cache))
        .route("/settings/cache/stats", get(get_cache_stats))
}

async fn get_settings(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
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

async fn get_setting(
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
        None => Err(ApiError::not_found(format!("setting '{}' not found", key))),
    }
}

async fn update_setting(
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
    Ok(Json(value))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearCacheResponse {
    pub success: bool,
    pub message: String,
}

async fn clear_cache(State(_state): State<AppState>) -> Result<Json<ClearCacheResponse>, ApiError> {
    let cache_dir = std::env::var("IMAGE_CACHE_DIR")
        .unwrap_or_else(|_| "/tmp/stripstream-image-cache".to_string());

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_size_mb: f64,
    pub file_count: u64,
    pub directory: String,
}

async fn get_cache_stats(State(_state): State<AppState>) -> Result<Json<CacheStats>, ApiError> {
    let cache_dir = std::env::var("IMAGE_CACHE_DIR")
        .unwrap_or_else(|_| "/tmp/stripstream-image-cache".to_string());

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

pub async fn get_settings_from_db(
    pool: &sqlx::PgPool,
) -> Result<AppSettings, ApiError> {
    let settings = get_settings_from_db_raw(pool).await?;
    
    let image_processing = settings
        .get("image_processing")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_else(|| ImageProcessingSettings {
            format: "webp".to_string(),
            quality: 85,
            filter: "lanczos3".to_string(),
            max_width: 2160,
        });

    let cache = settings
        .get("cache")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_else(|| CacheSettings {
            enabled: true,
            directory: "/tmp/stripstream-image-cache".to_string(),
            max_size_mb: 10000,
        });

    let limits = settings
        .get("limits")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_else(|| LimitsSettings {
            concurrent_renders: 4,
            timeout_seconds: 12,
            rate_limit_per_second: 120,
        });

    Ok(AppSettings {
        image_processing,
        cache,
        limits,
    })
}

async fn get_settings_from_db_raw(
    pool: &sqlx::PgPool,
) -> Result<std::collections::HashMap<String, Value>, ApiError> {
    let rows = sqlx::query(r#"SELECT key, value FROM app_settings"#)
        .fetch_all(pool)
        .await?;

    let mut settings = std::collections::HashMap::new();
    for row in rows {
        let key: String = row.get("key");
        let value: Value = row.get("value");
        settings.insert(key, value);
    }

    Ok(settings)
}
