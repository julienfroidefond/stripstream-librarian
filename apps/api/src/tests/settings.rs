//! Integration tests for the `settings` module.
//!
//! Handlers are called directly with a hand-built [`AppState`] backed by an
//! ephemeral Postgres provisioned by `#[sqlx::test]`. Filesystem-backed handlers
//! (`clear_cache`, `get_cache_stats`, `get_thumbnail_stats`) run against real
//! `tempfile` directories.
//!
//! No production fix is included: every assertion locks current behaviour.

use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};
use std::time::Instant;

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use tempfile::TempDir;
use tokio::sync::{Mutex, RwLock, Semaphore};
use uuid::Uuid;

use super::*;
use crate::state::{
    DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

fn test_state(pool: PgPool) -> AppState {
    AppState {
        pool,
        bootstrap_token: Arc::from("test-token"),
        page_cache: Arc::new(Mutex::new(crate::state::PageCache::new(1))),
        disk_cache_stats: Arc::new(Mutex::new(None::<DiskCacheStatsSnapshot>)),
        page_render_locks: Arc::new(PageRenderLocks::new(1)),
        page_render_limit: Arc::new(Semaphore::new(1)),
        metrics: Arc::new(Metrics {
            requests_total: AtomicU64::new(0),
            page_cache_hits: AtomicU64::new(0),
            page_cache_misses: AtomicU64::new(0),
        }),
        read_rate_limit: Arc::new(Mutex::new(ReadRateLimit {
            window_started_at: Instant::now(),
            requests_in_window: 0,
        })),
        settings: Arc::new(RwLock::new(DynamicSettings::default())),
        prowlarr_fetch_lock: Arc::new(Mutex::new(())),
        pending_tg_auth: Arc::new(Mutex::new(None)),
        telegram_download_limit: Arc::new(Semaphore::new(1)),
        telegram_abort_handles: Arc::new(Mutex::new(HashMap::new())),
    }
}

fn expect_err<T>(result: Result<T, ApiError>) -> ApiError {
    match result {
        Ok(_) => panic!("expected an error result"),
        Err(err) => err,
    }
}

fn key_path(key: &str) -> AxumPath<String> {
    AxumPath(key.to_string())
}

fn id_path(id: Uuid) -> AxumPath<Uuid> {
    AxumPath(id)
}

fn setting_body(value: Value) -> Json<UpdateSettingRequest> {
    Json(UpdateSettingRequest { value })
}

fn write_file(dir: &std::path::Path, name: &str, bytes: usize) {
    std::fs::write(dir.join(name), vec![0u8; bytes]).unwrap();
}

async fn get_setting_row(pool: &PgPool, key: &str) -> Value {
    sqlx::query_scalar("SELECT value FROM app_settings WHERE key = $1")
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn count_status_mappings(pool: &PgPool, provider_status: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM status_mappings WHERE provider_status = $1")
        .bind(provider_status)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// get_settings / get_setting
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_settings_returns_seeded_keys_as_object(pool: PgPool) {
    let state = test_state(pool);

    let Json(value) = get_settings(State(state)).await.unwrap();

    let obj = value.as_object().unwrap();
    assert!(obj.contains_key("limits"));
    assert!(obj.contains_key("cache"));
    assert!(obj.contains_key("image_processing"));
    assert!(obj["limits"].is_object());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_settings_includes_newly_written_key(pool: PgPool) {
    let state = test_state(pool.clone());

    let _ = update_setting(
        State(state.clone()),
        key_path("custom_flag"),
        setting_body(json!({ "on": true })),
    )
    .await
    .unwrap();

    let Json(value) = get_settings(State(state)).await.unwrap();
    assert_eq!(value["custom_flag"], json!({ "on": true }));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_setting_returns_null_for_unknown_key(pool: PgPool) {
    let state = test_state(pool);

    // LOCKED: the OpenAPI path documents a 404, but a missing key yields 200/null.
    let Json(value) = get_setting(State(state), key_path("does-not-exist"))
        .await
        .unwrap();

    assert_eq!(value, Value::Null);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_setting_returns_stored_value(pool: PgPool) {
    let state = test_state(pool.clone());

    let _ = update_setting(
        State(state.clone()),
        key_path("custom_list"),
        setting_body(json!([1, 2, 3])),
    )
    .await
    .unwrap();

    let Json(value) = get_setting(State(state), key_path("custom_list"))
        .await
        .unwrap();

    assert_eq!(value, json!([1, 2, 3]));
}

// ---------------------------------------------------------------------------
// update_setting
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_setting_inserts_and_persists_new_key(pool: PgPool) {
    let state = test_state(pool.clone());

    let Json(returned) = update_setting(
        State(state),
        key_path("custom_new"),
        setting_body(json!({ "a": 1 })),
    )
    .await
    .unwrap();

    assert_eq!(returned, json!({ "a": 1 }));
    assert_eq!(
        get_setting_row(&pool, "custom_new").await,
        json!({ "a": 1 })
    );

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM app_settings WHERE key = 'custom_new'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_setting_overwrites_and_bumps_updated_at(pool: PgPool) {
    let state = test_state(pool.clone());
    sqlx::query(
        "INSERT INTO app_settings (key, value, updated_at) \
         VALUES ('custom_ts', '{\"v\": 1}'::jsonb, '2000-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let _ = update_setting(
        State(state),
        key_path("custom_ts"),
        setting_body(json!({ "v": 2 })),
    )
    .await
    .unwrap();

    assert_eq!(get_setting_row(&pool, "custom_ts").await, json!({ "v": 2 }));
    let updated_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM app_settings WHERE key = 'custom_ts'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let old: DateTime<Utc> = "2000-01-01T00:00:00Z".parse().unwrap();
    assert!(updated_at > old);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_setting_limits_refreshes_runtime_settings(pool: PgPool) {
    let state = test_state(pool.clone());

    let _ = update_setting(
        State(state.clone()),
        key_path("limits"),
        setting_body(json!({ "rate_limit_per_second": 7, "timeout_seconds": 42 })),
    )
    .await
    .unwrap();

    let settings = state.settings.read().await;
    assert_eq!(settings.rate_limit_per_second, 7);
    assert_eq!(settings.timeout_seconds, 42);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_setting_image_processing_clamps_quality(pool: PgPool) {
    let state = test_state(pool.clone());

    let _ = update_setting(
        State(state.clone()),
        key_path("image_processing"),
        setting_body(json!({
            "format": "png",
            "quality": 150,
            "filter": "nearest",
            "max_width": 800
        })),
    )
    .await
    .unwrap();

    {
        let settings = state.settings.read().await;
        assert_eq!(settings.image_format, "png");
        assert_eq!(settings.image_quality, 100);
        assert_eq!(settings.image_filter, "nearest");
        assert_eq!(settings.image_max_width, 800);
    }

    let _ = update_setting(
        State(state.clone()),
        key_path("image_processing"),
        setting_body(json!({ "quality": 0 })),
    )
    .await
    .unwrap();

    assert_eq!(state.settings.read().await.image_quality, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_setting_cache_updates_page_cache_and_resets_disk_snapshot(pool: PgPool) {
    let state = test_state(pool.clone());
    let dir = TempDir::new().unwrap();
    *state.disk_cache_stats.lock().await = Some(DiskCacheStatsSnapshot {
        directory: "/old".to_string(),
        total_size_bytes: 10,
        file_count: 1,
        collected_at: Instant::now(),
    });

    let _ = update_setting(
        State(state.clone()),
        key_path("cache"),
        setting_body(json!({
            "directory": dir.path().to_string_lossy(),
            "memory_max_size_mb": 4
        })),
    )
    .await
    .unwrap();

    assert_eq!(
        state.settings.read().await.cache_directory,
        dir.path().to_string_lossy().to_string()
    );
    assert_eq!(state.settings.read().await.page_cache_max_size_mb, 4);
    assert_eq!(
        state.page_cache.lock().await.max_size_bytes(),
        4 * 1024 * 1024
    );
    assert!(state.disk_cache_stats.lock().await.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_setting_unrelated_key_leaves_runtime_settings_untouched(pool: PgPool) {
    let state = test_state(pool.clone());
    let before = {
        let s = state.settings.read().await;
        (
            s.rate_limit_per_second,
            s.image_format.clone(),
            s.cache_directory.clone(),
            s.page_cache_max_size_mb,
        )
    };

    let _ = update_setting(
        State(state.clone()),
        key_path("custom_noop"),
        setting_body(json!({ "x": 1 })),
    )
    .await
    .unwrap();

    let after = {
        let s = state.settings.read().await;
        (
            s.rate_limit_per_second,
            s.image_format.clone(),
            s.cache_directory.clone(),
            s.page_cache_max_size_mb,
        )
    };
    assert_eq!(before, after);
    assert_eq!(
        get_setting_row(&pool, "custom_noop").await,
        json!({ "x": 1 })
    );
}

// ---------------------------------------------------------------------------
// clear_cache
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn clear_cache_missing_directory_reports_nothing_to_clear(pool: PgPool) {
    let state = test_state(pool);
    let missing = std::env::temp_dir().join(format!("stl-missing-{}", Uuid::new_v4()));
    state.settings.write().await.cache_directory = missing.to_string_lossy().to_string();
    state
        .page_cache
        .lock()
        .await
        .put("k".into(), Arc::from(vec![7u8; 32]));

    let Json(resp) = clear_cache(State(state.clone())).await.unwrap();

    assert!(resp.success);
    assert_eq!(
        resp.message,
        format!(
            "Cache directory '{}' does not exist, nothing to clear",
            missing.to_string_lossy()
        )
    );
    assert_eq!(state.page_cache.lock().await.len(), 0);
    assert!(state.disk_cache_stats.lock().await.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn clear_cache_removes_directory_and_empties_memory_cache(pool: PgPool) {
    let state = test_state(pool);
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), "a.bin", 128);
    state.settings.write().await.cache_directory = dir.path().to_string_lossy().to_string();
    state
        .page_cache
        .lock()
        .await
        .put("k".into(), Arc::from(vec![7u8; 32]));

    let Json(resp) = clear_cache(State(state.clone())).await.unwrap();

    assert!(resp.success);
    assert!(resp.message.contains("cleared successfully"));
    assert!(!dir.path().exists());
    assert_eq!(state.page_cache.lock().await.len(), 0);
    assert!(state.disk_cache_stats.lock().await.is_none());
}

// ---------------------------------------------------------------------------
// get_cache_stats
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_cache_stats_reports_disk_and_memory_usage(pool: PgPool) {
    let state = test_state(pool);
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), "a.bin", 1024 * 1024);
    state.settings.write().await.cache_directory = dir.path().to_string_lossy().to_string();
    state
        .page_cache
        .lock()
        .await
        .put("k".into(), Arc::from(vec![9u8; 512]));

    let Json(stats) = get_cache_stats(State(state.clone())).await.unwrap();

    assert_eq!(stats.file_count, 1);
    assert!((stats.total_size_mb - 1.0).abs() < 1e-9);
    assert_eq!(stats.directory, dir.path().to_string_lossy().to_string());
    assert_eq!(stats.memory_page_count, 1);
    assert!((stats.memory_size_mb - 512.0 / 1024.0 / 1024.0).abs() < 1e-9);
    assert_eq!(stats.memory_max_size_mb, 1.0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_cache_stats_reuses_cached_snapshot_within_ttl(pool: PgPool) {
    let state = test_state(pool);
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), "a.bin", 10);
    state.settings.write().await.cache_directory = dir.path().to_string_lossy().to_string();

    let Json(first) = get_cache_stats(State(state.clone())).await.unwrap();
    assert_eq!(first.file_count, 1);

    write_file(dir.path(), "b.bin", 20);
    let Json(second) = get_cache_stats(State(state)).await.unwrap();
    assert_eq!(second.file_count, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_cache_stats_missing_directory_returns_zero(pool: PgPool) {
    let state = test_state(pool);
    let missing = std::env::temp_dir().join(format!("stl-missing-{}", Uuid::new_v4()));
    state.settings.write().await.cache_directory = missing.to_string_lossy().to_string();

    let Json(stats) = get_cache_stats(State(state.clone())).await.unwrap();

    assert_eq!(stats.file_count, 0);
    assert_eq!(stats.total_size_mb, 0.0);
    assert_eq!(stats.directory, missing.to_string_lossy().to_string());
    assert!(state.disk_cache_stats.lock().await.is_some());
}

// ---------------------------------------------------------------------------
// get_thumbnail_stats
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_thumbnail_stats_uses_configured_directory(pool: PgPool) {
    let state = test_state(pool);
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), "a.webp", 1024 * 1024);
    write_file(dir.path(), "b.webp", 1024 * 1024);
    let _ = update_setting(
        State(state.clone()),
        key_path("thumbnail"),
        setting_body(json!({ "directory": dir.path().to_string_lossy() })),
    )
    .await
    .unwrap();

    let Json(stats) = get_thumbnail_stats(State(state)).await.unwrap();

    assert_eq!(stats.file_count, 2);
    assert!((stats.total_size_mb - 2.0).abs() < 1e-9);
    assert_eq!(stats.directory, dir.path().to_string_lossy().to_string());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_thumbnail_stats_missing_directory_returns_zero(pool: PgPool) {
    let state = test_state(pool);
    let missing = std::env::temp_dir().join(format!("stl-thumbs-{}", Uuid::new_v4()));
    let _ = update_setting(
        State(state.clone()),
        key_path("thumbnail"),
        setting_body(json!({ "directory": missing.to_string_lossy() })),
    )
    .await
    .unwrap();

    let Json(stats) = get_thumbnail_stats(State(state)).await.unwrap();

    assert_eq!(stats.file_count, 0);
    assert_eq!(stats.total_size_mb, 0.0);
    assert_eq!(stats.directory, missing.to_string_lossy().to_string());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_thumbnail_stats_falls_back_when_setting_has_no_directory(pool: PgPool) {
    let state = test_state(pool);
    let _ = update_setting(
        State(state.clone()),
        key_path("thumbnail"),
        setting_body(json!({ "unrelated": true })),
    )
    .await
    .unwrap();

    let Json(stats) = get_thumbnail_stats(State(state)).await.unwrap();

    assert_eq!(stats.directory, "/data/thumbnails");
}

// ---------------------------------------------------------------------------
// status mappings
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_status_mappings_returns_seed_and_sorts_nulls_last(pool: PgPool) {
    let state = test_state(pool.clone());
    let finished: Uuid =
        sqlx::query_scalar("SELECT id FROM status_mappings WHERE provider_status = 'finished'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let _ = delete_status_mapping(State(state.clone()), id_path(finished))
        .await
        .unwrap();

    let Json(rows) = list_status_mappings(State(state)).await.unwrap();

    assert!(rows.len() >= 11);
    let releasing = rows
        .iter()
        .find(|r| r.provider_status == "releasing")
        .unwrap();
    assert_eq!(releasing.mapped_status.as_deref(), Some("ongoing"));
    let last = rows.last().unwrap();
    assert_eq!(last.provider_status, "finished");
    assert!(last.mapped_status.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn upsert_status_mapping_lowercases_and_is_idempotent(pool: PgPool) {
    let state = test_state(pool.clone());

    let Json(first) = upsert_status_mapping(
        State(state.clone()),
        Json(UpsertStatusMappingRequest {
            provider_status: "NewStatus".to_string(),
            mapped_status: "mapped-a".to_string(),
        }),
    )
    .await
    .unwrap();
    assert_eq!(first.provider_status, "newstatus");
    assert_eq!(first.mapped_status.as_deref(), Some("mapped-a"));
    assert_eq!(count_status_mappings(&pool, "newstatus").await, 1);

    let Json(second) = upsert_status_mapping(
        State(state),
        Json(UpsertStatusMappingRequest {
            provider_status: "NEWSTATUS".to_string(),
            mapped_status: "mapped-b".to_string(),
        }),
    )
    .await
    .unwrap();
    assert_eq!(second.id, first.id);
    assert_eq!(second.provider_status, "newstatus");
    assert_eq!(second.mapped_status.as_deref(), Some("mapped-b"));
    assert_eq!(count_status_mappings(&pool, "newstatus").await, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_status_mapping_unmaps_and_keeps_row(pool: PgPool) {
    let state = test_state(pool.clone());
    let id: Uuid =
        sqlx::query_scalar("SELECT id FROM status_mappings WHERE provider_status = 'cancelled'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let Json(first) = delete_status_mapping(State(state.clone()), id_path(id))
        .await
        .unwrap();
    assert_eq!(first.provider_status, "cancelled");
    assert!(first.mapped_status.is_none());
    assert_eq!(count_status_mappings(&pool, "cancelled").await, 1);

    // LOCKED: deleting twice still returns 200 because the row is only unmapped.
    let Json(second) = delete_status_mapping(State(state), id_path(id))
        .await
        .unwrap();
    assert!(second.mapped_status.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_status_mapping_unknown_id_is_not_found(pool: PgPool) {
    let state = test_state(pool);

    let err = expect_err(delete_status_mapping(State(state), id_path(Uuid::new_v4())).await);

    assert_eq!(err.status, StatusCode::NOT_FOUND);
    assert_eq!(err.message, "status mapping not found");
}
