//! Integration tests for the shared `app_settings` helpers
//! ([`stripstream_core::settings::load_setting`] / `load_setting_or_default`).
//!
//! They live in the API crate (which is where `#[sqlx::test]` already runs) rather
//! than in `crates/core`, because the latter also hosts tests that mutate the
//! process-global `DATABASE_URL`, which would race with the pool setup performed
//! by `#[sqlx::test]`.

use stripstream_core::settings::{load_setting, load_setting_or_default};

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_key_returns_none(pool: sqlx::PgPool) {
    let value: Option<String> = load_setting(&pool, "absent").await.unwrap();
    assert!(value.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn existing_key_is_deserialized(pool: sqlx::PgPool) {
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ($1, $2)")
        .bind("core_test_limits")
        .bind(serde_json::json!({ "concurrent_renders": 4 }))
        .execute(&pool)
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    struct Limits {
        concurrent_renders: u32,
    }

    let limits: Limits = load_setting(&pool, "core_test_limits")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(limits.concurrent_renders, 4);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn or_default_falls_back_when_missing(pool: sqlx::PgPool) {
    let value: String = load_setting_or_default(&pool, "absent", "fallback".to_string())
        .await
        .unwrap();
    assert_eq!(value, "fallback");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn malformed_json_is_an_error(pool: sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('core_test_bad', '\"not-an-object\"'::jsonb)",
    )
    .execute(&pool)
    .await
    .unwrap();

    #[derive(serde::Deserialize)]
    struct Obj {
        #[allow(dead_code)]
        x: u32,
    }

    assert!(load_setting::<Obj>(&pool, "core_test_bad").await.is_err());
}
