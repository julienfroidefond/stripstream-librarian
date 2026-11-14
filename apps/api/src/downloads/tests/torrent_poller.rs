//! qBittorrent poller, retry and source-retention rules of `torrent_import`.

use axum::http::StatusCode;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

// ─── Pure rules ─────────────────────────────────────────────────────────────

#[test]
fn qbittorrent_states_are_classified() {
    for state in [
        "uploading",
        "stalledUP",
        "pausedUP",
        "stoppedUP",
        "queuedUP",
    ] {
        assert_eq!(
            classify_qb_state(state),
            QbStateAction::Completed,
            "{state}"
        );
    }
    for state in ["pausedDL", "stoppedDL"] {
        assert_eq!(classify_qb_state(state), QbStateAction::Paused, "{state}");
    }
    assert_eq!(classify_qb_state("stalledDL"), QbStateAction::Stalled);
    for state in ["error", "missingFiles"] {
        assert_eq!(classify_qb_state(state), QbStateAction::Failed, "{state}");
    }
    for state in [
        "downloading",
        "metaDL",
        "queuedDL",
        "checkingDL",
        "forcedDL",
        "moving",
    ] {
        assert_eq!(
            classify_qb_state(state),
            QbStateAction::InProgress,
            "{state}"
        );
    }
}

#[test]
fn a_stall_expires_only_after_the_timeout() {
    let now = Utc::now();
    let timeout = Duration::from_secs(30 * 60);
    assert!(!stall_expired(now, now, timeout));
    assert!(!stall_expired(
        now - chrono::Duration::minutes(29),
        now,
        timeout
    ));
    assert!(stall_expired(
        now - chrono::Duration::minutes(30),
        now,
        timeout
    ));
}

#[test]
fn missing_hashes_ignore_case() {
    let tracked = vec!["AABB".to_string(), "ccdd".to_string(), "eeff".to_string()];
    assert_eq!(missing_hashes(&tracked, &["aabb", "CCDD"]), vec!["eeff"]);
    assert!(missing_hashes(&tracked, &["aabb", "ccdd", "eeff"]).is_empty());
}

#[test]
fn retry_plan_refuses_a_row_without_files() {
    let exists = |p: &str| p == "/downloads/here";
    assert_eq!(
        retry_plan(Some("/downloads/here"), "qbittorrent", exists),
        RetryPlan::Import
    );
    assert_eq!(
        retry_plan(Some("/downloads/gone"), "qbittorrent", exists),
        RetryPlan::SourcesGone
    );
    assert_eq!(
        retry_plan(None, "qbittorrent", exists),
        RetryPlan::NothingToImport
    );
    assert_eq!(
        retry_plan(None, "alldebrid", exists),
        RetryPlan::RestartAlldebrid
    );
    assert_eq!(
        retry_plan(Some("/downloads/gone"), "alldebrid", exists),
        RetryPlan::RestartAlldebrid
    );
}

#[test]
fn only_a_complete_import_consumes_its_sources() {
    assert!(should_clean_sources(ImportOutcome::Imported));
    assert!(!should_clean_sources(ImportOutcome::Partial));
    assert!(!should_clean_sources(ImportOutcome::NoFilesImported));
}

#[test]
fn a_pack_reports_every_volume_and_legacy_reports_still_parse() {
    let pack = ImportedFile::new(&[3, 1, 2], "s".into(), "d".into(), false);
    assert_eq!(pack.volume, 1, "the legacy field keeps the lowest volume");
    assert_eq!(pack.covered_volumes(), vec![1, 2, 3]);

    let oneshot = ImportedFile::new(&[], "s".into(), "d".into(), false);
    assert_eq!(oneshot.covered_volumes(), vec![0]);

    let legacy: ImportedFile =
        serde_json::from_value(serde_json::json!({"volume": 4, "source": "a", "destination": "b"}))
            .unwrap();
    assert_eq!(legacy.covered_volumes(), vec![4]);
}

#[test]
fn a_retry_report_keeps_the_previous_run_files() {
    let previous = vec![
        ImportedFile::new(&[1], "a".into(), "/lib/T01.cbz".into(), false),
        ImportedFile::new(&[2], "b".into(), "/lib/T02.cbz".into(), false),
    ];
    let current = vec![
        ImportedFile::new(&[2], "b2".into(), "/lib/T02.cbz".into(), true),
        ImportedFile::new(&[3], "c".into(), "/lib/T03.cbz".into(), false),
    ];
    let merged = merge_imported_files(previous, &current);
    assert_eq!(covered_volumes(&merged), vec![1, 2, 3]);
    assert_eq!(merged.len(), 3, "a destination is reported once");
}

// ─── Poller against a fake qBittorrent ──────────────────────────────────────

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

