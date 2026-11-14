use super::*;
use axum::http::StatusCode;

async fn set_qbittorrent(pool: &sqlx::PgPool, value: serde_json::Value) {
    sqlx::query("UPDATE app_settings SET value = $1 WHERE key = 'qbittorrent'")
        .bind(value)
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn errors_when_not_configured(pool: sqlx::PgPool) {
    sqlx::query("DELETE FROM app_settings WHERE key = 'qbittorrent'")
        .execute(&pool)
        .await
        .unwrap();

    let err = load_qbittorrent_config(&pool).await.unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert!(
        err.message.contains("not configured"),
        "got: {}",
        err.message
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn errors_when_disabled(pool: sqlx::PgPool) {
    set_qbittorrent(
        &pool,
        serde_json::json!({
            "url": "http://qb:8080", "username": "admin", "password": "pw", "enabled": false
        }),
    )
    .await;

    let err = load_qbittorrent_config(&pool).await.unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert!(err.message.contains("disabled"), "got: {}", err.message);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn enabled_flag_defaults_to_true_when_absent(pool: sqlx::PgPool) {
    // Configurations saved before the flag existed must keep working.
    set_qbittorrent(
        &pool,
        serde_json::json!({ "url": "http://qb:8080/", "username": "admin", "password": "pw" }),
    )
    .await;

    let (url, username, password) = load_qbittorrent_config(&pool).await.unwrap();
    assert_eq!(url, "http://qb:8080"); // trailing slash trimmed
    assert_eq!(username, "admin");
    assert_eq!(password, "pw");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn loads_when_explicitly_enabled(pool: sqlx::PgPool) {
    set_qbittorrent(
        &pool,
        serde_json::json!({
            "url": "http://qb", "username": "admin", "password": "pw", "enabled": true
        }),
    )
    .await;

    assert!(load_qbittorrent_config(&pool).await.is_ok());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn errors_when_credentials_are_incomplete(pool: sqlx::PgPool) {
    set_qbittorrent(
        &pool,
        serde_json::json!({ "url": "http://qb", "username": "", "password": "pw", "enabled": true }),
    )
    .await;

    let err = load_qbittorrent_config(&pool).await.unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

// ─── URL safety (SSRF hardening) ───────────────────────────────────────────

#[test]
fn rejects_loopback_link_local_and_reserved_ip_ranges() {
    for ip in [
        "127.0.0.1",
        "169.254.169.254",
        "0.0.0.0",
        "224.0.0.1",
        "255.255.255.255",
        "::1",
        "fe80::1",
        "ff02::1",
        "::ffff:127.0.0.1",
    ] {
        assert!(
            !is_allowed_torrent_ip(ip.parse().unwrap()),
            "{ip} should not be considered a valid torrent target"
        );
    }
}

#[test]
fn accepts_public_and_lan_ip_ranges() {
    // LAN addresses are expected: self-hosted indexers and Prowlarr proxies
    // commonly live on private Docker networks.
    for ip in [
        "8.8.8.8",
        "1.1.1.1",
        "10.0.0.1",
        "172.16.0.1",
        "192.168.1.1",
        "100.64.0.1",
        "fc00::1",
        "2606:4700:4700::1111",
    ] {
        assert!(
            is_allowed_torrent_ip(ip.parse().unwrap()),
            "{ip} should be an accepted torrent target"
        );
    }
}

#[tokio::test]
async fn refuses_non_http_schemes_and_non_routable_hosts() {
    for url in [
        "file:///etc/passwd",
        "ftp://example.com/torrent",
        "http://localhost:8080/torrent",
        "http://metadata.google.internal/computeMetadata/v1/",
        "http://169.254.169.254/latest/meta-data",
        "http://127.0.0.1:9696/download",
        "http://[::1]:9696/download",
    ] {
        let parsed = reqwest::Url::parse(url).expect("test URL parses");
        assert!(
            ensure_safe_remote_url(&parsed).await.is_err(),
            "{url} should be refused"
        );
    }

    for url in [
        "https://8.8.8.8/torrent",
        "http://192.168.1.10:9696/download/123",
    ] {
        let parsed = reqwest::Url::parse(url).unwrap();
        assert!(
            ensure_safe_remote_url(&parsed).await.is_ok(),
            "{url} should be accepted"
        );
    }
}

// ─── add_torrent against a fake qBittorrent ─────────────────────────────────

/// A fake qBittorrent whose `torrents/info?hashes=…` reports `existing`.
async fn fake_qbittorrent_for_add(
    pool: &sqlx::PgPool,
    existing: serde_json::Value,
) -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/login"))
        .respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "SID=abc; path=/"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(existing))
        .mount(&server)
        .await;
    for endpoint in ["createCategory", "add", "delete"] {
        Mock::given(method("POST"))
            .and(path(format!("/api/v2/torrents/{endpoint}")))
            .respond_with(ResponseTemplate::new(200).set_body_string("Ok."))
            .mount(&server)
            .await;
    }
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

async fn seed_add_library(pool: &sqlx::PgPool) -> Uuid {
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

async fn requests_to(server: &wiremock::MockServer, endpoint: &str) -> usize {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == format!("/api/v2/torrents/{endpoint}"))
        .count()
}

const HASH: &str = "0123456789abcdef0123456789abcdef01234567";

fn add_request(library_id: Uuid) -> QBittorrentAddRequest {
    QBittorrentAddRequest {
        url: format!("magnet:?xt=urn:btih:{HASH}&dn=Amulet"),
        library_id: Some(library_id),
        series_name: Some("Amulet".into()),
        expected_volumes: Some(vec![1]),
        replace_existing: false,
    }
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_magnet_already_in_qbittorrent_under_another_category_is_adopted(pool: sqlx::PgPool) {
    let library_id = seed_add_library(&pool).await;
    let server = fake_qbittorrent_for_add(
        &pool,
        serde_json::json!([{
            "hash": HASH, "category": "manga", "state": "downloading",
            "progress": 0.3, "save_path": "/downloads/manga", "content_path": "/downloads/manga/Amulet"
        }]),
    )
    .await;

    let resp = add_torrent(
        State(crate::state::test_state(pool.clone())),
        Json(add_request(library_id)),
    )
    .await
    .unwrap()
    .0;
    assert!(resp.success, "{}", resp.message);
    let id = resp.torrent_download_id.expect("tracked");

    let (hash, owned): (Option<String>, bool) =
        sqlx::query_as("SELECT qb_hash, qb_owned FROM torrent_downloads WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(hash.as_deref(), Some(HASH));
    assert!(!owned, "a torrent the user added must never be claimed");
    assert_eq!(
        requests_to(&server, "add").await,
        0,
        "the existing torrent is not re-added"
    );
    assert_eq!(requests_to(&server, "delete").await, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn a_new_magnet_is_owned(pool: sqlx::PgPool) {
    let library_id = seed_add_library(&pool).await;
    let server = fake_qbittorrent_for_add(&pool, serde_json::json!([])).await;

    let resp = add_torrent(
        State(crate::state::test_state(pool.clone())),
        Json(add_request(library_id)),
    )
    .await
    .unwrap()
    .0;
    assert!(resp.success, "{}", resp.message);
    let id = resp.torrent_download_id.expect("tracked");

    let (hash, owned): (Option<String>, bool) =
        sqlx::query_as("SELECT qb_hash, qb_owned FROM torrent_downloads WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(hash.as_deref(), Some(HASH));
    assert!(owned);
    assert_eq!(requests_to(&server, "add").await, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn an_unanswered_existence_check_refuses_the_add(pool: sqlx::PgPool) {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    let library_id = seed_add_library(&pool).await;
    let server = wiremock::MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/login"))
        .respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "SID=abc; path=/"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/torrents/createCategory"))
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
    .execute(&pool)
    .await
    .unwrap();

    let resp = add_torrent(
        State(crate::state::test_state(pool.clone())),
        Json(add_request(library_id)),
    )
    .await
    .unwrap()
    .0;
    assert!(!resp.success);
    assert_eq!(requests_to(&server, "add").await, 0);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM torrent_downloads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}
