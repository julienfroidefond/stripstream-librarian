//! Integration tests for the `libraries` module.
//!
//! These tests lock the current behaviour of the handlers (no production fix
//! is included). A few known inconsistencies are asserted on purpose and
//! annotated with `LOCKED:` so they are easy to find later.

use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};
use std::time::Instant;

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock, Semaphore};
use uuid::Uuid;

use super::*;
use crate::index_jobs::RebuildRequest;
use crate::state::{
    DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
};

fn expect_err<T>(result: Result<T, ApiError>) -> ApiError {
    match result {
        Ok(_) => panic!("expected an error, got a success"),
        Err(err) => err,
    }
}

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

async fn insert_library_at(
    pool: &PgPool,
    name: &str,
    root_path: &str,
    created_at: chrono::DateTime<Utc>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path, created_at) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(name)
        .bind(root_path)
        .bind(created_at)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn create_library_row(pool: &PgPool, name: &str) -> Uuid {
    insert_library_at(
        pool,
        name,
        &format!("/libraries/{}-{}", name, Uuid::new_v4()),
        Utc::now(),
    )
    .await
}

async fn create_series_row(pool: &PgPool, library_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, genres) \
         VALUES (gen_random_uuid(), $1, $2, '{}') RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_book_row(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Option<Uuid>,
    title: &str,
    volume: Option<i32>,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO books (id, library_id, kind, title, series_id, volume, volume_type) \
         VALUES (gen_random_uuid(), $1, 'comic', $2, $3, $4, 'regular') RETURNING id",
    )
    .bind(library_id)
    .bind(title)
    .bind(series_id)
    .bind(volume)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn library_exists(pool: &PgPool, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM libraries WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn count_rows(pool: &PgPool, table: &str, library_id: Uuid) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE library_id = $1");
    sqlx::query_scalar(&sql)
        .bind(library_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn find_list(lists: &[LibraryResponse], id: Uuid) -> &LibraryResponse {
    lists
        .iter()
        .find(|entry| entry.id == id)
        .expect("library present in list response")
}

// ---------------------------------------------------------------------------
// create_library
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_library_rejects_blank_name(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let state = test_state(pool);

    let err = expect_err(
        create_library(
            State(state),
            Json(CreateLibraryRequest {
                name: "   ".to_string(),
                root_path: dir.path().to_string_lossy().to_string(),
            }),
        )
        .await,
    );

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_library_rejects_relative_root_path(pool: PgPool) {
    let state = test_state(pool);

    let err = expect_err(
        create_library(
            State(state),
            Json(CreateLibraryRequest {
                name: "Relative".to_string(),
                root_path: "relative/path".to_string(),
            }),
        )
        .await,
    );

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_library_rejects_missing_root_path(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let state = test_state(pool);
    let missing = dir.path().join("does-not-exist");

    let err = expect_err(
        create_library(
            State(state),
            Json(CreateLibraryRequest {
                name: "Missing".to_string(),
                root_path: missing.to_string_lossy().to_string(),
            }),
        )
        .await,
    );

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_library_rejects_file_root_path(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("not-a-directory.txt");
    std::fs::write(&file, "content").unwrap();
    let state = test_state(pool);

    let err = expect_err(
        create_library(
            State(state),
            Json(CreateLibraryRequest {
                name: "File".to_string(),
                root_path: file.to_string_lossy().to_string(),
            }),
        )
        .await,
    );

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_library_trims_name_and_applies_defaults(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_string_lossy().to_string();
    let state = test_state(pool.clone());

    let Json(dto) = create_library(
        State(state),
        Json(CreateLibraryRequest {
            name: "  Comics  ".to_string(),
            root_path: root.clone(),
        }),
    )
    .await
    .unwrap();

    assert_eq!(dto.name, "Comics");
    assert_eq!(dto.root_path, root);
    assert!(dto.enabled);
    assert!(!dto.monitor_enabled);
    assert!(!dto.watcher_enabled);
    assert_eq!(dto.scan_mode, "manual");
    assert_eq!(dto.metadata_refresh_mode, "manual");
    assert_eq!(dto.reading_status_push_mode, "manual");
    assert_eq!(dto.download_detection_mode, "manual");
    assert_eq!(dto.book_count, 0);
    assert_eq!(dto.series_count, 0);
    assert!(dto.thumbnail_book_ids.is_empty());
    assert!(dto.tags.is_empty());

    let (name, enabled): (String, bool) =
        sqlx::query_as("SELECT name, enabled FROM libraries WHERE id = $1")
            .bind(dto.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(name, "Comics");
    assert!(enabled);
}

// LOCKED: a duplicate root_path surfaces as HTTP 400 (bad_request) through the
// sqlx error mapping, not 409 Conflict.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_library_rejects_duplicate_root_path(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_string_lossy().to_string();
    let state = test_state(pool);

    let _ = create_library(
        State(state.clone()),
        Json(CreateLibraryRequest {
            name: "First".to_string(),
            root_path: root.clone(),
        }),
    )
    .await
    .unwrap();

    let err = expect_err(
        create_library(
            State(state),
            Json(CreateLibraryRequest {
                name: "Second".to_string(),
                root_path: root,
            }),
        )
        .await,
    );

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert!(
        err.message.contains("duplicate"),
        "message: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// list_libraries
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_libraries_is_empty_without_rows(pool: PgPool) {
    let Json(lists) = list_libraries(State(test_state(pool))).await.unwrap();
    assert!(lists.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_libraries_orders_by_created_at_desc(pool: PgPool) {
    let oldest = insert_library_at(
        &pool,
        "oldest",
        &format!("/libraries/oldest-{}", Uuid::new_v4()),
        Utc::now() - Duration::minutes(10),
    )
    .await;
    let middle = insert_library_at(
        &pool,
        "middle",
        &format!("/libraries/middle-{}", Uuid::new_v4()),
        Utc::now() - Duration::minutes(5),
    )
    .await;
    let newest = insert_library_at(
        &pool,
        "newest",
        &format!("/libraries/newest-{}", Uuid::new_v4()),
        Utc::now(),
    )
    .await;

    let Json(lists) = list_libraries(State(test_state(pool))).await.unwrap();
    let ids: Vec<Uuid> = lists.iter().map(|entry| entry.id).collect();

    assert_eq!(ids, vec![newest, middle, oldest]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_libraries_counts_books_and_series(pool: PgPool) {
    let populated = create_library_row(&pool, "populated").await;
    let empty = create_library_row(&pool, "empty").await;

    let s1 = create_series_row(&pool, populated, "S1").await;
    let s2 = create_series_row(&pool, populated, "S2").await;
    create_book_row(&pool, populated, Some(s1), "S1 Vol 1", Some(1)).await;
    create_book_row(&pool, populated, Some(s2), "S2 Vol 1", Some(1)).await;
    create_book_row(&pool, populated, None, "Loose", None).await;

    let Json(lists) = list_libraries(State(test_state(pool))).await.unwrap();

    let populated_entry = find_list(&lists, populated);
    assert_eq!(populated_entry.book_count, 3);
    // 2 named series + 1 "unclassified" bucket for the loose book.
    assert_eq!(populated_entry.series_count, 3);

    let empty_entry = find_list(&lists, empty);
    assert_eq!(empty_entry.book_count, 0);
    assert_eq!(empty_entry.series_count, 0);
    assert!(empty_entry.thumbnail_book_ids.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_libraries_thumbnail_ids_pick_first_book_per_series(pool: PgPool) {
    let lib = create_library_row(&pool, "thumbs").await;
    let alpha = create_series_row(&pool, lib, "Alpha").await;
    let bravo = create_series_row(&pool, lib, "Bravo").await;
    let charlie = create_series_row(&pool, lib, "Charlie").await;

    create_book_row(&pool, lib, Some(alpha), "Alpha Vol 2", Some(2)).await;
    let alpha_v1 = create_book_row(&pool, lib, Some(alpha), "Alpha Vol 1", Some(1)).await;
    let bravo_v1 = create_book_row(&pool, lib, Some(bravo), "Bravo Vol 1", Some(1)).await;
    let charlie_v1 = create_book_row(&pool, lib, Some(charlie), "Charlie Vol 1", Some(1)).await;
    let loose = create_book_row(&pool, lib, None, "Loose", None).await;

    let Json(lists) = list_libraries(State(test_state(pool))).await.unwrap();
    let entry = find_list(&lists, lib);

    // Ordered by COALESCE(series.name, 'unclassified'): Alpha < Bravo < Charlie < unclassified.
    assert_eq!(
        entry.thumbnail_book_ids,
        vec![alpha_v1, bravo_v1, charlie_v1, loose]
    );
}

// ---------------------------------------------------------------------------
// delete_library
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_library_returns_not_found_for_unknown_id(pool: PgPool) {
    let err = expect_err(delete_library(State(test_state(pool)), AxumPath(Uuid::new_v4())).await);
    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_library_cascades_books_and_series(pool: PgPool) {
    let lib = create_library_row(&pool, "cascade").await;
    let series = create_series_row(&pool, lib, "Cascade Series").await;
    create_book_row(&pool, lib, Some(series), "Cascade Vol 1", Some(1)).await;

    let Json(response) = delete_library(State(test_state(pool.clone())), AxumPath(lib))
        .await
        .unwrap();

    assert!(response.deleted);
    assert_eq!(response.id, lib);
    assert!(!library_exists(&pool, lib).await);
    assert_eq!(count_rows(&pool, "books", lib).await, 0);
    assert_eq!(count_rows(&pool, "series", lib).await, 0);
}

// ---------------------------------------------------------------------------
// scan_library
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn scan_library_returns_not_found(pool: PgPool) {
    let err =
        expect_err(scan_library(State(test_state(pool)), AxumPath(Uuid::new_v4()), None).await);
    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn scan_library_defaults_to_rebuild_job(pool: PgPool) {
    let lib = create_library_row(&pool, "scan-default").await;

    let Json(job) = scan_library(State(test_state(pool.clone())), AxumPath(lib), None)
        .await
        .unwrap();

    assert_eq!(job.library_id, Some(lib));
    assert_eq!(job.r#type, "rebuild");
    assert_eq!(job.status, "pending");

    let stored_type: String = sqlx::query_scalar("SELECT type FROM index_jobs WHERE id = $1")
        .bind(job.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored_type, "rebuild");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn scan_library_full_creates_full_rebuild_job(pool: PgPool) {
    let lib = create_library_row(&pool, "scan-full").await;
    let payload = Some(Json(RebuildRequest {
        library_id: None,
        full: Some(true),
        rescan: None,
    }));

    let Json(job) = scan_library(State(test_state(pool)), AxumPath(lib), payload)
        .await
        .unwrap();

    assert_eq!(job.r#type, "full_rebuild");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn scan_library_rescan_creates_rescan_job(pool: PgPool) {
    let lib = create_library_row(&pool, "scan-rescan").await;
    let payload = Some(Json(RebuildRequest {
        library_id: None,
        full: None,
        rescan: Some(true),
    }));

    let Json(job) = scan_library(State(test_state(pool)), AxumPath(lib), payload)
        .await
        .unwrap();

    assert_eq!(job.r#type, "rescan");
}

// LOCKED: when both flags are set the handler silently prefers `full`.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn scan_library_full_takes_precedence_over_rescan(pool: PgPool) {
    let lib = create_library_row(&pool, "scan-both").await;
    let payload = Some(Json(RebuildRequest {
        library_id: None,
        full: Some(true),
        rescan: Some(true),
    }));

    let Json(job) = scan_library(State(test_state(pool)), AxumPath(lib), payload)
        .await
        .unwrap();

    assert_eq!(job.r#type, "full_rebuild");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn scan_library_uses_path_id_over_payload_library_id(pool: PgPool) {
    let lib = create_library_row(&pool, "scan-path").await;
    let other = Uuid::new_v4();
    let payload = Some(Json(RebuildRequest {
        library_id: Some(other),
        full: None,
        rescan: None,
    }));

    let Json(job) = scan_library(State(test_state(pool)), AxumPath(lib), payload)
        .await
        .unwrap();

    assert_eq!(job.library_id, Some(lib));
}

// ---------------------------------------------------------------------------
// update_monitoring
// ---------------------------------------------------------------------------

fn monitoring_request(
    monitor_enabled: bool,
    scan_mode: &str,
    watcher_enabled: Option<bool>,
    metadata_refresh_mode: Option<&str>,
    download_detection_mode: Option<&str>,
) -> UpdateMonitoringRequest {
    UpdateMonitoringRequest {
        monitor_enabled,
        scan_mode: scan_mode.to_string(),
        watcher_enabled,
        metadata_refresh_mode: metadata_refresh_mode.map(str::to_string),
        download_detection_mode: download_detection_mode.map(str::to_string),
    }
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_monitoring_rejects_invalid_scan_mode(pool: PgPool) {
    let lib = create_library_row(&pool, "mon-scan").await;
    let err = expect_err(
        update_monitoring(
            State(test_state(pool)),
            AxumPath(lib),
            Json(monitoring_request(true, "yearly", None, None, None)),
        )
        .await,
    );
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_monitoring_rejects_invalid_metadata_refresh_mode(pool: PgPool) {
    let lib = create_library_row(&pool, "mon-refresh").await;
    let err = expect_err(
        update_monitoring(
            State(test_state(pool)),
            AxumPath(lib),
            Json(monitoring_request(
                false,
                "manual",
                None,
                Some("yearly"),
                None,
            )),
        )
        .await,
    );
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_monitoring_rejects_invalid_download_detection_mode(pool: PgPool) {
    let lib = create_library_row(&pool, "mon-download").await;
    let err = expect_err(
        update_monitoring(
            State(test_state(pool)),
            AxumPath(lib),
            Json(monitoring_request(
                false,
                "manual",
                None,
                None,
                Some("yearly"),
            )),
        )
        .await,
    );
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_monitoring_enables_schedule_and_defaults_watcher_off(pool: PgPool) {
    let lib = create_library_row(&pool, "mon-enable").await;

    let Json(dto) = update_monitoring(
        State(test_state(pool)),
        AxumPath(lib),
        Json(monitoring_request(true, "hourly", None, None, None)),
    )
    .await
    .unwrap();

    assert!(dto.monitor_enabled);
    assert_eq!(dto.scan_mode, "hourly");
    assert!(dto.next_scan_at.is_some());
    // LOCKED: an omitted watcher_enabled is treated as `false`.
    assert!(!dto.watcher_enabled);
    assert!(dto.next_metadata_refresh_at.is_none());
    assert!(dto.next_download_detection_at.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_monitoring_disabling_clears_next_scan_at(pool: PgPool) {
    let lib = create_library_row(&pool, "mon-disable").await;

    let Json(dto) = update_monitoring(
        State(test_state(pool)),
        AxumPath(lib),
        Json(monitoring_request(false, "manual", Some(false), None, None)),
    )
    .await
    .unwrap();

    assert!(!dto.monitor_enabled);
    assert_eq!(dto.scan_mode, "manual");
    assert!(dto.next_scan_at.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_monitoring_sets_next_refresh_when_not_manual(pool: PgPool) {
    let lib = create_library_row(&pool, "mon-refresh-set").await;

    let Json(dto) = update_monitoring(
        State(test_state(pool)),
        AxumPath(lib),
        Json(monitoring_request(
            false,
            "manual",
            Some(true),
            Some("daily"),
            Some("weekly"),
        )),
    )
    .await
    .unwrap();

    assert!(dto.watcher_enabled);
    assert_eq!(dto.metadata_refresh_mode, "daily");
    assert!(dto.next_metadata_refresh_at.is_some());
    assert_eq!(dto.download_detection_mode, "weekly");
    assert!(dto.next_download_detection_at.is_some());
}

// LOCKED: unlike `list_libraries`, the update response does not apply
// DISTINCT ON series, so it can return several books from the same series.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_monitoring_returns_refreshed_counts_without_distinct_series(pool: PgPool) {
    let lib = create_library_row(&pool, "mon-counts").await;
    let series = create_series_row(&pool, lib, "Counts Series").await;
    let first = create_book_row(&pool, lib, Some(series), "Counts Vol 1", Some(1)).await;
    let second = create_book_row(&pool, lib, Some(series), "Counts Vol 2", Some(2)).await;

    let Json(dto) = update_monitoring(
        State(test_state(pool)),
        AxumPath(lib),
        Json(monitoring_request(true, "daily", Some(true), None, None)),
    )
    .await
    .unwrap();

    assert_eq!(dto.book_count, 2);
    assert_eq!(dto.series_count, 1);
    assert_eq!(dto.thumbnail_book_ids.len(), 2);
    assert!(dto.thumbnail_book_ids.contains(&first));
    assert!(dto.thumbnail_book_ids.contains(&second));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_monitoring_returns_not_found(pool: PgPool) {
    let err = expect_err(
        update_monitoring(
            State(test_state(pool)),
            AxumPath(Uuid::new_v4()),
            Json(monitoring_request(true, "manual", None, None, None)),
        )
        .await,
    );
    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// update_metadata_provider
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_metadata_provider_sets_and_clears_values(pool: PgPool) {
    let lib = create_library_row(&pool, "metadata").await;
    let state = test_state(pool.clone());

    let Json(dto) = update_metadata_provider(
        State(state.clone()),
        AxumPath(lib),
        Json(UpdateMetadataProviderRequest {
            metadata_provider: Some("comicvine".to_string()),
            fallback_metadata_provider: Some("anilist".to_string()),
        }),
    )
    .await
    .unwrap();
    assert_eq!(dto.metadata_provider.as_deref(), Some("comicvine"));
    assert_eq!(dto.fallback_metadata_provider.as_deref(), Some("anilist"));

    let Json(cleared) = update_metadata_provider(
        State(state),
        AxumPath(lib),
        Json(UpdateMetadataProviderRequest {
            metadata_provider: Some(String::new()),
            fallback_metadata_provider: None,
        }),
    )
    .await
    .unwrap();
    assert_eq!(cleared.metadata_provider, None);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_metadata_provider_returns_not_found(pool: PgPool) {
    let err = expect_err(
        update_metadata_provider(
            State(test_state(pool)),
            AxumPath(Uuid::new_v4()),
            Json(UpdateMetadataProviderRequest {
                metadata_provider: Some("comicvine".to_string()),
                fallback_metadata_provider: None,
            }),
        )
        .await,
    );
    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// update_reading_status_provider
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_reading_status_provider_rejects_invalid_mode(pool: PgPool) {
    let lib = create_library_row(&pool, "reading-invalid").await;
    let err = expect_err(
        update_reading_status_provider(
            State(test_state(pool)),
            AxumPath(lib),
            Json(UpdateReadingStatusProviderRequest {
                reading_status_provider: Some("anilist".to_string()),
                reading_status_push_mode: Some("yearly".to_string()),
            }),
        )
        .await,
    );
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_reading_status_provider_sets_mode_and_schedule(pool: PgPool) {
    let lib = create_library_row(&pool, "reading-set").await;
    let state = test_state(pool.clone());

    let Json(value) = update_reading_status_provider(
        State(state.clone()),
        AxumPath(lib),
        Json(UpdateReadingStatusProviderRequest {
            reading_status_provider: Some("anilist".to_string()),
            reading_status_push_mode: Some("daily".to_string()),
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        value["reading_status_provider"],
        serde_json::json!("anilist")
    );
    assert_eq!(
        value["reading_status_push_mode"],
        serde_json::json!("daily")
    );

    let next: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT next_reading_status_push_at FROM libraries WHERE id = $1")
            .bind(lib)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(next.is_some());

    let Json(manual) = update_reading_status_provider(
        State(state),
        AxumPath(lib),
        Json(UpdateReadingStatusProviderRequest {
            reading_status_provider: None,
            reading_status_push_mode: Some("manual".to_string()),
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        manual["reading_status_push_mode"],
        serde_json::json!("manual")
    );

    let next_after: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT next_reading_status_push_at FROM libraries WHERE id = $1")
            .bind(lib)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(next_after.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_reading_status_provider_returns_not_found(pool: PgPool) {
    let err = expect_err(
        update_reading_status_provider(
            State(test_state(pool)),
            AxumPath(Uuid::new_v4()),
            Json(UpdateReadingStatusProviderRequest {
                reading_status_provider: Some("anilist".to_string()),
                reading_status_push_mode: Some("daily".to_string()),
            }),
        )
        .await,
    );
    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// update_tags
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_tags_replaces_tags(pool: PgPool) {
    let lib = create_library_row(&pool, "tags").await;

    let Json(value) = update_tags(
        State(test_state(pool.clone())),
        AxumPath(lib),
        Json(UpdateTagsRequest {
            tags: vec!["a".to_string(), "b".to_string()],
        }),
    )
    .await
    .unwrap();
    assert_eq!(value["tags"], serde_json::json!(["a", "b"]));

    let stored: Vec<String> = sqlx::query_scalar("SELECT tags FROM libraries WHERE id = $1")
        .bind(lib)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, vec!["a".to_string(), "b".to_string()]);

    let Json(replaced) = update_tags(
        State(test_state(pool.clone())),
        AxumPath(lib),
        Json(UpdateTagsRequest {
            tags: vec!["c".to_string()],
        }),
    )
    .await
    .unwrap();
    assert_eq!(replaced["tags"], serde_json::json!(["c"]));

    let stored_after: Vec<String> = sqlx::query_scalar("SELECT tags FROM libraries WHERE id = $1")
        .bind(lib)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored_after, vec!["c".to_string()]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_tags_returns_not_found(pool: PgPool) {
    let err = expect_err(
        update_tags(
            State(test_state(pool)),
            AxumPath(Uuid::new_v4()),
            Json(UpdateTagsRequest {
                tags: vec!["a".to_string()],
            }),
        )
        .await,
    );
    assert_eq!(err.status, StatusCode::NOT_FOUND);
}
