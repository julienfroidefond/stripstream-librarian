//! Tests for AI reading-list suggestions.
//!
//! Network-free paths only: gate validation, candidate selection and payload
//! sanitising. The actual provider call is never exercised here.

use std::collections::HashMap;

use axum::extract::State;
use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::*;
use crate::error::ApiError;
use crate::state::test_state;

fn suggestions_body(limit: Option<i64>) -> Json<SuggestReadingListsRequest> {
    Json(SuggestReadingListsRequest { limit })
}

fn expect_err<T>(result: Result<T, ApiError>) -> ApiError {
    match result {
        Ok(_) => panic!("expected an error result"),
        Err(err) => err,
    }
}

async fn set_ai_setting(pool: &PgPool, value: Value) {
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('ai_tagging', $1) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(value)
    .execute(pool)
    .await
    .unwrap();
}

async fn create_library(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO libraries (id, name, root_path) \
         VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(name)
    .bind(format!("/libraries/{}-{}", name, Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_series(pool: &PgPool, library_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name) \
         VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_list(pool: &PgPool, name: &str, series_ids: &[Uuid]) -> Uuid {
    let list_id: Uuid = sqlx::query_scalar(
        "INSERT INTO reading_lists (id, name) VALUES (gen_random_uuid(), $1) RETURNING id",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap();

    for (position, series_id) in series_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO reading_list_items (list_id, series_id, position) VALUES ($1, $2, $3)",
        )
        .bind(list_id)
        .bind(series_id)
        .bind(position as i32)
        .execute(pool)
        .await
        .unwrap();
    }

    list_id
}

fn suggested(id: Uuid, name: &str) -> AiSuggestedSeries {
    AiSuggestedSeries {
        id,
        name: name.to_string(),
        cover_url: None,
        first_book_id: None,
        first_book_updated_at: None,
        library_name: "main".to_string(),
        current_list_id: None,
        current_list_name: None,
    }
}

fn context(series: &[(Uuid, &str)], lists: &[(Uuid, &str)]) -> SuggestionContext {
    SuggestionContext {
        candidates: series
            .iter()
            .map(|(id, name)| (*id, suggested(*id, name)))
            .collect::<HashMap<_, _>>(),
        memberships: HashMap::new(),
        list_names: lists
            .iter()
            .map(|(id, name)| (*id, name.to_string()))
            .collect(),
        existing_names: lists
            .iter()
            .map(|(_, name)| name.trim().to_lowercase())
            .collect(),
    }
}

/// Context where `series_ids` belong to the list `list_id`, mirroring what
/// `fetch_listed_candidates` returns.
fn context_with_memberships(
    series: &[(Uuid, &str)],
    lists: &[(Uuid, &str)],
    memberships: &[(Uuid, Uuid)],
) -> SuggestionContext {
    let mut context = context(series, lists);
    for (series_id, list_id) in memberships {
        context.memberships.insert(*series_id, *list_id);
        if let Some(series) = context.candidates.get_mut(series_id) {
            series.current_list_id = Some(*list_id);
            series.current_list_name = context.list_names.get(list_id).cloned();
        }
    }
    context
}

// ---------------------------------------------------------------------------
// Configuration gate
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggestions_disabled_when_setting_missing(pool: PgPool) {
    let state = test_state(pool);

    let err = expect_err(suggest_reading_lists(State(state), suggestions_body(None)).await);

    assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.message, "AI reading list suggestions are disabled");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggestions_disabled_without_their_own_flag(pool: PgPool) {
    let state = test_state(pool.clone());
    set_ai_setting(&pool, json!({ "enabled": true, "api_key": "sk-test" })).await;

    let err = expect_err(suggest_reading_lists(State(state), suggestions_body(None)).await);

    assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.message, "AI reading list suggestions are disabled");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggestions_not_configured_when_api_key_blank(pool: PgPool) {
    let state = test_state(pool.clone());
    set_ai_setting(
        &pool,
        json!({ "reading_lists_enabled": true, "api_key": "   " }),
    )
    .await;

    let err = expect_err(suggest_reading_lists(State(state), suggestions_body(None)).await);

    assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        err.message,
        "AI reading list suggestions are not configured"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggestions_are_empty_when_nothing_can_be_suggested(pool: PgPool) {
    let state = test_state(pool.clone());
    set_ai_setting(
        &pool,
        json!({
            "reading_lists_enabled": true,
            // Unreachable on purpose: no candidate means no HTTP request.
            "base_url": "http://127.0.0.1:1",
            "api_key": "sk-test",
        }),
    )
    .await;

    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "Already listed").await;
    // Every series is listed and there is only one list: no addition and no
    // move is possible, so the model must not be called.
    create_list(&pool, "Favourites", &[series]).await;

    let Json(suggestions) = suggest_reading_lists(State(state), suggestions_body(None))
        .await
        .unwrap();

    assert!(suggestions.additions.is_empty());
    assert!(suggestions.moves.is_empty());
    assert!(suggestions.new_lists.is_empty());
}

// ---------------------------------------------------------------------------
// Candidate selection
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn candidates_exclude_series_that_belong_to_a_list_from_the_unlisted_window(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let free = create_series(&pool, library, "Free").await;
    let listed = create_series(&pool, library, "Listed").await;
    let list_id = create_list(&pool, "Favourites", &[listed]).await;

    let window = fetch_candidates(&state, 50).await.unwrap();

    let unlisted: Vec<Uuid> = window.unlisted.iter().map(|c| c.series.id).collect();
    assert_eq!(unlisted, vec![free]);

    // Listed series stay available for moves, carrying their current list.
    assert_eq!(window.listed.len(), 1);
    assert_eq!(window.listed[0].series.id, listed);
    assert_eq!(window.listed[0].series.current_list_id, Some(list_id));
    assert_eq!(
        window.listed[0].series.current_list_name.as_deref(),
        Some("Favourites")
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn candidates_are_capped_by_the_requested_limit(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    for index in 0..5 {
        create_series(&pool, library, &format!("Series {index}")).await;
    }

    let window = fetch_candidates(&state, 3).await.unwrap();

    assert_eq!(window.unlisted.len(), 3);
    assert!(window.listed.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn listed_candidates_are_capped_for_the_prompt(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series_ids: Vec<Uuid> = {
        let mut ids = Vec::new();
        for index in 0..(MAX_MOVE_CANDIDATES + 3) {
            ids.push(create_series(&pool, library, &format!("Series {index}")).await);
        }
        ids
    };
    create_list(&pool, "All", &series_ids).await;

    let window = fetch_candidates(&state, 50).await.unwrap();

    assert_eq!(window.unlisted.len(), 0);
    assert_eq!(window.listed.len(), MAX_MOVE_CANDIDATES as usize);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn existing_lists_are_capped_for_the_prompt(pool: PgPool) {
    let state = test_state(pool.clone());
    for index in 0..(MAX_LISTS_IN_PROMPT + 5) {
        create_list(&pool, &format!("List {index}"), &[]).await;
    }

    let lists = fetch_existing_lists(&state).await.unwrap();
    let names = fetch_all_list_names(&state).await.unwrap();

    assert_eq!(lists.len(), MAX_LISTS_IN_PROMPT as usize);
    assert_eq!(names.len(), (MAX_LISTS_IN_PROMPT + 5) as usize);
    assert!(names.values().any(|name| name == "List 0"));
}

// ---------------------------------------------------------------------------
// Payload sanitising
// ---------------------------------------------------------------------------

#[test]
fn parse_keeps_valid_additions_and_new_lists_once() {
    let list_id = Uuid::new_v4();
    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let s3 = Uuid::new_v4();
    let s4 = Uuid::new_v4();
    let ctx = context(
        &[(s1, "One"), (s2, "Two"), (s3, "Three"), (s4, "Four")],
        &[(list_id, "Favourites")],
    );
    let payload = json!({
        "additions": [
            { "list_id": list_id, "reason": " same universe ", "series_ids": [s1, s1, Uuid::new_v4()] },
            { "list_id": Uuid::new_v4(), "reason": "unknown list", "series_ids": [s2] },
        ],
        "new_lists": [
            {
                "name": " Duo ",
                "description": "Two series that go together.",
                "reason": " shared theme ",
                "series_ids": [s2, s3, s4],
            },
            {
                "name": "duo",
                "description": "Duplicate name.",
                "reason": "duplicate",
                "series_ids": [s2, s3],
            },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert_eq!(parsed.additions.len(), 1);
    assert_eq!(parsed.additions[0].list_id, list_id);
    assert_eq!(parsed.additions[0].list_name, "Favourites");
    assert_eq!(parsed.additions[0].reason, "same universe");
    assert_eq!(
        parsed.additions[0]
            .series
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        vec![s1]
    );

    assert_eq!(parsed.new_lists.len(), 1);
    assert_eq!(parsed.new_lists[0].name, "Duo");
    assert_eq!(
        parsed.new_lists[0].description,
        "Two series that go together."
    );
    assert_eq!(parsed.new_lists[0].reason, "shared theme");
    assert_eq!(
        parsed.new_lists[0]
            .series
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        vec![s2, s3, s4]
    );
}

#[test]
fn parse_drops_new_lists_without_a_real_description() {
    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let ctx = context(&[(s1, "One"), (s2, "Two")], &[]);
    let payload = json!({
        "new_lists": [
            { "name": "Duo", "description": "   ", "reason": "r", "series_ids": [s1, s2] },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert!(parsed.new_lists.is_empty());
}

#[test]
fn parse_drops_new_lists_with_a_single_series() {
    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let ctx = context(&[(s1, "One"), (s2, "Two")], &[]);
    let payload = json!({
        "new_lists": [
            { "name": "Solo", "description": "Only one.", "reason": "r", "series_ids": [s1] },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert!(parsed.new_lists.is_empty());
}

#[test]
fn parse_does_not_let_an_invalid_list_claim_its_name() {
    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let s3 = Uuid::new_v4();
    let ctx = context(&[(s1, "One"), (s2, "Two"), (s3, "Three")], &[]);
    let payload = json!({
        "new_lists": [
            { "name": "Solo", "description": "Only one.", "reason": "r", "series_ids": [s1] },
            { "name": "solo", "description": "Two series.", "reason": "r", "series_ids": [s2, s3] },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert_eq!(parsed.new_lists.len(), 1);
    assert_eq!(parsed.new_lists[0].description, "Two series.");
    assert_eq!(
        parsed.new_lists[0]
            .series
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        vec![s2, s3]
    );
}

#[test]
fn parse_does_not_recreate_an_existing_list() {
    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let list_id = Uuid::new_v4();
    let ctx = context(&[(s1, "One"), (s2, "Two")], &[(list_id, "Favourites")]);
    let payload = json!({
        "new_lists": [
            {
                "name": " favourites ",
                "description": "Already exists.",
                "reason": "r",
                "series_ids": [s1, s2],
            },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert!(parsed.new_lists.is_empty());
}

#[test]
fn parse_drops_proposals_without_a_reason() {
    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let list_id = Uuid::new_v4();
    let ctx = context(&[(s1, "One"), (s2, "Two")], &[(list_id, "Favourites")]);
    let payload = json!({
        "additions": [
            { "list_id": list_id, "reason": "   ", "series_ids": [s1] },
        ],
        "new_lists": [
            { "name": "Duo", "description": "Together.", "series_ids": [s2] },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert!(parsed.additions.is_empty());
    assert!(parsed.new_lists.is_empty());
}

#[test]
fn parse_never_reuses_a_series_across_suggestions() {
    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let list_id = Uuid::new_v4();
    let ctx = context(&[(s1, "One"), (s2, "Two")], &[(list_id, "Favourites")]);
    let payload = json!({
        "additions": [
            { "list_id": list_id, "reason": "r", "series_ids": [s1] },
        ],
        "new_lists": [
            { "name": "Duo", "description": "Together.", "reason": "r", "series_ids": [s1, s2] },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert_eq!(parsed.additions.len(), 1);
    // s1 is already used by the addition, leaving only one series: the new list is dropped.
    assert!(parsed.new_lists.is_empty());
}

#[test]
fn parse_keeps_a_valid_move_with_the_real_list_names() {
    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    let ctx = context_with_memberships(
        &[(s1, "One"), (s2, "Two")],
        &[(from, "Favourites"), (to, "Space operas")],
        &[(s1, from)],
    );
    let payload = json!({
        "moves": [
            {
                "from_list_id": from,
                "to_list_id": to,
                "reason": " space opera ",
                "series_ids": [s1, s1, Uuid::new_v4()],
            },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert_eq!(parsed.moves.len(), 1);
    let proposal = &parsed.moves[0];
    assert_eq!(proposal.from_list_id, from);
    assert_eq!(proposal.from_list_name, "Favourites");
    assert_eq!(proposal.to_list_id, to);
    assert_eq!(proposal.to_list_name, "Space operas");
    assert_eq!(proposal.reason, "space opera");
    assert_eq!(
        proposal.series.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![s1]
    );
}

#[test]
fn parse_rejects_moves_contradicting_the_database_membership() {
    let s1 = Uuid::new_v4();
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    // s1 actually belongs to `to`, not to `from`.
    let ctx = context_with_memberships(
        &[(s1, "One")],
        &[(from, "Favourites"), (to, "Space operas")],
        &[(s1, to)],
    );
    let payload = json!({
        "moves": [
            { "from_list_id": from, "to_list_id": to, "reason": "r", "series_ids": [s1] },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert!(parsed.moves.is_empty());
}

#[test]
fn parse_rejects_self_moves_and_unknown_lists() {
    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let from = Uuid::new_v4();
    let other = Uuid::new_v4();
    let ctx = context_with_memberships(
        &[(s1, "One"), (s2, "Two")],
        &[(from, "Favourites"), (other, "Space operas")],
        &[(s1, from), (s2, from)],
    );
    let payload = json!({
        "moves": [
            { "from_list_id": from, "to_list_id": from, "reason": "r", "series_ids": [s1] },
            {
                "from_list_id": from,
                "to_list_id": Uuid::new_v4(),
                "reason": "r",
                "series_ids": [s2],
            },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert!(parsed.moves.is_empty());
}

#[test]
fn parse_never_moves_the_same_series_twice() {
    let s1 = Uuid::new_v4();
    let from = Uuid::new_v4();
    let to_a = Uuid::new_v4();
    let to_b = Uuid::new_v4();
    let ctx = context_with_memberships(
        &[(s1, "One")],
        &[(from, "Favourites"), (to_a, "A"), (to_b, "B")],
        &[(s1, from)],
    );
    let payload = json!({
        "moves": [
            { "from_list_id": from, "to_list_id": to_a, "reason": "r", "series_ids": [s1] },
            { "from_list_id": from, "to_list_id": to_b, "reason": "r", "series_ids": [s1] },
        ],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert_eq!(parsed.moves.len(), 1);
    assert_eq!(parsed.moves[0].to_list_id, to_a);
}

#[test]
fn parse_truncates_long_fields() {
    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let ctx = context(&[(s1, "One"), (s2, "Two")], &[]);
    let payload = json!({
        "new_lists": [{
            "name": "N".repeat(200),
            "description": "D".repeat(800),
            "reason": "R".repeat(500),
            "series_ids": [s1, s2],
        }],
    });

    let parsed = parse_suggestions(&payload, &ctx);

    assert_eq!(parsed.new_lists[0].name.chars().count(), 120);
    assert_eq!(parsed.new_lists[0].description.chars().count(), 500);
    assert_eq!(parsed.new_lists[0].reason.chars().count(), 300);
}
