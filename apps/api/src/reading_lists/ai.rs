//! AI-assisted reading-list suggestions.
//!
//! The model receives the existing lists (with their series) and the series
//! that do not belong to any list yet. It can either propose additions to an
//! existing list or brand-new lists grouping several related series.
//!
//! Nothing is written here: every proposal is returned to the backoffice and
//! only applied after the user validates it.

use std::collections::{HashMap, HashSet};

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

const DEFAULT_LISTS_PROMPT: &str = "You are a librarian organizing a comics and manga collection. Suggest reading lists that group series sharing a strong, meaningful association (same universe, same author, same genre and audience, complementary themes, or a coherent reading order).\n\nAnalyse the existing lists: {{lists}}\n\nAnalyse these series, which are not part of any list yet: {{series}}\n\nReturn at most {{max_lists}} new lists. Only group series when the association is obvious and useful; never group unrelated series just to fill a list. A new list must contain at least 2 series.\n\nAlso propose additions of available series to existing lists when they clearly belong there. Never propose a series that is already in a list.\n\nEvery new list needs a short, specific name and a real description that explains what the series have in common (1 to 2 sentences). Write both in the same language as the existing list names when possible, otherwise in French. Add a short reason for every proposal.\n\nReturn only JSON in the form {\"additions\":[{\"list_id\":\"uuid\",\"reason\":\"...\",\"series_ids\":[\"uuid\"]}],\"new_lists\":[{\"name\":\"...\",\"description\":\"...\",\"reason\":\"...\",\"series_ids\":[\"uuid\",\"uuid\"]}]}. Use only the series ids supplied above.";