/// A `downloading` qBittorrent row created `age` ago.
async fn seed_qb_row(
    pool: &PgPool,
    library_id: Uuid,
    hash: &str,
    age: chrono::Duration,
    qb_owned: bool,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO torrent_downloads \
         (id, library_id, series_name, client, status, qb_hash, qb_owned, created_at) \
         VALUES ($1, $2, 'Amulet', 'qbittorrent', 'downloading', $3, $4, NOW() - make_interval(secs => $5))",
    )
    .bind(id)
    .bind(library_id)
    .bind(hash)
    .bind(qb_owned)
    .bind(age.num_seconds() as f64)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn row_state(pool: &PgPool, id: Uuid) -> (String, Option<DateTime<Utc>>) {
    sqlx::query_as("SELECT status, stall_started_at FROM torrent_downloads WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A fake qBittorrent answering `torrents/info` with `torrents`.
async fn fake_qbittorrent(pool: &PgPool, torrents: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/login"))
        .respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "SID=abc; path=/"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(torrents))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/torrents/delete"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('qbittorrent', $1) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(serde_json::json!({
        "url": server.uri(), "username": "admin", "password": "pw", "enabled": true
    }))
    .execute(pool)
    .await
    .unwrap();
    server
}

async fn deleted_hashes(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.url.path() == "/api/v2/torrents/delete")
        .map(|r| String::from_utf8_lossy(&r.body).into_owned())
        .collect()
}

fn torrent(hash: &str, state: &str) -> serde_json::Value {
    serde_json::json!({"hash": hash, "state": state, "progress": 0.4, "total_size": 100})
}

