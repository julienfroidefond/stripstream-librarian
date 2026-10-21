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

const DEFAULT_LISTS_PROMPT: &str = "You are a librarian organizing a comics and manga collection. Suggest reading lists that group series sharing a strong, meaningful association (same universe, same author, same genre and audience, complementary themes, or a coherent reading order).\n\nAnalyse the existing lists: {{lists}}\n\nAnalyse these series: {{series}}\n\nSome series already belong to a list: they carry a non-null current_list_id (and current_list). Only propose an already-listed series as a move to another existing list, and only when its current list is clearly wrong for it. Never propose an already-listed series as an addition or in a new list.\n\nReturn at most {{max_lists}} new lists. Only group series when the association is obvious and useful; never group unrelated series just to fill a list. A new list must contain at least 2 series.\n\nAlso propose additions of series that are not in any list (current_list_id is null) to existing lists when they clearly belong there.\n\nEvery new list needs a short, specific name and a real description that explains what the series have in common (1 to 2 sentences). Write both in the same language as the existing list names when possible, otherwise in French. Add a short reason for every proposal.\n\nReturn only JSON in the form {\"additions\":[{\"list_id\":\"uuid\",\"reason\":\"...\",\"series_ids\":[\"uuid\"]}],\"moves\":[{\"from_list_id\":\"uuid\",\"to_list_id\":\"uuid\",\"reason\":\"...\",\"series_ids\":[\"uuid\"]}],\"new_lists\":[{\"name\":\"...\",\"description\":\"...\",\"reason\":\"...\",\"series_ids\":[\"uuid\",\"uuid\"]}]}. Use only the series ids supplied above.";

