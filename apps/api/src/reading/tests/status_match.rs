use super::normalize_title;

#[test]
fn normalize_basic() {
    assert_eq!(normalize_title("Hello World"), "hello world");
}

#[test]
fn normalize_removes_punctuation() {
    assert_eq!(
        normalize_title("One Piece: Stampede!"),
        "one piece stampede"
    );
}

#[test]
fn normalize_collapses_whitespace() {
    assert_eq!(
        normalize_title("  Naruto   Shippuden  "),
        "naruto shippuden"
    );
}

#[test]
fn normalize_replaces_special_chars() {
    assert_eq!(normalize_title("Dragon-Ball_Z"), "dragon ball z");
    assert_eq!(
        normalize_title("JoJo's Bizarre Adventure"),
        "jojo s bizarre adventure"
    );
    assert_eq!(normalize_title("What...?!"), "what");
}

#[test]
fn normalize_mixed_case() {
    assert_eq!(
        normalize_title("FULLMETAL ALCHEMIST"),
        "fullmetal alchemist"
    );
    assert_eq!(normalize_title("FuLlMeTaL"), "fullmetal");
}

#[test]
fn normalize_empty_string() {
    assert_eq!(normalize_title(""), "");
}

#[test]
fn normalize_only_punctuation() {
    assert_eq!(normalize_title(":!?.,'-\"_"), "");
}

#[test]
fn normalize_preserves_numbers() {
    assert_eq!(normalize_title("One Piece 100"), "one piece 100");
}

#[test]
fn normalize_quotes_replaced() {
    assert_eq!(normalize_title("\"Quoted Title\""), "quoted title");
}

#[test]
fn normalize_comma_in_title() {
    assert_eq!(normalize_title("Spy, Family"), "spy family");
}

#[test]
fn normalize_idempotent() {
    let input = "already normalized";
    assert_eq!(normalize_title(input), input);
}

#[test]
fn normalize_consecutive_special_chars() {
    assert_eq!(normalize_title("title---subtitle"), "title subtitle");
}

// -----------------------------------------------------------------------
// insert_event tests
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn event_anilist_linked_written(pool: sqlx::PgPool) {
    use sqlx::Row;
    use uuid::Uuid;

    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestEvtLib', '/libraries/test_evt')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'running', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    super::insert_event(
        &pool,
        job_id,
        "anilist_linked",
        "info",
        Some("Naruto"),
        None,
        None,
    )
    .await;

    let row = sqlx::query(
            "SELECT event_type, level, entity_name, message, detail FROM index_job_events WHERE job_id = $1",
        )
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<String, _>("event_type"), "anilist_linked");
    assert_eq!(row.get::<String, _>("level"), "info");
    assert_eq!(
        row.get::<Option<String>, _>("entity_name"),
        Some("Naruto".to_string())
    );
    assert!(row.get::<Option<String>, _>("message").is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn event_error_has_error_level(pool: sqlx::PgPool) {
    use sqlx::Row;
    use uuid::Uuid;

    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestEvtErrLib', '/libraries/test_evt_err')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'running', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    super::insert_event(
        &pool,
        job_id,
        "error",
        "error",
        Some("BrokenSeries"),
        Some("AniList API failed"),
        None,
    )
    .await;

    let row = sqlx::query(
        "SELECT event_type, level, entity_name, message FROM index_job_events WHERE job_id = $1",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.get::<String, _>("event_type"), "error");
    assert_eq!(row.get::<String, _>("level"), "error");
    assert_eq!(
        row.get::<Option<String>, _>("entity_name"),
        Some("BrokenSeries".to_string())
    );
    assert_eq!(
        row.get::<Option<String>, _>("message"),
        Some("AniList API failed".to_string())
    );
}