fn test_root() -> stripstream_core::paths::LibraryRoot {
    stripstream_core::paths::LibraryRoot::new(stripstream_core::paths::LIBRARIES_DB_ROOT)
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn paused_and_fresh_stalled_torrents_are_kept(pool: PgPool) {
    let library_id = seed_library(&pool).await;
    let old = chrono::Duration::hours(1);
    let paused = seed_qb_row(&pool, library_id, "aa01", old, true).await;
    let stopped = seed_qb_row(&pool, library_id, "aa02", old, true).await;
    let stalled = seed_qb_row(&pool, library_id, "aa03", old, true).await;
    let server = fake_qbittorrent(
        &pool,
        serde_json::json!([
            torrent("aa01", "pausedDL"),
            torrent("aa02", "stoppedDL"),
            torrent("aa03", "stalledDL"),
        ]),
    )
    .await;

    assert!(poll_qbittorrent_downloads(&pool, &test_root())
        .await
        .unwrap());

    assert_eq!(row_state(&pool, paused).await.0, "downloading");
    assert_eq!(row_state(&pool, stopped).await.0, "downloading");
    let (status, stall) = row_state(&pool, stalled).await;
    assert_eq!(status, "downloading", "a fresh stall gets a grace period");
    assert!(stall.is_some(), "the stall start is recorded");
    assert!(deleted_hashes(&server).await.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_stall_past_the_timeout_fails_and_a_resumed_one_is_cleared(pool: PgPool) {
    let library_id = seed_library(&pool).await;
    let old = chrono::Duration::hours(3);
    let stalled = seed_qb_row(&pool, library_id, "bb01", old, true).await;
    let resumed = seed_qb_row(&pool, library_id, "bb02", old, true).await;
    sqlx::query(
        "UPDATE torrent_downloads SET stall_started_at = NOW() - INTERVAL '2 hours' \
         WHERE id = ANY($1)",
    )
    .bind(vec![stalled, resumed])
    .execute(&pool)
    .await
    .unwrap();
    let server = fake_qbittorrent(
        &pool,
        serde_json::json!([torrent("bb01", "stalledDL"), torrent("bb02", "downloading")]),
    )
    .await;

    poll_qbittorrent_downloads(&pool, &test_root())
        .await
        .unwrap();

    assert_eq!(row_state(&pool, stalled).await.0, "error");
    let (status, stall) = row_state(&pool, resumed).await;
    assert_eq!(status, "downloading");
    assert!(stall.is_none(), "progress again clears the stall marker");
    let deleted = deleted_hashes(&server).await;
    assert_eq!(deleted.len(), 1);
    assert!(deleted[0].contains("bb01"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn failed_torrents_are_removed_only_when_stripstream_added_them(pool: PgPool) {
    let library_id = seed_library(&pool).await;
    let old = chrono::Duration::hours(1);
    let owned = seed_qb_row(&pool, library_id, "cc01", old, true).await;
    let adopted = seed_qb_row(&pool, library_id, "cc02", old, false).await;
    let server = fake_qbittorrent(
        &pool,
        serde_json::json!([torrent("cc01", "error"), torrent("cc02", "missingFiles")]),
    )
    .await;

    poll_qbittorrent_downloads(&pool, &test_root())
        .await
        .unwrap();

    assert_eq!(row_state(&pool, owned).await.0, "error");
    assert_eq!(row_state(&pool, adopted).await.0, "error");
    let deleted = deleted_hashes(&server).await;
    assert_eq!(
        deleted.len(),
        1,
        "the adopted torrent must stay: {deleted:?}"
    );
    assert!(deleted[0].contains("cc01"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_torrent_gone_from_qbittorrent_fails_after_a_grace_period(pool: PgPool) {
    let library_id = seed_library(&pool).await;
    let gone = seed_qb_row(&pool, library_id, "dd01", chrono::Duration::hours(1), true).await;
    let just_added = seed_qb_row(&pool, library_id, "dd02", chrono::Duration::zero(), true).await;
    // The stored hash is uppercase: qBittorrent's lowercase answer must match.
    let listed = seed_qb_row(&pool, library_id, "DD03", chrono::Duration::hours(1), true).await;
    fake_qbittorrent(&pool, serde_json::json!([torrent("dd03", "downloading")])).await;

    poll_qbittorrent_downloads(&pool, &test_root())
        .await
        .unwrap();

    assert_eq!(row_state(&pool, gone).await.0, "error");
    assert_eq!(row_state(&pool, just_added).await.0, "downloading");
    assert_eq!(row_state(&pool, listed).await.0, "downloading");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_failed_qbittorrent_answer_never_fails_a_torrent(pool: PgPool) {
    let library_id = seed_library(&pool).await;
    let id = seed_qb_row(&pool, library_id, "ee01", chrono::Duration::hours(1), true).await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/login"))
        .respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "SID=abc"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('qbittorrent', $1) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(serde_json::json!({
        "url": server.uri(), "username": "admin", "password": "pw", "enabled": true
    }))
    .execute(&pool)
    .await
    .unwrap();

    assert!(poll_qbittorrent_downloads(&pool, &test_root())
        .await
        .is_err());
    assert_eq!(row_state(&pool, id).await.0, "downloading");
}

// ─── Resume of interrupted imports ──────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_stale_completed_row_is_resumed(pool: PgPool) {
    let library_id = seed_library(&pool).await;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO torrent_downloads (id, library_id, series_name, client, status, content_path) \
         VALUES ($1, $2, 'Amulet', 'alldebrid', 'completed', '/downloads/does-not-exist')",
    )
    .bind(id)
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    resume_stale_completed(&pool, &test_root(), chrono::Duration::zero()).await;

    let status: String = sqlx::query_scalar("SELECT status FROM torrent_downloads WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(status, "completed", "the import was reserved and started");
}

// ─── Retry endpoint ─────────────────────────────────────────────────────────

async fn seed_retry_row(
    pool: &PgPool,
    status: &str,
    client: &str,
    content_path: Option<&str>,
) -> Uuid {
    let library_id = seed_library(pool).await;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO torrent_downloads (id, library_id, series_name, client, status, content_path) \
         VALUES ($1, $2, 'Amulet', $3, $4, $5)",
    )
    .bind(id)
    .bind(library_id)
    .bind(client)
    .bind(status)
    .bind(content_path)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn status_of(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM torrent_downloads WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn retrying_a_running_import_is_a_conflict(pool: PgPool) {
    let id = seed_retry_row(&pool, "importing", "qbittorrent", Some("/downloads/x")).await;
    let state = crate::state::test_state(pool.clone());

    let err = retry_torrent_import(State(state), Path(id))
        .await
        .unwrap_err();
    assert_eq!(err.status, StatusCode::CONFLICT);
    assert_eq!(status_of(&pool, id).await, "importing");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn retrying_a_download_that_never_completed_is_refused(pool: PgPool) {
    let id = seed_retry_row(&pool, "error", "qbittorrent", None).await;
    let state = crate::state::test_state(pool.clone());

    let err = retry_torrent_import(State(state), Path(id))
        .await
        .unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(status_of(&pool, id).await, "error");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_retry_reserves_the_row_before_answering(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().to_string_lossy().into_owned();
    let id = seed_retry_row(&pool, "completed", "qbittorrent", Some(&content)).await;
    let state = crate::state::test_state(pool.clone());

    let _ = retry_torrent_import(State(state), Path(id)).await.unwrap();
    assert_ne!(
        status_of(&pool, id).await,
        "completed",
        "a completed row is retryable and reserved on the spot"
    );
}

// ─── Delete endpoint ────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn deleting_a_partial_row_removes_only_torrents_stripstream_added(pool: PgPool) {
    let library_id = seed_library(&pool).await;
    let server = fake_qbittorrent(&pool, serde_json::json!([])).await;
    let state = crate::state::test_state(pool.clone());

    let mut ids = Vec::new();
    for (hash, owned) in [("ff01", true), ("ff02", false)] {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO torrent_downloads \
             (id, library_id, series_name, client, status, qb_hash, qb_owned, content_path) \
             VALUES ($1, $2, 'Amulet', 'qbittorrent', 'partial', $3, $4, '/downloads/x')",
        )
        .bind(id)
        .bind(library_id)
        .bind(hash)
        .bind(owned)
        .execute(&pool)
        .await
        .unwrap();
        ids.push(id);
    }

    for id in ids {
        let _ = delete_torrent_download(State(state.clone()), Path(id))
            .await
            .unwrap();
    }

    let deleted = deleted_hashes(&server).await;
    assert_eq!(
        deleted.len(),
        1,
        "the adopted torrent must stay: {deleted:?}"
    );
    assert!(deleted[0].contains("ff01"));
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM torrent_downloads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

// ─── Source cleanup order ───────────────────────────────────────────────────

/// A fake qBittorrent whose `torrents/delete` answers `delete_status`.
async fn fake_qbittorrent_with_delete(
    pool: &PgPool,
    torrents: serde_json::Value,
    delete_status: u16,
) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/login"))
        .respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "SID=abc; path=/"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(torrents))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/torrents/delete"))
        .respond_with(ResponseTemplate::new(delete_status))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/torrents/removeCategories"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('qbittorrent', $1) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(serde_json::json!({
        "url": server.uri(), "username": "admin", "password": "pw", "enabled": true
    }))
    .execute(pool)
    .await
    .unwrap();
    server
}

fn staging_with_file(root: &tempfile::TempDir, id: Uuid) -> std::path::PathBuf {
    let dir = root.path().join(format!("sl-{id}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Amulet T01.cbz"), b"data").unwrap();
    dir
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn an_owned_torrent_is_deleted_before_its_staging_dir(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let dir = staging_with_file(&root, id);
    let server = fake_qbittorrent_with_delete(&pool, serde_json::json!([]), 200).await;

    release_sources(
        &pool,
        Some(dir.clone()),
        id,
        "qbittorrent",
        Some("ab01"),
        true,
        "[T]",
    )
    .await;

    assert_eq!(deleted_hashes(&server).await.len(), 1);
    assert!(
        !dir.exists(),
        "the staging dir goes once the torrent is gone"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_refused_torrent_delete_keeps_the_staging_dir(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let dir = staging_with_file(&root, id);
    let server = fake_qbittorrent_with_delete(&pool, serde_json::json!([]), 500).await;

    release_sources(
        &pool,
        Some(dir.clone()),
        id,
        "qbittorrent",
        Some("ab01"),
        true,
        "[T]",
    )
    .await;

    assert_eq!(deleted_hashes(&server).await.len(), 1);
    assert!(
        dir.join("Amulet T01.cbz").exists(),
        "files under a live torrent must stay"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn an_unreachable_qbittorrent_keeps_the_staging_dir(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let dir = staging_with_file(&root, id);
    // No qBittorrent configured at all.

    release_sources(
        &pool,
        Some(dir.clone()),
        id,
        "qbittorrent",
        Some("ab01"),
        true,
        "[T]",
    )
    .await;

    assert!(dir.join("Amulet T01.cbz").exists());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn an_adopted_torrent_storing_data_in_the_staging_dir_is_left_alone(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let dir = staging_with_file(&root, id);
    let server = fake_qbittorrent_with_delete(
        &pool,
        serde_json::json!([{
            "hash": "ab01", "state": "uploading",
            "save_path": dir.to_string_lossy(),
            "content_path": dir.join("Amulet T01.cbz").to_string_lossy(),
        }]),
        200,
    )
    .await;

    release_sources(
        &pool,
        Some(dir.clone()),
        id,
        "qbittorrent",
        Some("ab01"),
        false,
        "[T]",
    )
    .await;

    assert!(
        deleted_hashes(&server).await.is_empty(),
        "never delete the user's torrent"
    );
    assert!(
        dir.join("Amulet T01.cbz").exists(),
        "never delete the user's data"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn an_adopted_torrent_stored_elsewhere_lets_the_staging_dir_go(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let dir = staging_with_file(&root, id);
    let server = fake_qbittorrent_with_delete(
        &pool,
        serde_json::json!([{
            "hash": "ab01", "state": "uploading",
            "save_path": "/elsewhere/manga", "content_path": "/elsewhere/manga/Amulet",
        }]),
        200,
    )
    .await;

    release_sources(
        &pool,
        Some(dir.clone()),
        id,
        "qbittorrent",
        Some("ab01"),
        false,
        "[T]",
    )
    .await;

    assert!(deleted_hashes(&server).await.is_empty());
    assert!(!dir.exists());
}