const MAX_ADDITIONS: usize = 15;
const MAX_MOVES: usize = 10;
const MAX_CREATIONS: usize = 8;
const MAX_SERIES_PER_SUGGESTION: usize = 30;
const MAX_LIST_SERIES_IN_PROMPT: usize = 40;
const MAX_LISTS_IN_PROMPT: i64 = 40;
const MAX_LIST_DESCRIPTION_IN_PROMPT: usize = 300;
const PROMPT_MAX_NEW_LISTS: usize = 5;
const DEFAULT_CANDIDATE_LIMIT: i64 = 120;
const MAX_CANDIDATE_LIMIT: i64 = 250;
/// Cap on the offset so a crafted request cannot make PostgreSQL walk the whole
/// library to skip rows.
const MAX_CANDIDATE_OFFSET: i64 = 10_000;
/// Cap on series already in a list that are offered to the model for moves.
const MAX_MOVE_CANDIDATES: i64 = 120;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SuggestReadingListsRequest {
    /// Maximum number of unassigned series sent to the model (clamped to 20-250, default 120).
    pub limit: Option<i64>,
    /// Number of series to skip in the deterministic candidate window (default 0).
    ///
    /// Combined with `limit`, this lets the caller walk the whole library in
    /// several calls instead of always seeing the same head of the window.
    pub offset: Option<i64>,
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
    /// Reading list the series currently belongs to (`None` when unlisted).
    #[schema(value_type = Option<String>)]
    pub current_list_id: Option<Uuid>,
    pub current_list_name: Option<String>,
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
pub struct AiListMoveSuggestion {
    #[schema(value_type = String)]
    pub from_list_id: Uuid,
    pub from_list_name: String,
    #[schema(value_type = String)]
    pub to_list_id: Uuid,
    pub to_list_name: String,
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
    pub moves: Vec<AiListMoveSuggestion>,
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

/// Series offered to the model, split between the two kinds of proposals.
struct CandidateWindow {
    /// Series that belong to no list: candidates for additions and new lists.
    unlisted: Vec<CandidateSeries>,
    /// Series that already belong to a list: candidates for moves only.
    listed: Vec<CandidateSeries>,
}

struct SuggestionContext {
    candidates: HashMap<Uuid, AiSuggestedSeries>,
    /// Series already in a list → list id, as stored in the database. Used to
    /// validate moves instead of trusting the model payload.
    memberships: HashMap<Uuid, Uuid>,
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
    let offset = body.offset.unwrap_or(0).clamp(0, MAX_CANDIDATE_OFFSET);

    let window = fetch_candidates(&state, limit, offset).await?;
    let list_names = fetch_all_list_names(&state).await?;
    // Moves need a destination list, so a single-list library has nothing to
    // offer once every series is already listed.
    let can_move = !window.listed.is_empty() && list_names.len() >= 2;
    if window.unlisted.is_empty() && !can_move {
        return Ok(Json(AiReadingListSuggestions {
            additions: Vec::new(),
            moves: Vec::new(),
            new_lists: Vec::new(),
        }));
    }

    let existing_lists = fetch_existing_lists(&state).await?;
    let existing_names = list_names
        .values()
        .map(|name| name.trim().to_lowercase())
        .collect();
    let candidates: Vec<&CandidateSeries> =
        window.unlisted.iter().chain(window.listed.iter()).collect();
    let context = SuggestionContext {
        candidates: candidates
            .iter()
            .map(|candidate| (candidate.series.id, candidate.series.clone()))
            .collect(),
        memberships: window
            .listed
            .iter()
            .filter_map(|candidate| {
                candidate
                    .series
                    .current_list_id
                    .map(|list_id| (candidate.series.id, list_id))
            })
            .collect(),
        list_names,
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
                "current_list_id": candidate.series.current_list_id,
                "current_list": candidate.series.current_list_name,
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

async fn fetch_candidates(
    state: &AppState,
    limit: i64,
    offset: i64,
) -> Result<CandidateWindow, ApiError> {
    Ok(CandidateWindow {
        unlisted: fetch_unlisted_candidates(state, limit, offset).await?,
        listed: fetch_listed_candidates(state, MAX_MOVE_CANDIDATES, offset).await?,
    })
}

/// Series that belong to no list, sampled deterministically and fairly.
///
/// Candidates are bucketed by `(library, first genre)` and read round-robin:
/// every bucket contributes its first series before any bucket contributes its
/// second one. A large homogeneous bucket (often one genre in one library) can
/// therefore not crowd out the rest of the library, and the same request always
/// returns the same window.
async fn fetch_unlisted_candidates(
    state: &AppState,
    limit: i64,
    offset: i64,
) -> Result<Vec<CandidateSeries>, ApiError> {
    let rows = sqlx::query(
        r#"
        WITH unlisted AS (
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
                s.library_id,
                l.name AS library_name,
                fb.id AS first_book_id,
                fb.updated_at AS first_book_updated_at,
                COALESCE(s.genres[1], '') AS genre_bucket
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
        ),
        ranked AS (
            SELECT
                *,
                ROW_NUMBER() OVER (
                    PARTITION BY library_id, genre_bucket
                    ORDER BY name, id
                ) AS bucket_rank
            FROM unlisted
        )
        SELECT
            id,
            name,
            description,
            cover_url,
            genres,
            authors,
            publishers,
            start_year,
            total_volumes,
            status,
            library_name,
            first_book_id,
            first_book_updated_at,
            NULL::uuid AS current_list_id,
            NULL::text AS current_list_name
        FROM ranked
        ORDER BY bucket_rank, library_id, genre_bucket, name, id
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows.iter().map(candidate_from_row).collect())
}

/// Series that already belong to a list, with the list they are in.
///
/// They are only offered for moves: the model must never add them to another
/// list without removing them from the current one. They are read round-robin
/// per list so a single large list cannot fill the whole move window.
async fn fetch_listed_candidates(
    state: &AppState,
    limit: i64,
    offset: i64,
) -> Result<Vec<CandidateSeries>, ApiError> {
    let rows = sqlx::query(
        r#"
        WITH listed AS (
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
                fb.updated_at AS first_book_updated_at,
                membership.list_id AS current_list_id,
                membership.list_name AS current_list_name
            FROM series s
            JOIN libraries l ON l.id = s.library_id
            JOIN LATERAL (
                SELECT rli.list_id, rl.name AS list_name
                FROM reading_list_items rli
                JOIN reading_lists rl ON rl.id = rli.list_id
                WHERE rli.series_id = s.id
                ORDER BY rl.updated_at DESC, rl.id
                LIMIT 1
            ) membership ON TRUE
            LEFT JOIN LATERAL (
                SELECT b.id, b.updated_at
                FROM books b
                WHERE b.series_id = s.id
                ORDER BY CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END, b.volume NULLS LAST
                LIMIT 1
            ) fb ON TRUE
        ),
        ranked AS (
            SELECT
                *,
                ROW_NUMBER() OVER (
                    PARTITION BY current_list_id
                    ORDER BY name, id
                ) AS bucket_rank
            FROM listed
        )
        SELECT
            id,
            name,
            description,
            cover_url,
            genres,
            authors,
            publishers,
            start_year,
            total_volumes,
            status,
            library_name,
            first_book_id,
            first_book_updated_at,
            current_list_id,
            current_list_name
        FROM ranked
        ORDER BY bucket_rank, current_list_name, current_list_id, name, id
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows.iter().map(candidate_from_row).collect())
}

fn candidate_from_row(row: &sqlx::postgres::PgRow) -> CandidateSeries {
    CandidateSeries {
        series: AiSuggestedSeries {
            id: row.get("id"),
            name: row.get("name"),
            cover_url: row.get("cover_url"),
            first_book_id: row.get("first_book_id"),
            first_book_updated_at: row.get("first_book_updated_at"),
            library_name: row.get("library_name"),
            current_list_id: row.get("current_list_id"),
            current_list_name: row.get("current_list_name"),
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
    }
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

/// All reading-list names, keyed by id, used to validate and name moves and to
/// reject a proposal that would recreate an existing list even when that list is
/// outside the prompt cap.
async fn fetch_all_list_names(state: &AppState) -> Result<HashMap<Uuid, String>, ApiError> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, name FROM reading_lists")
        .fetch_all(&state.pool)
        .await?;

    Ok(rows.into_iter().collect())
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

    let mut moves = Vec::new();
    let move_items = payload
        .get("moves")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for item in move_items.iter().take(MAX_MOVES) {
        let (Some(from_list_id), Some(to_list_id)) = (
            uuid_field(item, "from_list_id"),
            uuid_field(item, "to_list_id"),
        ) else {
            continue;
        };
        if from_list_id == to_list_id {
            continue;
        }
        let (Some(from_list_name), Some(to_list_name)) = (
            context.list_names.get(&from_list_id),
            context.list_names.get(&to_list_id),
        ) else {
            continue;
        };
        let reason = truncate_chars(&text_field(item, "reason"), 300);
        if reason.is_empty() {
            continue;
        }

        let series = take_movable_series(item, context, from_list_id, &used_series);
        if series.is_empty() {
            continue;
        }
        used_series.extend(series.iter().map(|s| s.id));

        moves.push(AiListMoveSuggestion {
            from_list_id,
            from_list_name: from_list_name.clone(),
            to_list_id,
            to_list_name: to_list_name.clone(),
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
        moves,
        new_lists,
    }
}

/// Unlisted series from a proposal, ignoring anything already used elsewhere.
fn take_available_series(
    item: &Value,
    context: &SuggestionContext,
    used: &HashSet<Uuid>,
) -> Vec<AiSuggestedSeries> {
    take_series(item, used, |id| {
        context
            .candidates
            .get(&id)
            .filter(|series| series.current_list_id.is_none())
    })
}

/// Series of a move: they must actually belong to the source list according to
/// the database, never to whatever the model claimed.
fn take_movable_series(
    item: &Value,
    context: &SuggestionContext,
    from_list_id: Uuid,
    used: &HashSet<Uuid>,
) -> Vec<AiSuggestedSeries> {
    take_series(item, used, |id| {
        context
            .memberships
            .get(&id)
            .filter(|list_id| **list_id == from_list_id)
            .and_then(|_| context.candidates.get(&id))
    })
}

fn take_series<'a>(
    item: &Value,
    used: &HashSet<Uuid>,
    resolve: impl Fn(Uuid) -> Option<&'a AiSuggestedSeries>,
) -> Vec<AiSuggestedSeries> {
    let mut seen = HashSet::new();
    series_ids(item)
        .filter(|id| !used.contains(id))
        .filter_map(resolve)
        .filter(|series| seen.insert(series.id))
        .take(MAX_SERIES_PER_SUGGESTION)
        .cloned()
        .collect()
}

fn series_ids(item: &Value) -> impl Iterator<Item = Uuid> + '_ {
    item.get("series_ids")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|id| id.parse::<Uuid>().ok())
}

fn uuid_field(value: &Value, key: &str) -> Option<Uuid> {
    value.get(key).and_then(Value::as_str)?.parse().ok()
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
