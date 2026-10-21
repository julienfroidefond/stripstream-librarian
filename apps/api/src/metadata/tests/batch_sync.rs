use super::*;

fn candidate(title: &str, confidence: f32) -> metadata_providers::SeriesCandidate {
    metadata_providers::SeriesCandidate {
        external_id: format!("test:{title}"),
        title: title.to_string(),
        authors: vec![],
        description: None,
        publishers: vec![],
        start_year: None,
        total_volumes: None,
        cover_url: None,
        external_url: None,
        confidence,
        metadata_json: serde_json::json!({}),
    }
}

// ---------------------------------------------------------------------------
// Prudence rule: only a perfect confidence is auto-matched
// ---------------------------------------------------------------------------

#[test]
fn confident_secondary_candidate_is_auto_matched() {
    let outcome = evaluate_candidates(vec![candidate("Blacksad", 1.0)]);
    match outcome {
        SearchOutcome::AutoMatch(c) => assert_eq!(c.title, "Blacksad"),
        _ => panic!("confidence 1.0 must auto-match"),
    }
}

#[test]
fn uncertain_secondary_candidate_is_not_auto_matched() {
    let outcome = evaluate_candidates(vec![candidate("Blacksad", 0.9)]);
    match outcome {
        SearchOutcome::LowConfidence(c) => assert_eq!(c.title, "Blacksad"),
        _ => panic!("confidence below 1.0 must stay for manual review"),
    }
}

#[test]
fn uncertain_best_candidate_is_not_auto_matched() {
    let outcome = evaluate_candidates(vec![
        candidate("Blacksad", 0.95),
        candidate("Blacksad (intégrale)", 0.6),
    ]);
    assert!(matches!(outcome, SearchOutcome::LowConfidence(_)));
}

#[test]
fn no_candidates_is_no_results() {
    assert!(matches!(
        evaluate_candidates(vec![]),
        SearchOutcome::NoResults
    ));
}

// ---------------------------------------------------------------------------
// Provider selection: secondary can be matched in the same job
// ---------------------------------------------------------------------------

fn outcome(status: &'static str, provider: &str) -> ProviderOutcome {
    ProviderOutcome {
        status,
        provider: provider.to_string(),
        fallback_used: provider == "bdphile",
        candidates_count: 1,
        best_confidence: match status {
            "auto_matched" => Some(1.0),
            "low_confidence" => Some(0.7),
            _ => None,
        },
        best_candidate: None,
        link_id: None,
        message: None,
    }
}

#[test]
fn confident_secondary_is_selected_when_primary_is_uncertain() {
    let primary = outcome("low_confidence", "bedetheque");
    let secondary = outcome("auto_matched", "bdphile");

    let (main, from_secondary) =
        select_main_outcome(Some(&primary), Some(&secondary)).expect("an outcome is selected");

    assert_eq!(main.provider, "bdphile");
    assert!(
        from_secondary,
        "the reported match comes from the secondary"
    );
    assert!(main.is_auto_matched());
}

#[test]
fn uncertain_secondary_does_not_override_primary() {
    let primary = outcome("low_confidence", "bedetheque");
    let secondary = outcome("low_confidence", "bdphile");

    let (main, from_secondary) =
        select_main_outcome(Some(&primary), Some(&secondary)).expect("an outcome is selected");

    assert_eq!(main.provider, "bedetheque");
    assert!(!from_secondary);
    assert!(
        !main.is_auto_matched(),
        "two uncertain providers must never produce an auto-match"
    );
}

#[test]
fn confident_primary_wins_over_confident_secondary() {
    let primary = outcome("auto_matched", "bedetheque");
    let secondary = outcome("auto_matched", "bdphile");

    let (main, from_secondary) =
        select_main_outcome(Some(&primary), Some(&secondary)).expect("an outcome is selected");

    assert_eq!(main.provider, "bedetheque");
    assert!(
        !from_secondary,
        "a confident primary stays the reported provider"
    );
}