const MAX_ADDITIONS: usize = 15;
const MAX_CREATIONS: usize = 8;
const MAX_SERIES_PER_SUGGESTION: usize = 30;
const MAX_LIST_SERIES_IN_PROMPT: usize = 40;
const MAX_LISTS_IN_PROMPT: i64 = 40;
const MAX_LIST_DESCRIPTION_IN_PROMPT: usize = 300;
const PROMPT_MAX_NEW_LISTS: usize = 5;
const DEFAULT_CANDIDATE_LIMIT: i64 = 120;
const MAX_CANDIDATE_LIMIT: i64 = 250;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SuggestReadingListsRequest {
    /// Maximum number of unassigned series sent to the model (clamped to 20-250, default 120).
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AiSuggestedSeries {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    pub cover_url: Option<String>,
    #[schema(value_type = Option<String>)]
    pub first_book_id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub first_book_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub library_name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AiListAdditionSuggestion {
    #[schema(value_type = String)]
    pub list_id: Uuid,
    pub list_name: String,
    pub reason: String,
    pub series: Vec<AiSuggestedSeries>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AiListCreationSuggestion {
    pub name: String,
    pub description: String,
    pub reason: String,
    pub series: Vec<AiSuggestedSeries>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AiReadingListSuggestions {
    pub additions: Vec<AiListAdditionSuggestion>,
    pub new_lists: Vec<AiListCreationSuggestion>,
}

struct CandidateSeries {
    series: AiSuggestedSeries,
    description: Option<String>,
    genres: Vec<String>,
    authors: Vec<String>,
    publishers: Vec<String>,
    start_year: Option<i32>,
    total_volumes: Option<i32>,
    status: Option<String>,
}

struct ExistingList {
    id: Uuid,
    name: String,
    description: Option<String>,
    series_names: Vec<String>,
}

struct SuggestionContext {
    candidates: HashMap<Uuid, AiSuggestedSeries>,
    list_names: HashMap<Uuid, String>,
    /// Every existing list name, not only the capped ones sent to the model, so
    /// the model cannot create a duplicate of an older list.
    existing_names: HashSet<String>,
}

/// Suggest additions to existing lists and new lists to create.
#[utoipa::path(
    post,
    path = "/reading-lists/ai-suggest",
    tag = "reading-lists",
    request_body = SuggestReadingListsRequest,
    responses(
        (status = 200, body = AiReadingListSuggestions),
        (status = 401, description = "Unauthorized"),
        (status = 422, description = "AI suggestions are disabled or not configured"),
    ),
    security(("Bearer" = []))
)]
pub async fn suggest_reading_lists(
    State(state): State<AppState>,
    Json(body): Json<SuggestReadingListsRequest>,
) -> Result<Json<AiReadingListSuggestions>, ApiError> {
    let setting = crate::ai::load_settings(&state.pool).await?;
    let config = crate::ai::provider_config(
        &setting,
        "reading_lists_enabled",
        "AI reading list suggestions are disabled",
        "AI reading list suggestions are not configured",
    )?;

    let limit = body
        .limit
        .unwrap_or(DEFAULT_CANDIDATE_LIMIT)
        .clamp(20, MAX_CANDIDATE_LIMIT);

    let candidates = fetch_candidates(&state, limit).await?;
    if candidates.is_empty() {
        return Ok(Json(AiReadingListSuggestions {
            additions: Vec::new(),
            new_lists: Vec::new(),
        }));
    }

    let existing_lists = fetch_existing_lists(&state).await?;
    let existing_names = fetch_all_list_names(&state).await?;
    let context = SuggestionContext {
        candidates: candidates
            .iter()
            .map(|candidate| (candidate.series.id, candidate.series.clone()))
            .collect(),
        list_names: existing_lists
            .iter()
            .map(|list| (list.id, list.name.clone()))
            .collect(),
        existing_names,
    };

    let lists_input: Vec<Value> = existing_lists
        .iter()
        .map(|list| {
            serde_json::json!({
                "list_id": list.id,
                "name": truncate_chars(&list.name, 120),
                "description": list
                    .description
                    .as_deref()
                    .map(|description| truncate_chars(description, MAX_LIST_DESCRIPTION_IN_PROMPT)),
                "series": list
                    .series_names
                    .iter()
                    .take(MAX_LIST_SERIES_IN_PROMPT)
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let series_input: Vec<Value> = candidates
        .iter()
        .map(|candidate| {
            serde_json::json!({
                "series_id": candidate.series.id,
                "name": candidate.series.name,
                "description": truncate_chars(
                    candidate.description.as_deref().unwrap_or(""),
                    400
                ),
                "genres": candidate.genres,
                "authors": candidate.authors,
                "publishers": candidate.publishers,
                "start_year": candidate.start_year,
                "total_volumes": candidate.total_volumes,
                "status": candidate.status,
                "library": candidate.series.library_name,
            })
        })
        .collect();

    let prompt_template = setting
        .get("reading_list_prompt")
        .and_then(Value::as_str)
        .filter(|prompt| !prompt.trim().is_empty())
        .unwrap_or(DEFAULT_LISTS_PROMPT);
    // Resolve the data placeholders before injecting the data so that a series
    // name containing another placeholder cannot be expanded by accident.
    let lists_json =
        serde_json::to_string(&lists_input).map_err(|e| ApiError::internal(e.to_string()))?;
    let series_json =
        serde_json::to_string(&series_input).map_err(|e| ApiError::internal(e.to_string()))?;
    let prompt = prompt_template
        .replace("{{max_lists}}", &PROMPT_MAX_NEW_LISTS.to_string())
        .replace("{{lists}}", "\u{0}lists\u{0}")
        .replace("{{series}}", "\u{0}series\u{0}")
        .replace("\u{0}lists\u{0}", &lists_json)
        .replace("\u{0}series\u{0}", &series_json);

    let parsed = crate::ai::complete_json(
        &config,
        "You organize comics and manga series into reading lists. Follow the requested JSON schema exactly.",
        &prompt,
    )
    .await?;

    Ok(Json(parse_suggestions(&parsed, &context)))
}

async fn fetch_candidates(state: &AppState, limit: i64) -> Result<Vec<CandidateSeries>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT
            s.id,
            s.name,
            s.description,
            s.cover_url,
            s.genres,
            s.authors,
            s.publishers,
            s.start_year,
            s.total_volumes,
            s.status,
            l.name AS library_name,
            fb.id AS first_book_id,
            fb.updated_at AS first_book_updated_at
        FROM series s
        JOIN libraries l ON l.id = s.library_id
        LEFT JOIN LATERAL (
            SELECT b.id, b.updated_at
            FROM books b
            WHERE b.series_id = s.id
            ORDER BY CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END, b.volume NULLS LAST
            LIMIT 1
        ) fb ON TRUE
        WHERE NOT EXISTS (
            SELECT 1 FROM reading_list_items rli WHERE rli.series_id = s.id
        )
        ORDER BY COALESCE(array_to_string(s.genres, ', '), ''), s.name
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| CandidateSeries {
            series: AiSuggestedSeries {
                id: row.get("id"),
                name: row.get("name"),
                cover_url: row.get("cover_url"),
                first_book_id: row.get("first_book_id"),
                first_book_updated_at: row.get("first_book_updated_at"),
                library_name: row.get("library_name"),
            },
            description: row.get("description"),
            genres: row
                .get::<Option<Vec<String>>, _>("genres")
                .unwrap_or_default(),
            authors: row
                .get::<Option<Vec<String>>, _>("authors")
                .unwrap_or_default(),
            publishers: row
                .get::<Option<Vec<String>>, _>("publishers")
                .unwrap_or_default(),
            start_year: row.get("start_year"),
            total_volumes: row.get("total_volumes"),
            status: row.get("status"),
        })
        .collect())
}

async fn fetch_existing_lists(state: &AppState) -> Result<Vec<ExistingList>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT
            rl.id,
            rl.name,
            rl.description,
            COALESCE(
                array_agg(s.name ORDER BY rli.position) FILTER (WHERE s.id IS NOT NULL),
                '{}'
            ) AS series_names
        FROM reading_lists rl
        LEFT JOIN reading_list_items rli ON rli.list_id = rl.id
        LEFT JOIN series s ON s.id = rli.series_id
        GROUP BY rl.id
        -- Only the most recently edited lists are sent to the model: this keeps
        -- the prompt bounded for large libraries and favours lists still in use.
        ORDER BY rl.updated_at DESC, rl.name
        LIMIT $1
        "#,
    )
    .bind(MAX_LISTS_IN_PROMPT)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ExistingList {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            series_names: row.get("series_names"),
        })
        .collect())
}

