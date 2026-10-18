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
