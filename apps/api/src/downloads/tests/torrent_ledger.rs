use sqlx::PgPool;
use uuid::Uuid;

use super::*;

async fn seed_library(pool: &PgPool) -> Uuid {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(library_id)
        .bind(format!("lib_{library_id}"))
        .bind(format!("/libraries/{library_id}"))
        .execute(pool)
        .await
        .unwrap();
    library_id
}

async fn seed_row(pool: &PgPool, status: TorrentStatus) -> Uuid {
    let library_id = seed_library(pool).await;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO torrent_downloads (id, library_id, series_name, status) \
         VALUES ($1, $2, 'Amulet', $3)",
    )
    .bind(id)
    .bind(library_id)
    .bind(status.as_str())
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn status_of(pool: &PgPool, id: Uuid) -> TorrentStatus {
    let raw: String = sqlx::query_scalar("SELECT status FROM torrent_downloads WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    TorrentStatus::try_from(raw.as_str()).unwrap()
}

#[test]
fn only_volume_coverage_makes_an_import_partial() {
    assert_eq!(import_outcome(&[1], &[1, 2], false), ImportOutcome::Partial);
    assert_eq!(
        import_outcome(&[1, 2], &[1, 2], false),
        ImportOutcome::Imported
    );
    assert_eq!(import_outcome(&[1], &[], false), ImportOutcome::Imported);
    assert_eq!(
        import_outcome(&[1], &[1, 2], true),
        ImportOutcome::Imported,
        "replace mode imports every file, coverage does not apply"
    );
    assert_eq!(
        import_outcome(&[], &[1], false),
        ImportOutcome::NoFilesImported
    );
}

#[test]
fn retry_accepts_every_unfinished_import() {
    for status in [
        TorrentStatus::Completed,
        TorrentStatus::Error,
        TorrentStatus::NoFilesImported,
        TorrentStatus::Partial,
    ] {
        assert!(status.is_retryable(), "{status} must be retryable");
    }
    // `importing` is owned by a running import: a retry would race it.
    for status in [
        TorrentStatus::Downloading,
        TorrentStatus::Importing,
        TorrentStatus::Imported,
    ] {
        assert!(!status.is_retryable(), "{status} must not be retryable");
    }
}

#[test]
fn every_status_round_trips_through_its_name() {
    for status in [
        TorrentStatus::Downloading,
        TorrentStatus::Completed,
        TorrentStatus::Importing,
        TorrentStatus::Imported,
        TorrentStatus::Partial,
        TorrentStatus::NoFilesImported,
        TorrentStatus::Error,
    ] {
        assert_eq!(TorrentStatus::try_from(status.as_str()).unwrap(), status);
    }
    assert!(TorrentStatus::try_from("failed").is_err());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_full_lifecycle_moves_the_row_through_every_status(pool: PgPool) {
    let id = seed_row(&pool, TorrentStatus::Downloading).await;

    mark_completed(&pool, id, "/downloads/sl-42").await.unwrap();
    assert_eq!(status_of(&pool, id).await, TorrentStatus::Completed);
    let (content_path, progress): (String, f32) =
        sqlx::query_as("SELECT content_path, progress FROM torrent_downloads WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(content_path, "/downloads/sl-42");
    assert_eq!(progress, 1.0);

    begin_import(&pool, id).await.unwrap();
    assert_eq!(status_of(&pool, id).await, TorrentStatus::Importing);

    let report = serde_json::json!({"imported": [], "skipped": [], "total_source_files": 2});
    finish_import(&pool, id, ImportOutcome::Imported, &report)
        .await
        .unwrap();
    assert_eq!(status_of(&pool, id).await, TorrentStatus::Imported);

    // A finished import must never be overwritten by a late poller.
    let error = fail(&pool, id, "late failure").await.unwrap_err();
    assert!(matches!(
        error,
        TorrentTransitionError::NotAllowed {
            from: TorrentStatus::Imported,
            ..
        }
    ));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_short_import_is_recorded_as_partial(pool: PgPool) {
    let id = seed_row(&pool, TorrentStatus::Importing).await;

    let report = serde_json::json!({"imported": [{"volume": 1}], "skipped": []});
    finish_import(&pool, id, ImportOutcome::Partial, &report)
        .await
        .unwrap();

    assert_eq!(status_of(&pool, id).await, TorrentStatus::Partial);
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT imported_files FROM torrent_downloads WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored, report);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_transition_from_an_unexpected_status_is_refused(pool: PgPool) {
    let id = seed_row(&pool, TorrentStatus::Imported).await;

    let error = begin_import(&pool, id).await.unwrap_err();
    assert!(matches!(
        error,
        TorrentTransitionError::NotAllowed {
            from: TorrentStatus::Imported,
            action: "start an import for",
            ..
        }
    ));
    assert_eq!(
        status_of(&pool, id).await,
        TorrentStatus::Imported,
        "a refused transition must leave the row untouched"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_missing_row_is_reported_as_not_found(pool: PgPool) {
    let id = Uuid::new_v4();

    let error = mark_completed(&pool, id, "/downloads/x").await.unwrap_err();
    assert!(matches!(error, TorrentTransitionError::NotFound(missing) if missing == id));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn restart_scope_failed_only_refuses_a_retryable_row(pool: PgPool) {
    let id = seed_row(&pool, TorrentStatus::Partial).await;

    let error = restart(&pool, id, RestartScope::Failed).await.unwrap_err();
    assert!(matches!(
        error,
        TorrentTransitionError::NotAllowed {
            from: TorrentStatus::Partial,
            ..
        }
    ));

    restart(&pool, id, RestartScope::Retryable).await.unwrap();
    assert_eq!(status_of(&pool, id).await, TorrentStatus::Downloading);

    let error = restart(&pool, id, RestartScope::Retryable)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        TorrentTransitionError::NotAllowed {
            from: TorrentStatus::Downloading,
            ..
        }
    ));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn progress_is_ignored_once_the_row_left_downloading(pool: PgPool) {
    let completed = seed_row(&pool, TorrentStatus::Completed).await;
    let update = ProgressUpdate {
        progress: 0.5,
        speed: 10,
        eta: 3,
        file_size: Some(100),
        seeders: Some(2),
        name: Some("Amulet T01"),
        downloaded: 0,
    };
    record_progress(&pool, completed, &update).await.unwrap();

    let progress: f32 = sqlx::query_scalar("SELECT progress FROM torrent_downloads WHERE id = $1")
        .bind(completed)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(progress, 0.0, "a finished row must not receive progress");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn progress_keeps_known_values_and_clears_the_stall_marker(pool: PgPool) {
    let id = seed_row(&pool, TorrentStatus::Downloading).await;
    sqlx::query(
        "UPDATE torrent_downloads SET file_size = 500, seeders = 4, stall_started_at = NOW() \
         WHERE id = $1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let update = ProgressUpdate {
        progress: 0.5,
        speed: 100,
        eta: 10,
        // `None` means "the backend did not report a size" and must not erase 500.
        file_size: None,
        seeders: None,
        name: None,
        downloaded: 250,
    };
    record_progress(&pool, id, &update).await.unwrap();

    let (progress, file_size, seeders, stall): (
        f32,
        Option<i64>,
        Option<i32>,
        Option<DateTime<Utc>>,
    ) = sqlx::query_as(
        "SELECT progress, file_size, seeders, stall_started_at \
             FROM torrent_downloads WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(progress, 0.5);
    assert_eq!(file_size, Some(500));
    assert_eq!(seeders, Some(4));
    assert!(stall.is_none(), "transferred bytes clear the stall marker");

    // A backend that reports a zero total does not know the size: the stored
    // value survives, exactly like a `None`.
    record_progress(
        &pool,
        id,
        &ProgressUpdate {
            file_size: Some(0),
            ..update
        },
    )
    .await
    .unwrap();
    let file_size: Option<i64> =
        sqlx::query_scalar("SELECT file_size FROM torrent_downloads WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(file_size, Some(500));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn arm_stall_keeps_the_first_zero_byte_instant(pool: PgPool) {
    let id = seed_row(&pool, TorrentStatus::Downloading).await;
    let update = ProgressUpdate {
        progress: 0.0,
        speed: 0,
        eta: 0,
        file_size: Some(100),
        seeders: Some(0),
        name: Some("Amulet T01"),
        downloaded: 0,
    };

    let first = arm_stall(&pool, id, &update).await.unwrap().unwrap();
    let second = arm_stall(&pool, id, &update).await.unwrap().unwrap();
    assert_eq!(first, second, "the marker must not move on each poll");

    record_progress(
        &pool,
        id,
        &ProgressUpdate {
            downloaded: 1,
            ..update
        },
    )
    .await
    .unwrap();
    assert!(arm_stall(&pool, id, &update).await.unwrap().is_some());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_late_resolved_hash_is_recorded(pool: PgPool) {
    let id = seed_row(&pool, TorrentStatus::Downloading).await;

    set_hash(&pool, id, "AABBCC").await.unwrap();

    let hash: Option<String> =
        sqlx::query_scalar("SELECT qb_hash FROM torrent_downloads WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(hash.as_deref(), Some("AABBCC"));
}

/// The import pipeline hands its result to the ledger: a torrent whose expected
/// volume is missing must land in `partial` so the retry button keeps meaning
/// something.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn an_import_missing_a_tome_lands_in_partial(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let library_dir = root.path().join("lib");
    let downloads_dir = root.path().join("dl");
    std::fs::create_dir_all(&library_dir).unwrap();
    std::fs::create_dir_all(&downloads_dir).unwrap();
    std::fs::write(downloads_dir.join("Amulet - 01.cbz"), b"fake").unwrap();

    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(library_id)
        .bind(format!("lib_{library_id}"))
        .bind(library_dir.to_string_lossy().into_owned())
        .execute(&pool)
        .await
        .unwrap();

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO torrent_downloads \
         (id, library_id, series_name, expected_volumes, content_path, client, status) \
         VALUES ($1, $2, 'Amulet', ARRAY[1, 2], $3, 'alldebrid', 'completed')",
    )
    .bind(id)
    .bind(library_id)
    .bind(downloads_dir.to_string_lossy().into_owned())
    .execute(&pool)
    .await
    .unwrap();

    let library_root =
        stripstream_core::paths::LibraryRoot::new(stripstream_core::paths::LIBRARIES_DB_ROOT);
    super::super::torrent_import::process_torrent_import(pool.clone(), &library_root, id)
        .await
        .unwrap();

    assert_eq!(status_of(&pool, id).await, TorrentStatus::Partial);

    let report: serde_json::Value =
        sqlx::query_scalar("SELECT imported_files FROM torrent_downloads WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let destination = report["imported"][0]["destination"]
        .as_str()
        .expect("the imported volume must be reported");
    assert_eq!(std::fs::read(destination).unwrap(), b"fake");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn begin_import_is_an_exclusive_reservation(pool: PgPool) {
    let id = seed_row(&pool, TorrentStatus::Completed).await;

    let (first, second) = tokio::join!(begin_import(&pool, id), begin_import(&pool, id));
    let results = [first, second];
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "exactly one concurrent caller reserves the row"
    );
    assert!(results.iter().any(|r| matches!(
        r,
        Err(TorrentTransitionError::NotAllowed {
            from: TorrentStatus::Importing,
            ..
        })
    )));

    // A row already importing is never reserved again.
    assert!(begin_import(&pool, id).await.is_err());
    assert_eq!(status_of(&pool, id).await, TorrentStatus::Importing);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn interrupted_imports_are_handed_back_and_listed_for_resume(pool: PgPool) {
    let interrupted = seed_row(&pool, TorrentStatus::Importing).await;
    let finished = seed_row(&pool, TorrentStatus::Imported).await;
    sqlx::query("UPDATE torrent_downloads SET content_path = '/downloads/x' WHERE id = $1")
        .bind(interrupted)
        .execute(&pool)
        .await
        .unwrap();
    // A completed row without content path cannot be resumed.
    seed_row(&pool, TorrentStatus::Completed).await;

    assert_eq!(recover_interrupted_imports(&pool).await.unwrap(), 1);
    assert_eq!(
        status_of(&pool, interrupted).await,
        TorrentStatus::Completed
    );
    assert_eq!(status_of(&pool, finished).await, TorrentStatus::Imported);

    let resumable = stale_completed(&pool, chrono::Duration::zero())
        .await
        .unwrap();
    assert_eq!(
        resumable,
        vec![interrupted],
        "a row without files is not resumable"
    );
    assert!(stale_completed(&pool, chrono::Duration::hours(1))
        .await
        .unwrap()
        .is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn abandon_import_only_releases_a_reserved_row(pool: PgPool) {
    let importing = seed_row(&pool, TorrentStatus::Importing).await;
    let partial = seed_row(&pool, TorrentStatus::Partial).await;

    assert!(abandon_import(&pool, importing, "boom").await.unwrap());
    assert_eq!(status_of(&pool, importing).await, TorrentStatus::Error);
    assert!(!abandon_import(&pool, partial, "boom").await.unwrap());
    assert_eq!(status_of(&pool, partial).await, TorrentStatus::Partial);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn clear_stall_forgets_the_marker(pool: PgPool) {
    let id = seed_row(&pool, TorrentStatus::Downloading).await;
    sqlx::query("UPDATE torrent_downloads SET stall_started_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    clear_stall(&pool, id).await.unwrap();

    let stall: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT stall_started_at FROM torrent_downloads WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(stall.is_none());
}

/// Seed a library rooted in a temp dir and a `completed` row whose content is
/// `downloads_dir`.
async fn seed_import(
    pool: &PgPool,
    library_dir: &std::path::Path,
    downloads_dir: &std::path::Path,
    expected: &[i32],
) -> Uuid {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(library_id)
        .bind(format!("lib_{library_id}"))
        .bind(library_dir.to_string_lossy().into_owned())
        .execute(pool)
        .await
        .unwrap();

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO torrent_downloads \
         (id, library_id, series_name, expected_volumes, content_path, client, status) \
         VALUES ($1, $2, 'Amulet', $3, $4, 'alldebrid', 'completed')",
    )
    .bind(id)
    .bind(library_id)
    .bind(expected)
    .bind(downloads_dir.to_string_lossy().into_owned())
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn report_of(pool: &PgPool, id: Uuid) -> serde_json::Value {
    sqlx::query_scalar("SELECT imported_files FROM torrent_downloads WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A multi-volume pack covers every tome it contains: the import is complete
/// and the report lists all of them.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_multi_volume_pack_covers_every_tome(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let library_dir = root.path().join("lib");
    let downloads_dir = root.path().join("dl");
    std::fs::create_dir_all(&library_dir).unwrap();
    std::fs::create_dir_all(&downloads_dir).unwrap();
    std::fs::write(downloads_dir.join("Amulet T01-T03.cbz"), b"pack").unwrap();
    let id = seed_import(&pool, &library_dir, &downloads_dir, &[1, 2, 3]).await;

    let library_root =
        stripstream_core::paths::LibraryRoot::new(stripstream_core::paths::LIBRARIES_DB_ROOT);
    super::super::torrent_import::process_torrent_import(pool.clone(), &library_root, id)
        .await
        .unwrap();

    assert_eq!(status_of(&pool, id).await, TorrentStatus::Imported);
    let report = report_of(&pool, id).await;
    assert_eq!(
        report["imported"][0]["volumes"],
        serde_json::json!([1, 2, 3])
    );
    assert_eq!(report["imported"][0]["volume"], 1);
}

/// A retry after a partial import counts the volumes the first run already
/// moved into the library: once the missing tome arrives, the row is complete.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_retry_completes_a_partial_import(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let library_dir = root.path().join("lib");
    let downloads_dir = root.path().join("dl");
    std::fs::create_dir_all(&library_dir).unwrap();
    std::fs::create_dir_all(&downloads_dir).unwrap();
    std::fs::write(downloads_dir.join("Amulet - 01.cbz"), b"one").unwrap();
    let id = seed_import(&pool, &library_dir, &downloads_dir, &[1, 2]).await;
    let library_root =
        stripstream_core::paths::LibraryRoot::new(stripstream_core::paths::LIBRARIES_DB_ROOT);

    super::super::torrent_import::process_torrent_import(pool.clone(), &library_root, id)
        .await
        .unwrap();
    assert_eq!(status_of(&pool, id).await, TorrentStatus::Partial);
    assert!(downloads_dir.exists(), "a partial import keeps its sources");

    std::fs::write(downloads_dir.join("Amulet - 02.cbz"), b"two").unwrap();
    super::super::torrent_import::process_torrent_import(pool.clone(), &library_root, id)
        .await
        .unwrap();

    assert_eq!(status_of(&pool, id).await, TorrentStatus::Imported);
    let report = report_of(&pool, id).await;
    let mut volumes: Vec<i64> = report["imported"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["volume"].as_i64().unwrap())
        .collect();
    volumes.sort_unstable();
    assert_eq!(volumes, vec![1, 2]);
}
