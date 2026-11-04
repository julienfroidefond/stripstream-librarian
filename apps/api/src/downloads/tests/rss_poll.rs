//! Tests for the Prowlarr RSS poll: handler guards, multi-indexer fetch and
//! the end-to-end detection pipeline.
//!
//! Prowlarr is faked with `wiremock`; the database is a real migrated
//! temporary one.

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use sqlx::Row;
use stripstream_core::http::build_http_client_with_agent;
use uuid::Uuid;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::state::test_state;

// ── helpers ─────────────────────────────────────────────────────────────────

async fn configure_prowlarr(pool: &sqlx::PgPool, url: &str) {
    let value = serde_json::json!({ "url": url, "api_key": "test-key" });
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('prowlarr', $1) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(&value)
    .execute(pool)
    .await
    .unwrap();
}

async fn create_job(pool: &sqlx::PgPool, library_id: Option<Uuid>) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) \
         VALUES (gen_random_uuid(), $1, 'prowlarr_rss', 'running', NOW()) RETURNING id",
    )
    .bind(library_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO libraries (id, name, root_path) VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(name)
    .bind(format!("/libraries/{name}"))
    .fetch_one(pool)
    .await
    .unwrap()
}

/// An approved link whose external metadata still has unlinked volumes.
async fn seed_series_with_missing_volume(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    name: &str,
    volume: i32,
) -> Uuid {
    let series_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO series (id, library_id, name) VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap();

    let link_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO external_metadata_links \
             (id, library_id, series_id, provider, external_id, status, is_primary) \
         VALUES (gen_random_uuid(), $1, $2, 'google_books', 'ext', 'approved', TRUE) RETURNING id",
    )
    .bind(library_id)
    .bind(series_id)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO external_book_metadata (id, link_id, volume_number) \
         VALUES (gen_random_uuid(), $1, $2)",
    )
    .bind(link_id)
    .bind(volume)
    .execute(pool)
    .await
    .unwrap();

    series_id
}

fn release(guid: &str, title: &str) -> serde_json::Value {
    serde_json::json!({
        "guid": guid,
        "title": title,
        "size": 1024,
        "downloadUrl": format!("http://prowlarr/download/{guid}"),
        "indexer": "MockIndexer",
        "seeders": 7,
        "leechers": 1,
    })
}

async fn mount_indexers(server: &MockServer, indexers: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/api/v1/indexer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(indexers))
        .mount(server)
        .await;
}

async fn mount_search(server: &MockServer, indexer_id: i64, releases: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(query_param("indexerIds", indexer_id.to_string()))
        .respond_with(ResponseTemplate::new(200).set_body_json(releases))
        .mount(server)
        .await;
}

