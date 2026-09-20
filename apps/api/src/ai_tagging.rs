use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

const DEFAULT_TAGGING_PROMPT: &str = "Suggest up to {{max_tags}} relevant genres for each comic or manga series. Usually return only 1 to 3 genres; do not fill the limit when fewer genres fit.\n\nYou MUST choose tags only from this existing genre list, preserving the exact spelling and casing: {{genres}}. Never create or paraphrase a genre.\n\nUse the supplied metadata to disambiguate the series. For a recognizable, well-known title, you may use reliable general knowledge even when its metadata is sparse. Do not make uncertain associations: omit a series if you cannot identify it with confidence. Do not select a format, medium, age category, or generic label merely because the series is a comic (for example 'BD' or 'Books/Comics'). Select those labels only if they are genuinely relevant genres for the series. Return only JSON in the form {\"suggestions\":[{\"series_id\":\"uuid\",\"tags\":[\"existing genre\"]}]}.\n\nSeries: {{series}}";

#[derive(Debug, Deserialize, ToSchema)]
pub struct SuggestTagsRequest {
    #[schema(value_type = Vec<String>)]
    pub series_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SuggestedTags {
    #[schema(value_type = String)]
    pub series_id: Uuid,
    pub name: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SuggestTagsResponse {
    pub suggestions: Vec<SuggestedTags>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TestConnectionRequest {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TestConnectionResponse {
    pub ok: bool,
    pub message: String,
}

pub async fn test_connection(
    Json(body): Json<TestConnectionRequest>,
) -> Result<Json<TestConnectionResponse>, ApiError> {
    let api_key = body.api_key.trim();
    if api_key.is_empty() {
        return Err(ApiError::bad_request("API key is required"));
    }
    let base_url = body.base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return Err(ApiError::bad_request("API URL is required"));
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let response = client
        .get(format!("{base_url}/models"))
        .bearer_auth(api_key)
        .send()
        .await?;
    let status = response.status();
    let payload: Value = response.json().await.unwrap_or_default();
    if !status.is_success() {
        let message = payload
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("AI provider rejected the connection");
        return Err(ApiError::bad_request(message));
    }
    let model_available = payload
        .get("data")
        .and_then(Value::as_array)
        .map(|models| {
            models
                .iter()
                .any(|item| item.get("id").and_then(Value::as_str) == Some(body.model.trim()))
        })
        .unwrap_or(false);
    let message = if model_available {
        format!(
            "Connection successful; model '{}' is available",
            body.model.trim()
        )
    } else {
        format!(
            "Connection successful; model '{}' was not found in the provider model list",
            body.model.trim()
        )
    };
    Ok(Json(TestConnectionResponse { ok: true, message }))
}

pub async fn suggest_tags(
    State(state): State<AppState>,
    Json(body): Json<SuggestTagsRequest>,
) -> Result<Json<SuggestTagsResponse>, ApiError> {
    if body.series_ids.is_empty() {
        return Err(ApiError::bad_request("at least one series is required"));
    }
    if body.series_ids.len() > 25 {
        return Err(ApiError::bad_request(
            "at most 25 series can be analyzed at once",
        ));
    }

    let setting = sqlx::query("SELECT value FROM app_settings WHERE key = 'ai_tagging'")
        .fetch_optional(&state.pool)
        .await?
        .map(|row| row.get::<Value, _>("value"))
        .unwrap_or_else(|| serde_json::json!({}));
    if !setting
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(ApiError::unprocessable_entity("AI tagging is disabled"));
    }
    let api_key = setting
        .get("api_key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if api_key.is_empty() {
        return Err(ApiError::unprocessable_entity(
            "AI tagging is not configured",
        ));
    }

    let base_url = setting
        .get("base_url")
        .and_then(Value::as_str)
        .unwrap_or("https://openrouter.ai/api/v1")
        .trim_end_matches('/');
    let model = setting
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("openrouter/free");
    let max_tags = setting
        .get("max_tags")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .clamp(1, 10);

    let rows = sqlx::query(
        "SELECT id, name, description, authors, publishers, start_year, total_volumes, status
         FROM series WHERE id = ANY($1)",
    )
    .bind(&body.series_ids)
    .fetch_all(&state.pool)
    .await?;

    let existing_genres: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT genre FROM (SELECT unnest(genres) AS genre FROM series) genres
         WHERE genre IS NOT NULL AND btrim(genre) <> '' ORDER BY genre",
    )
    .fetch_all(&state.pool)
    .await?;
    if existing_genres.is_empty() {
        return Err(ApiError::unprocessable_entity(
            "no existing genres are available",
        ));
    }

    let input: Vec<Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "series_id": row.get::<Uuid, _>("id"),
                "name": row.get::<String, _>("name"),
                "description": row.get::<Option<String>, _>("description"),
                "authors": row.get::<Vec<String>, _>("authors"),
                "publishers": row.get::<Vec<String>, _>("publishers"),
                "start_year": row.get::<Option<i32>, _>("start_year"),
                "total_volumes": row.get::<Option<i32>, _>("total_volumes"),
                "status": row.get::<Option<String>, _>("status"),
            })
        })
        .collect();

    let prompt_template = setting
        .get("prompt")
        .and_then(Value::as_str)
        .filter(|prompt| !prompt.trim().is_empty())
        .unwrap_or(DEFAULT_TAGGING_PROMPT);
    let prompt = prompt_template
        .replace("{{max_tags}}", &max_tags.to_string())
        .replace(
            "{{genres}}",
            &serde_json::to_string(&existing_genres)
                .map_err(|e| ApiError::internal(e.to_string()))?,
        )
        .replace(
            "{{series}}",
            &serde_json::to_string(&input).map_err(|e| ApiError::internal(e.to_string()))?,
        );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()?;
    let response = client
        .post(format!("{base_url}/chat/completions"))
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://stripstream.app")
        .json(&serde_json::json!({
            "model": model,
            "temperature": 0,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": "You classify series genres. Follow the requested JSON schema exactly." },
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
    let parsed: Value = serde_json::from_str(json_text)
        .map_err(|e| ApiError::internal(format!("AI returned invalid JSON: {e}")))?;
    let by_id = parsed
        .get("suggestions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let names: std::collections::HashMap<Uuid, String> = rows
        .into_iter()
        .map(|row| (row.get("id"), row.get("name")))
        .collect();
    let canonical_genres: std::collections::HashMap<String, String> = existing_genres
        .into_iter()
        .map(|genre| (genre.to_lowercase(), genre))
        .collect();
    let suggestions = by_id
        .into_iter()
        .filter_map(|item| {
            let id = item.get("series_id")?.as_str()?.parse().ok()?;
            let name = names.get(&id)?.clone();
            let tags = item
                .get("tags")?
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter_map(|tag| canonical_genres.get(&tag.to_lowercase()).cloned())
                .collect::<Vec<_>>()
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .take(max_tags as usize)
                .collect();
            Some(SuggestedTags {
                series_id: id,
                name,
                tags,
            })
        })
        .collect();

    Ok(Json(SuggestTagsResponse { suggestions }))
}

#[cfg(test)]
#[path = "tests/ai_tagging.rs"]
mod tests;