/// All reading-list names, used to reject a proposal that would recreate an
/// existing list even when that list is outside the prompt cap.
async fn fetch_all_list_names(state: &AppState) -> Result<HashSet<String>, ApiError> {
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM reading_lists")
        .fetch_all(&state.pool)
        .await?;

    Ok(names
        .into_iter()
        .map(|name| name.trim().to_lowercase())
        .collect())
}

/// Validate the model payload against the data we actually sent it.
///
/// Unknown ids, already-suggested series and malformed new lists are dropped so
/// the UI can trust every proposal it receives.
fn parse_suggestions(payload: &Value, context: &SuggestionContext) -> AiReadingListSuggestions {
    let mut used_series: HashSet<Uuid> = HashSet::new();
    let mut used_names: HashSet<String> = context.existing_names.clone();

    let mut additions = Vec::new();
    let addition_items = payload
        .get("additions")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for item in addition_items.iter().take(MAX_ADDITIONS) {
        let Some(list_id) = item
            .get("list_id")
            .and_then(Value::as_str)
            .and_then(|id| id.parse::<Uuid>().ok())
        else {
            continue;
        };
        let Some(list_name) = context.list_names.get(&list_id) else {
            continue;
        };
        let reason = truncate_chars(&text_field(item, "reason"), 300);
        if reason.is_empty() {
            continue;
        }

        let series = take_available_series(item, context, &used_series);
        if series.is_empty() {
            continue;
        }
        used_series.extend(series.iter().map(|s| s.id));

        additions.push(AiListAdditionSuggestion {
            list_id,
            list_name: list_name.clone(),
            reason,
            series,
        });
    }

    let mut new_lists = Vec::new();
    let creation_items = payload
        .get("new_lists")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for item in creation_items.iter().take(MAX_CREATIONS) {
        let name = truncate_chars(&text_field(item, "name"), 120);
        let description = truncate_chars(&text_field(item, "description"), 500);
        let reason = truncate_chars(&text_field(item, "reason"), 300);
        if name.is_empty() || description.is_empty() || reason.is_empty() {
            continue;
        }
        let normalized_name = name.to_lowercase();
        if used_names.contains(&normalized_name) {
            continue;
        }

        let series = take_available_series(item, context, &used_series);
        if series.len() < 2 {
            continue;
        }
        used_names.insert(normalized_name);
        used_series.extend(series.iter().map(|s| s.id));

        new_lists.push(AiListCreationSuggestion {
            name,
            description,
            reason,
            series,
        });
    }

    AiReadingListSuggestions {
        additions,
        new_lists,
    }
}

fn take_available_series(
    item: &Value,
    context: &SuggestionContext,
    used: &HashSet<Uuid>,
) -> Vec<AiSuggestedSeries> {
    let mut seen = HashSet::new();
    item.get("series_ids")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|id| id.parse::<Uuid>().ok())
        .filter(|id| !used.contains(id))
        .filter_map(|id| context.candidates.get(&id))
        .filter(|series| seen.insert(series.id))
        .take(MAX_SERIES_PER_SUGGESTION)
        .cloned()
        .collect()
}

fn text_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

fn truncate_chars(value: &str, max: usize) -> String {
    value
        .chars()
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
#[path = "../tests/reading_lists_ai.rs"]
mod tests;
