//! Helpers to read values from the `app_settings` key/value table.
//!
//! Every service (API, indexer, notifications) reads runtime configuration from
//! `app_settings`. These helpers centralize the SQL + JSON decoding so callers
//! only deal with a typed value.

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use sqlx::PgPool;

/// Fetch and deserialize the `app_settings` value stored under `key`.
///
/// Returns `Ok(None)` when the key is absent. JSON that cannot be decoded into
/// `T` is reported as an error (the row exists but is malformed).
pub async fn load_setting<T>(pool: &PgPool, key: &str) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    let value =
        sqlx::query_scalar::<_, serde_json::Value>("SELECT value FROM app_settings WHERE key = $1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .with_context(|| format!("failed to read setting '{key}'"))?;

    match value {
        Some(value) => {
            let parsed = serde_json::from_value(value)
                .with_context(|| format!("invalid JSON for setting '{key}'"))?;
            Ok(Some(parsed))
        }
        None => Ok(None),
    }
}

/// Like [`load_setting`], but returns `default` when the key is missing.
pub async fn load_setting_or_default<T>(pool: &PgPool, key: &str, default: T) -> Result<T>
where
    T: DeserializeOwned,
{
    Ok(load_setting(pool, key).await?.unwrap_or(default))
}

// Integration tests for these helpers live in `apps/api/src/tests/core_settings.rs`.
// They use `#[sqlx::test]`, which reads the process-global `DATABASE_URL` during
// pool setup; keeping them out of this crate avoids racing with the env-mutating
// tests in `config.rs`.