#[test]
fn secondary_is_selected_when_primary_is_already_linked() {
    let secondary = outcome("auto_matched", "bdphile");

    let (main, from_secondary) =
        select_main_outcome(None, Some(&secondary)).expect("the secondary is evaluated");

    assert_eq!(main.provider, "bdphile");
    assert!(from_secondary);
}

#[test]
fn linked_primary_with_uncertain_secondary_stays_already_linked() {
    let no_results = outcome("no_results", "bdphile");
    assert!(matches!(
        select_series_outcome(true, false, false, None, Some(&no_results)),
        SeriesOutcome::AlreadyLinked
    ));

    let low_confidence = outcome("low_confidence", "bdphile");
    assert!(
        matches!(
            select_series_outcome(true, false, false, None, Some(&low_confidence)),
            SeriesOutcome::AlreadyLinked
        ),
        "a linked series must not be downgraded by an uncertain secondary"
    );
}

#[test]
fn linked_primary_with_confident_secondary_is_reported() {
    let secondary = outcome("auto_matched", "bdphile");

    match select_series_outcome(true, false, false, None, Some(&secondary)) {
        SeriesOutcome::Reported {
            main,
            from_secondary,
        } => {
            assert_eq!(main.provider, "bdphile");
            assert!(from_secondary);
        }
        _ => panic!("a confident secondary must still be auto-matched"),
    }
}

#[test]
fn both_providers_linked_is_already_linked() {
    assert!(matches!(
        select_series_outcome(true, true, false, None, None),
        SeriesOutcome::AlreadyLinked
    ));
}

#[test]
fn rate_limited_is_only_an_error_without_any_link() {
    assert!(matches!(
        select_series_outcome(false, false, true, None, None),
        SeriesOutcome::RateLimited
    ));
    assert!(
        matches!(
            select_series_outcome(true, false, true, None, None),
            SeriesOutcome::AlreadyLinked
        ),
        "a series already linked to its primary must not turn red when SC is rate-limited"
    );
}

#[test]
fn primary_error_with_confident_secondary_reports_secondary() {
    let primary = outcome("error", "bedetheque");
    let secondary = outcome("auto_matched", "bdphile");

    match select_series_outcome(false, false, false, Some(&primary), Some(&secondary)) {
        SeriesOutcome::Reported {
            main,
            from_secondary,
        } => {
            assert_eq!(main.provider, "bdphile");
            assert!(from_secondary);
        }
        _ => panic!("a confident secondary must be reported when the primary fails"),
    }
}

// ---------------------------------------------------------------------------
// Per-provider link tracking (#150)
// ---------------------------------------------------------------------------

#[test]
fn approved_links_are_tracked_per_provider() {
    let links = approved_links_by_series(vec![
        ("Asterix".to_string(), "bedetheque".to_string()),
        ("Asterix".to_string(), "bdphile".to_string()),
        ("Naruto".to_string(), "bedetheque".to_string()),
    ]);

    assert!(has_approved_link(&links, "Asterix", "bedetheque"));
    assert!(has_approved_link(&links, "Asterix", "bdphile"));
    assert!(
        !has_approved_link(&links, "Asterix", "senscritique"),
        "a series linked to one provider must not look linked for another"
    );
    assert!(has_approved_link(&links, "Naruto", "bedetheque"));
    assert!(!has_approved_link(&links, "Naruto", "bdphile"));
    assert!(!has_approved_link(&links, "Unknown", "bedetheque"));
}