/// Regression: series_id must be populated via LEFT JOIN series when the series exists.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_id_returned_when_series_exists(pool: sqlx::PgPool) {
    use sqlx::Row;
    use uuid::Uuid;

    let library_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestLib', '/libraries/test')",
    )
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Naruto')")
        .bind(series_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_name) \
             VALUES ($1, 'anilist_linked', 'info', 'Naruto')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    // Run the actual query from get_match_results (no status filter)
    let rows = sqlx::query(
            "SELECT e.id, e.entity_name, e.event_type, e.message, e.detail, s.id AS series_id
             FROM index_job_events e
             LEFT JOIN series s ON s.library_id = $2 AND LOWER(unaccent(s.name)) = LOWER(unaccent(e.entity_name))
             WHERE e.job_id = $1
             ORDER BY e.event_type, e.entity_name",
        )
        .bind(job_id)
        .bind(Some(library_id))
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    let returned_series_id: Option<Uuid> = rows[0].get("series_id");
    assert_eq!(
        returned_series_id,
        Some(series_id),
        "series_id should match the created series"
    );
}

/// Regression: series_id should be None when no matching series exists.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_id_none_when_series_missing(pool: sqlx::PgPool) {
    use sqlx::Row;
    use uuid::Uuid;

    let library_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestLib', '/libraries/test')",
    )
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert event with a series_name that does NOT exist in series table
    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_name) \
             VALUES ($1, 'anilist_no_results', 'info', 'NonExistentSeries')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    let rows = sqlx::query(
            "SELECT e.id, e.entity_name, e.event_type, e.message, e.detail, s.id AS series_id
             FROM index_job_events e
             LEFT JOIN series s ON s.library_id = $2 AND LOWER(unaccent(s.name)) = LOWER(unaccent(e.entity_name))
             WHERE e.job_id = $1
             ORDER BY e.event_type, e.entity_name",
        )
        .bind(job_id)
        .bind(Some(library_id))
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    let returned_series_id: Option<Uuid> = rows[0].get("series_id");
    assert!(
        returned_series_id.is_none(),
        "series_id should be None when series does not exist"
    );
}

/// Regression: LEFT JOIN with LOWER(unaccent()) resolves series_id even when
/// the event entity_name has different accents/casing than the series name.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_id_resolved_with_unaccent_in_results(pool: sqlx::PgPool) {
    use sqlx::Row;
    use uuid::Uuid;

    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestUnaccent', '/libraries/test_unaccent')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

    // Series with accented name
    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Astérix')")
        .bind(series_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert event with entity_name WITHOUT accent
    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_name) \
             VALUES ($1, 'anilist_linked', 'info', 'Asterix')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    // Run the actual query from get_match_results
    let rows = sqlx::query(
            "SELECT e.id, e.entity_name, e.event_type, e.message, e.detail, s.id AS series_id
             FROM index_job_events e
             LEFT JOIN series s ON s.library_id = $2 AND LOWER(unaccent(s.name)) = LOWER(unaccent(e.entity_name))
             WHERE e.job_id = $1
             ORDER BY e.event_type, e.entity_name",
        )
        .bind(job_id)
        .bind(Some(library_id))
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    let returned_series_id: Option<Uuid> = rows[0].get("series_id");
    assert_eq!(
        returned_series_id,
        Some(series_id),
        "series_id should be resolved despite accent difference (Astérix vs Asterix)"
    );
}

