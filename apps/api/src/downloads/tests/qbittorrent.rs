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