async fn create_lib(pool: &sqlx::PgPool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(name)
        .bind(format!("/libraries/{name}"))
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn create_series(pool: &sqlx::PgPool, lib_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO series (id, library_id, name) VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(lib_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_link(
    pool: &sqlx::PgPool,
    lib_id: Uuid,
    series_id: Uuid,
    provider: &str,
    external_id: &str,
) {
    sqlx::query(
        "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id, status, confidence) \
         VALUES ($1, $2, $3, $4, 'approved', 1.0)",
    )
    .bind(lib_id)
    .bind(series_id)
    .bind(provider)
    .bind(external_id)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn load_approved_links_evaluates_each_provider_separately(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "per_provider").await;
    let asterix = create_series(&pool, lib_id, "Asterix").await;
    let naruto = create_series(&pool, lib_id, "Naruto").await;

    // Asterix has both providers linked; Naruto only the primary.
    create_link(&pool, lib_id, asterix, "bedetheque", "bd_1").await;
    create_link(&pool, lib_id, asterix, "bdphile", "bdp_1").await;
    create_link(&pool, lib_id, naruto, "bedetheque", "bd_2").await;

    let links = load_approved_links(&pool, lib_id).await.unwrap();

    assert!(has_approved_link(&links, "Asterix", "bedetheque"));
    assert!(has_approved_link(&links, "Asterix", "bdphile"));

    // The regression: a primary link must not make the secondary look linked.
    assert!(
        !has_approved_link(&links, "Naruto", "bdphile"),
        "Naruto is only linked to bedetheque, so bdphile still needs a match"
    );
    assert!(has_approved_link(&links, "Naruto", "bedetheque"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn load_approved_links_ignores_pending_links(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "pending_links").await;
    let series_id = create_series(&pool, lib_id, "Blacksad").await;

    sqlx::query(
        "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id, status) \
         VALUES ($1, $2, 'bdphile', 'bdp_1', 'pending')",
    )
    .bind(lib_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    let links = load_approved_links(&pool, lib_id).await.unwrap();
    assert!(!has_approved_link(&links, "Blacksad", "bdphile"));
}

/// Rematch cleanup must keep the configured secondary link while dropping
/// links from providers that are no longer configured (#150).
#[sqlx::test(migrations = "../../infra/migrations")]
async fn rematch_cleanup_keeps_primary_and_secondary_links(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "secondary_cleanup").await;
    let series_id = create_series(&pool, lib_id, "Asterix").await;

    sqlx::query(
        "INSERT INTO external_metadata_links \
             (library_id, series_id, provider, external_id, status, confidence, is_primary) \
         VALUES ($1, $2, 'bedetheque', 'bd_1', 'approved', 1.0, true), \
                ($1, $2, 'bdphile', 'bdp_1', 'approved', 1.0, false), \
                ($1, $2, 'anilist', 'an_1', 'approved', 1.0, false)",
    )
    .bind(lib_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    let keep_providers = vec!["bedetheque".to_string(), "bdphile".to_string()];
    let deleted = cleanup_rematch_links(&pool, lib_id, "Asterix", &keep_providers)
        .await
        .unwrap();

    assert_eq!(deleted, 1, "only the anilist link is dropped");

    let providers: Vec<String> = sqlx::query_scalar(
        "SELECT provider FROM external_metadata_links WHERE series_id = $1 ORDER BY provider",
    )
    .bind(series_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(providers, vec!["bdphile", "bedetheque"]);

    let primary: String = sqlx::query_scalar(
        "SELECT provider FROM external_metadata_links WHERE series_id = $1 AND is_primary",
    )
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(primary, "bedetheque", "the primary link is untouched");
}

/// When the old primary is no longer configured, the freshly matched link must
/// become the reference provider instead of leaving the series without one.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn rematch_cleanup_promotes_the_remaining_link(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "promote_after_cleanup").await;
    let series_id = create_series(&pool, lib_id, "Naruto").await;

    sqlx::query(
        "INSERT INTO external_metadata_links \
             (library_id, series_id, provider, external_id, status, confidence, is_primary) \
         VALUES ($1, $2, 'bedetheque', 'bd_1', 'approved', 1.0, true), \
                ($1, $2, 'senscritique', 'sc_1', 'approved', 1.0, false)",
    )
    .bind(lib_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    let keep_providers = vec!["senscritique".to_string()];
    let deleted = cleanup_rematch_links(&pool, lib_id, "Naruto", &keep_providers)
        .await
        .unwrap();
    assert_eq!(deleted, 1, "the old primary link is dropped");

    let (provider, is_primary): (String, bool) = sqlx::query_as(
        "SELECT provider, is_primary FROM external_metadata_links WHERE series_id = $1",
    )
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(provider, "senscritique");
    assert!(is_primary, "the remaining link must become primary");
}