// -----------------------------------------------------------------------
// Report endpoint: GROUP BY event_type counts from index_job_events
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn report_counts_from_events(pool: sqlx::PgPool) {
    use sqlx::Row;
    use uuid::Uuid;

    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'MatchReportLib', '/libraries/match_report')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at, total_files) VALUES ($1, $2, 'reading_status_match', 'success', NOW(), 7)",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert events with different event_types
    super::insert_event(
        &pool,
        job_id,
        "anilist_linked",
        "info",
        Some("Naruto"),
        None,
        Some(serde_json::json!({"anilist_id": 20})),
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "anilist_linked",
        "info",
        Some("Bleach"),
        None,
        Some(serde_json::json!({"anilist_id": 21})),
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "anilist_already_linked",
        "info",
        Some("OnePiece"),
        None,
        None,
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "anilist_no_results",
        "info",
        Some("Obscure"),
        None,
        None,
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "anilist_no_results",
        "info",
        Some("Obscure2"),
        None,
        None,
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "anilist_ambiguous",
        "warning",
        Some("Dragon"),
        None,
        None,
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "error",
        "error",
        Some("Broken"),
        Some("API timeout"),
        None,
    )
    .await;

    // Run the report query
    let counts = sqlx::query(
            "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
        )
        .bind(job_id)
        .fetch_all(&pool)
        .await
        .unwrap();

    let mut linked = 0i64;
    let mut already_linked = 0i64;
    let mut no_results = 0i64;
    let mut ambiguous = 0i64;
    let mut errors = 0i64;

    for r in &counts {
        let event_type: String = r.get("event_type");
        let cnt: i64 = r.get("cnt");
        match event_type.as_str() {
            "anilist_linked" => linked = cnt,
            "anilist_already_linked" => already_linked = cnt,
            "anilist_no_results" => no_results = cnt,
            "anilist_ambiguous" => ambiguous = cnt,
            "error" => errors = cnt,
            _ => {}
        }
    }

    assert_eq!(linked, 2, "anilist_linked -> linked");
    assert_eq!(
        already_linked, 1,
        "anilist_already_linked -> already_linked"
    );
    assert_eq!(no_results, 2, "anilist_no_results -> no_results");
    assert_eq!(ambiguous, 1, "anilist_ambiguous -> ambiguous");
    assert_eq!(errors, 1, "error -> errors");
}

// -----------------------------------------------------------------------
// Results endpoint: event detail JSON mapped to ReadingStatusMatchResultDto
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn results_mapped_from_event_detail(pool: sqlx::PgPool) {
    use sqlx::Row;
    use uuid::Uuid;

    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'MatchResultLib', '/libraries/match_result')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let detail = serde_json::json!({
        "anilist_id": 12345,
        "anilist_title": "Naruto Shippuden",
        "anilist_url": "https://anilist.co/manga/12345",
    });

    super::insert_event(
        &pool,
        job_id,
        "anilist_linked",
        "info",
        Some("Naruto"),
        None,
        Some(detail),
    )
    .await;

    // Run the actual results query (no filter)
    let rows = sqlx::query(
            "SELECT e.id, e.entity_name, e.event_type, e.message, e.detail, s.id AS series_id
             FROM index_job_events e
             LEFT JOIN series s ON s.library_id = $2 AND LOWER(unaccent(s.name)) = LOWER(unaccent(e.entity_name))
             WHERE e.job_id = $1
             ORDER BY e.event_type, e.entity_name",
        )
        .bind(job_id)
        .bind(Some(library_id))
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);

    let row = &rows[0];
    let event_type: String = row.get("event_type");
    let row_detail: Option<serde_json::Value> = row.get("detail");

    // Map event_type to status like the endpoint does
    let status = match event_type.as_str() {
        "anilist_linked" => "linked",
        "anilist_already_linked" => "already_linked",
        "anilist_no_results" => "no_results",
        "anilist_ambiguous" => "ambiguous",
        "error" => "error",
        other => other,
    };

    assert_eq!(status, "linked");

    let d = row_detail.as_ref().unwrap();
    let anilist_id = d["anilist_id"].as_i64().map(|v| v as i32);
    let anilist_title = d["anilist_title"].as_str().map(String::from);
    let anilist_url = d["anilist_url"].as_str().map(String::from);

    assert_eq!(anilist_id, Some(12345));
    assert_eq!(anilist_title.as_deref(), Some("Naruto Shippuden"));
    assert_eq!(
        anilist_url.as_deref(),
        Some("https://anilist.co/manga/12345")
    );

    let entity_name: Option<String> = row.get("entity_name");
    assert_eq!(entity_name.as_deref(), Some("Naruto"));
}
