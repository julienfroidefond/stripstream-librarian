//! Shared helpers for the OpenAI-compatible AI integrations.
//!
//! Genre tagging and reading-list suggestions both read the same
//! `ai_tagging` setting (provider endpoint, API key, model) and talk to a
//! `chat/completions` endpoint. This module centralises the setting loading,
//! the enablement gate, the request and the JSON extraction so the two
//! features cannot drift apart.

use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;
use stripstream_core::http::build_http_client;
use stripstream_core::settings::load_setting;

use crate::error::ApiError;

/// Settings key shared by every AI feature.
pub const SETTINGS_KEY: &str = "ai_tagging";
pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const DEFAULT_MODEL: &str = "openrouter/free";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Resolved provider configuration (never contains an empty API key).
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// Load the raw `ai_tagging` settings object.
pub async fn load_settings(pool: &PgPool) -> Result<Value, ApiError> {
    Ok(load_setting::<Value>(pool, SETTINGS_KEY)
        .await?
        .unwrap_or_else(|| serde_json::json!({})))
}

/// Validate the enablement flag and credentials for one AI feature.
///
/// `enabled_key` lets a feature opt in independently while sharing the provider
/// configuration (for example `enabled` for genre tagging and
/// `reading_lists_enabled` for reading-list suggestions).
pub fn provider_config(
    setting: &Value,
    enabled_key: &str,
    disabled_message: &str,
    not_configured_message: &str,
) -> Result<ProviderConfig, ApiError> {
    if !setting
        .get(enabled_key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(ApiError::unprocessable_entity(disabled_message));
    }

    let api_key = setting
        .get("api_key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if api_key.is_empty() {
        return Err(ApiError::unprocessable_entity(not_configured_message));
    }

    let base_url = setting
        .get("base_url")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_BASE_URL)
        .trim()
        .trim_end_matches('/')
        .to_string();
    if base_url.is_empty() {
        return Err(ApiError::unprocessable_entity(not_configured_message));
    }

    let model = setting
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_MODEL)
        .trim()
        .to_string();

    Ok(ProviderConfig {
        base_url,
        api_key: api_key.to_string(),
        model: if model.is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            model
        },
    })
}

/// Send a JSON-mode chat completion and return the parsed JSON content.
///
/// Models occasionally wrap their JSON in a Markdown fence even when a JSON
/// response format is requested, so the fences are stripped before parsing.
pub async fn complete_json(
    config: &ProviderConfig,
    system_prompt: &str,
    prompt: &str,
) -> Result<Value, ApiError> {
    let client = build_http_client(REQUEST_TIMEOUT)?;
    let response = client
        .post(format!("{}/chat/completions", config.base_url))
        .bearer_auth(&config.api_key)
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://stripstream.app")
        .json(&serde_json::json!({
            "model": config.model,
            "temperature": 0,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": prompt }
            ]
        }))
        .send()
        .await?;

    let status = response.status();
    let payload: Value = response
        .json()
        .await
        .map_err(|e| ApiError::internal(format!("invalid AI response: {e}")))?;
    if !status.is_success() {
        let message = payload
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("AI provider request failed");
        return Err(ApiError::bad_request(message));
    }

    let content = payload
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::internal("AI provider returned no content"))?;
    let json_text = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str(json_text)
        .map_err(|e| ApiError::internal(format!("AI returned invalid JSON: {e}")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn provider_config_requires_the_feature_flag() {
        let setting = json!({ "enabled": true, "reading_lists_enabled": false, "api_key": "sk" });

        let err = provider_config(
            &setting,
            "reading_lists_enabled",
            "disabled",
            "unconfigured",
        )
        .unwrap_err();

        assert_eq!(err.message, "disabled");
    }

    #[test]
    fn provider_config_trims_credentials_and_url() {
        let setting = json!({
            "enabled": true,
            "base_url": " https://example.test/v1/ ",
            "api_key": " sk-test ",
            "model": " test-model ",
        });

        let config = provider_config(&setting, "enabled", "disabled", "unconfigured").unwrap();

        assert_eq!(config.base_url, "https://example.test/v1");
        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.model, "test-model");
    }

    #[test]
    fn provider_config_falls_back_to_defaults() {
        let setting = json!({ "enabled": true, "api_key": "sk", "base_url": "", "model": "" });

        let err = provider_config(&setting, "enabled", "disabled", "unconfigured").unwrap_err();
        assert_eq!(err.message, "unconfigured");

        let setting = json!({ "enabled": true, "api_key": "sk" });
        let config = provider_config(&setting, "enabled", "disabled", "unconfigured").unwrap();
        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.model, DEFAULT_MODEL);
    }
}