async fn job_status(pool: &sqlx::PgPool, job_id: Uuid) -> (String, Option<serde_json::Value>) {
    let row = sqlx::query("SELECT status, stats_json FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(pool)
        .await
        .unwrap();
    (row.get("status"), row.get("stats_json"))
}

// ── handler guards ──────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn start_rss_poll_requires_prowlarr_credentials(pool: sqlx::PgPool) {
    let state = test_state(pool.clone());

    // Migration 0043 seeds an empty `prowlarr` settings row.
    let err = start_rss_poll(State(state), Json(StartRssPollRequest { library_id: None }))
        .await
        .map(|_| ())
        .expect_err("an unconfigured Prowlarr must be rejected");

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert!(
        err.message.contains("Prowlarr"),
        "unexpected: {}",
        err.message
    );

    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM index_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        jobs, 0,
        "no job may be created when Prowlarr is unconfigured"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn start_rss_poll_rejects_an_invalid_library_id(pool: sqlx::PgPool) {
    configure_prowlarr(&pool, "http://127.0.0.1:1").await;
    let state = test_state(pool.clone());

    let err = start_rss_poll(
        State(state),
        Json(StartRssPollRequest {
            library_id: Some("not-a-uuid".to_string()),
        }),
    )
    .await
    .map(|_| ())
    .expect_err("a malformed library id must be rejected");

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert!(err.message.contains("invalid library_id"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn start_rss_poll_rejects_an_unknown_library(pool: sqlx::PgPool) {
    configure_prowlarr(&pool, "http://127.0.0.1:1").await;
    let state = test_state(pool.clone());

    let err = start_rss_poll(
        State(state),
        Json(StartRssPollRequest {
            library_id: Some(Uuid::new_v4().to_string()),
        }),
    )
    .await
    .map(|_| ())
    .expect_err("an unknown library must 404");

    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn start_rss_poll_returns_an_already_running_global_job(pool: sqlx::PgPool) {
    configure_prowlarr(&pool, "http://127.0.0.1:1").await;
    let existing = create_job(&pool, None).await;
    let state = test_state(pool.clone());

    let body = start_rss_poll(State(state), Json(StartRssPollRequest { library_id: None }))
        .await
        .unwrap()
        .0;

    assert_eq!(body["status"], "already_running");
    assert_eq!(body["id"], existing.to_string());

    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM index_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 1, "no second job may be created");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn start_rss_poll_returns_an_already_running_library_job(pool: sqlx::PgPool) {
    configure_prowlarr(&pool, "http://127.0.0.1:1").await;
    let lib = create_library(&pool, "rss_lib_running").await;
    let existing = create_job(&pool, Some(lib)).await;
    let state = test_state(pool.clone());

    let body = start_rss_poll(
        State(state),
        Json(StartRssPollRequest {
            library_id: Some(lib.to_string()),
        }),
    )
    .await
    .unwrap()
    .0;

    assert_eq!(body["status"], "already_running");
    assert_eq!(body["id"], existing.to_string());
}

// ── fetch_rss_releases ──────────────────────────────────────────────────────

#[tokio::test]
async fn fetch_rss_dedupes_by_guid_across_indexers() {
    let server = MockServer::start().await;
    mount_indexers(
        &server,
        serde_json::json!([{ "id": 1, "name": "Alpha" }, { "id": 2, "name": "Beta" }]),
    )
    .await;
    // Indexer 1 and 2 both return the same GUID, plus one unique to each.
    mount_search(
        &server,
        1,
        serde_json::json!([
            release("shared", "Asterix T01"),
            release("only-a", "Asterix T02")
        ]),
    )
    .await;
    mount_search(
        &server,
        2,
        serde_json::json!([
            release("shared", "Asterix T01"),
            release("only-b", "Asterix T03")
        ]),
    )
    .await;

    let client = build_http_client_with_agent(Duration::from_secs(10)).unwrap();
    let result = fetch_rss_releases(&client, &server.uri(), "key", &[7030])
        .await
        .unwrap();

    let mut guids: Vec<&str> = result.releases.iter().map(|r| r.guid.as_str()).collect();
    guids.sort_unstable();
    assert_eq!(guids, vec!["only-a", "only-b", "shared"]);

    assert_eq!(result.indexer_stats.len(), 2);
    for stat in &result.indexer_stats {
        assert_eq!(
            stat["count"], 2,
            "each indexer returned its own two releases"
        );
    }
    // "shared" is returned by both indexers but kept once, so the per-indexer
    // dedup counts add up to 3 rather than 4.
    let deduped_total: i64 = result
        .indexer_stats
        .iter()
        .map(|s| s["deduped"].as_i64().unwrap())
        .sum();
    assert_eq!(deduped_total, 3);
}

#[tokio::test]
async fn fetch_rss_records_a_failing_indexer_without_dropping_the_others() {
    let server = MockServer::start().await;
    mount_indexers(
        &server,
        serde_json::json!([{ "id": 1, "name": "Good" }, { "id": 2, "name": "Bad" }]),
    )
    .await;
    mount_search(
        &server,
        1,
        serde_json::json!([release("g1", "Asterix T01")]),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(query_param("indexerIds", "2"))
        .respond_with(ResponseTemplate::new(500).set_body_string("nope"))
        .mount(&server)
        .await;

    let client = build_http_client_with_agent(Duration::from_secs(10)).unwrap();
    let result = fetch_rss_releases(&client, &server.uri(), "key", &[7030])
        .await
        .unwrap();

    // The healthy indexer's release is still returned.
    assert_eq!(result.releases.len(), 1);
    assert_eq!(result.releases[0].guid, "g1");

    let bad = result
        .indexer_stats
        .iter()
        .find(|s| s["id"] == 2)
        .expect("missing stats for the failing indexer");
    assert_eq!(bad["count"], 0);
    assert!(
        bad["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Prowlarr returned 500"),
        "unexpected error: {}",
        bad["error"]
    );
}

#[tokio::test]
async fn fetch_rss_records_an_indexer_with_an_invalid_id() {
    let server = MockServer::start().await;
    mount_indexers(
        &server,
        serde_json::json!([
            { "id": 1, "name": "Good" },
            { "id": "not-a-number", "name": "Broken" }
        ]),
    )
    .await;
    mount_search(
        &server,
        1,
        serde_json::json!([release("g1", "Asterix T01")]),
    )
    .await;

    let client = build_http_client_with_agent(Duration::from_secs(10)).unwrap();
    let result = fetch_rss_releases(&client, &server.uri(), "key", &[7030])
        .await
        .unwrap();

    // The healthy indexer still contributes, and the unusable entry is counted
    // as an attempt so a fully malformed list cannot look like a success.
    assert_eq!(result.releases.len(), 1);
    assert_eq!(result.indexer_stats.len(), 2);
    let broken = result
        .indexer_stats
        .iter()
        .find(|s| s["name"] == "Broken")
        .expect("the unparseable indexer must stay visible");
    assert!(
        broken.get("error").is_some(),
        "an invalid id must produce an error stat"
    );
}

// ── process_rss_poll ────────────────────────────────────────────────────────

#[tokio::test]
async fn fetch_rss_rejects_a_non_success_indexer_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/indexer"))
        .respond_with(ResponseTemplate::new(503).set_body_string("prowlarr down"))
        .mount(&server)
        .await;

    let client = build_http_client_with_agent(Duration::from_secs(10)).unwrap();
    let error = fetch_rss_releases(&client, &server.uri(), "key", &[7030])
        .await
        .err()
        .expect("a non-2xx indexer list must not be swallowed into an empty result");

    assert!(
        error.contains("indexer list returned 503"),
        "unexpected: {error}"
    );
}

#[tokio::test]
async fn fetch_rss_rejects_an_unparseable_indexer_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/indexer"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = build_http_client_with_agent(Duration::from_secs(10)).unwrap();
    let error = fetch_rss_releases(&client, &server.uri(), "key", &[7030])
        .await
        .err()
        .expect("an unparseable indexer list must surface as an error");

    assert!(
        error.contains("Failed to parse Prowlarr indexer list"),
        "unexpected: {error}"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn process_rss_poll_fails_when_the_indexer_list_is_unavailable(pool: sqlx::PgPool) {
    let server = MockServer::start().await;
    let lib = create_library(&pool, "rss_indexer_list_failed").await;
    seed_series_with_missing_volume(&pool, lib, "Astérix", 1).await;

    configure_prowlarr(&pool, &server.uri()).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/indexer"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let job_id = create_job(&pool, None).await;
    process_rss_poll(&pool, job_id, None).await.unwrap();

    let row = sqlx::query("SELECT status, error_opt FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.get::<String, _>("status"),
        "failed",
        "an unreachable indexer list is a failed run, not an empty success"
    );
    assert!(
        row.get::<Option<String>, _>("error_opt")
            .unwrap_or_default()
            .contains("indexer list returned 500"),
        "the job must record why it failed"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn process_rss_poll_completes_early_when_there_is_no_target(pool: sqlx::PgPool) {
    configure_prowlarr(&pool, "http://127.0.0.1:1").await;
    let job_id = create_job(&pool, None).await;

    process_rss_poll(&pool, job_id, None).await.unwrap();

    let (status, stats) = job_status(&pool, job_id).await;
    assert_eq!(status, "success");
    assert_eq!(stats.unwrap()["message"], "No series with missing volumes");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn process_rss_poll_detects_and_stores_matching_releases(pool: sqlx::PgPool) {
    let server = MockServer::start().await;
    let lib = create_library(&pool, "rss_detect").await;
    let series_id = seed_series_with_missing_volume(&pool, lib, "Astérix", 1).await;

    configure_prowlarr(&pool, &server.uri()).await;
    mount_indexers(&server, serde_json::json!([{ "id": 1, "name": "Mock" }])).await;
    mount_search(
        &server,
        1,
        serde_json::json!([
            release("match", "Astérix T01 [CBZ]"),
            release("miss", "Blacksad T09 [CBZ]"),
        ]),
    )
    .await;

    let job_id = create_job(&pool, None).await;
    process_rss_poll(&pool, job_id, None).await.unwrap();

    let (status, stats) = job_status(&pool, job_id).await;
    let stats = stats.expect("stats_json missing");
    assert_eq!(status, "success");
    assert_eq!(stats["total_series"], 1);
    assert_eq!(stats["found"], 1);
    assert_eq!(stats["rss_releases_fetched"], 2);
    assert_eq!(stats["new_releases"], 1);

    // The matching release was stored for the series.
    let row = sqlx::query(
        "SELECT missing_count, available_releases FROM available_downloads WHERE series_id = $1",
    )
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i32, _>("missing_count"), 1);
    let releases: serde_json::Value = row.get("available_releases");
    let titles: Vec<&str> = releases
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Astérix T01 [CBZ]"]);

    // A `downloads_found` event documents the hit.
    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM index_job_events WHERE job_id = $1 AND event_type = 'downloads_found'",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn process_rss_poll_reports_when_nothing_matches(pool: sqlx::PgPool) {
    let server = MockServer::start().await;
    let lib = create_library(&pool, "rss_nomatch").await;
    seed_series_with_missing_volume(&pool, lib, "Astérix", 1).await;

    configure_prowlarr(&pool, &server.uri()).await;
    mount_indexers(&server, serde_json::json!([{ "id": 1, "name": "Mock" }])).await;
    mount_search(
        &server,
        1,
        serde_json::json!([release("x", "Blacksad T09 [CBZ]")]),
    )
    .await;

    let job_id = create_job(&pool, None).await;
    process_rss_poll(&pool, job_id, None).await.unwrap();

    let (status, stats) = job_status(&pool, job_id).await;
    let stats = stats.expect("stats_json missing");
    assert_eq!(status, "success");
    assert_eq!(stats["found"], 0);
    assert_eq!(stats["new_releases"], 0);

    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM index_job_events \
         WHERE job_id = $1 AND event_type = 'downloads_not_found'",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 1);

    // Nothing was stored for the series.
    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM available_downloads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn process_rss_poll_is_partial_when_some_indexers_fail(pool: sqlx::PgPool) {
    let server = MockServer::start().await;
    let lib = create_library(&pool, "rss_partial").await;
    seed_series_with_missing_volume(&pool, lib, "Astérix", 1).await;

    configure_prowlarr(&pool, &server.uri()).await;
    mount_indexers(
        &server,
        serde_json::json!([{ "id": 1, "name": "Good" }, { "id": 2, "name": "Bad" }]),
    )
    .await;
    mount_search(
        &server,
        1,
        serde_json::json!([release("match", "Astérix T01 [CBZ]")]),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(query_param("indexerIds", "2"))
        .respond_with(ResponseTemplate::new(500).set_body_string("nope"))
        .mount(&server)
        .await;

    let job_id = create_job(&pool, None).await;
    process_rss_poll(&pool, job_id, None).await.unwrap();

    let row = sqlx::query("SELECT status, error_opt, stats_json FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("status"), "partial");
    let stats: serde_json::Value = row.get("stats_json");
    let indexer_stats = stats["rss_indexer_stats"].as_array().unwrap();
    assert_eq!(indexer_stats.len(), 2);
    assert!(
        indexer_stats
            .iter()
            .any(|s| s["name"] == "Bad" && s.get("error").is_some()),
        "the failing indexer must stay visible in stats_json"
    );
    assert_eq!(
        stats["rss_releases"].as_array().map(Vec::len),
        Some(1),
        "a partial run must keep its RSS snapshot for the UI"
    );
    let error_opt: Option<String> = row.get("error_opt");
    assert!(
        error_opt.unwrap_or_default().contains("1/2"),
        "summary should name the failure ratio"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn process_rss_poll_is_failed_when_every_indexer_fails(pool: sqlx::PgPool) {
    let server = MockServer::start().await;
    let lib = create_library(&pool, "rss_failed").await;
    seed_series_with_missing_volume(&pool, lib, "Astérix", 1).await;

    configure_prowlarr(&pool, &server.uri()).await;
    mount_indexers(&server, serde_json::json!([{ "id": 1, "name": "Bad" }])).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(query_param("indexerIds", "1"))
        .respond_with(ResponseTemplate::new(500).set_body_string("nope"))
        .mount(&server)
        .await;

    let job_id = create_job(&pool, None).await;
    process_rss_poll(&pool, job_id, None).await.unwrap();

    let row = sqlx::query("SELECT status, error_opt FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert!(row
        .get::<Option<String>, _>("error_opt")
        .unwrap_or_default()
        .contains("1/1"));
}
