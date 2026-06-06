use sqlx::Row;
use uuid::Uuid;

#[sqlx::test(migrations = "../../infra/migrations")]
async fn event_status_pushed_written(pool: sqlx::PgPool) {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'PushEvtLib', '/libraries/push_evt')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_push', 'running', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let detail = serde_json::json!({"anilist_status": "CURRENT", "progress": 5});
    super::insert_event(
        &pool,
        job_id,
        "status_pushed",
        "info",
        Some("One Piece"),
        None,
        Some(detail.clone()),
    )
    .await;

    let row = sqlx::query(
            "SELECT event_type, level, entity_name, message, detail FROM index_job_events WHERE job_id = $1",
        )
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<String, _>("event_type"), "status_pushed");
    assert_eq!(row.get::<String, _>("level"), "info");
    assert_eq!(
        row.get::<Option<String>, _>("entity_name"),
        Some("One Piece".to_string())
    );
    let stored_detail: serde_json::Value = row.get("detail");
    assert_eq!(stored_detail["anilist_status"], "CURRENT");
    assert_eq!(stored_detail["progress"], 5);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn event_error_has_error_level(pool: sqlx::PgPool) {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'PushEvtErrLib', '/libraries/push_evt_err')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_push', 'running', NOW())",
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
        Some("FailedSeries"),
        Some("rate limit hit"),
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
        Some("FailedSeries".to_string())
    );
    assert_eq!(
        row.get::<Option<String>, _>("message"),
        Some("rate limit hit".to_string())
    );
}

/// Regression: LEFT JOIN with LOWER(unaccent()) resolves series_id even when
/// the event entity_name has different accents/casing than the series name.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_id_resolved_with_unaccent_in_results(pool: sqlx::PgPool) {
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
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_push', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert event with entity_name WITHOUT accent
    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_name) \
             VALUES ($1, 'status_pushed', 'info', 'Asterix')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    // Run the actual results query
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
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'PushReportLib', '/libraries/push_report')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at, total_files) VALUES ($1, $2, 'reading_status_push', 'success', NOW(), 6)",
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
        "status_pushed",
        "info",
        Some("Naruto"),
        None,
        Some(serde_json::json!({"anilist_status": "CURRENT", "progress": 5})),
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "status_pushed",
        "info",
        Some("Bleach"),
        None,
        Some(serde_json::json!({"anilist_status": "COMPLETED", "progress": 74})),
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "status_skipped",
        "info",
        Some("OnePiece"),
        None,
        None,
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "status_no_books",
        "info",
        Some("EmptySeries"),
        None,
        None,
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "status_no_books",
        "info",
        Some("EmptySeries2"),
        None,
        None,
    )
    .await;
    super::insert_event(
        &pool,
        job_id,
        "error",
        "error",
        Some("BrokenSeries"),
        Some("rate limit"),
        None,
    )
    .await;

    // Run the same report query used in get_push_report
    let counts = sqlx::query(
            "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
        )
        .bind(job_id)
        .fetch_all(&pool)
        .await
        .unwrap();

    let mut pushed = 0i64;
    let mut skipped = 0i64;
    let mut no_books = 0i64;
    let mut errors = 0i64;

    for r in &counts {
        let event_type: String = r.get("event_type");
        let cnt: i64 = r.get("cnt");
        match event_type.as_str() {
            "status_pushed" => pushed = cnt,
            "status_skipped" => skipped = cnt,
            "status_no_books" => no_books = cnt,
            "error" => errors = cnt,
            _ => {}
        }
    }

    assert_eq!(pushed, 2, "status_pushed -> pushed");
    assert_eq!(skipped, 1, "status_skipped -> skipped");
    assert_eq!(no_books, 2, "status_no_books -> no_books");
    assert_eq!(errors, 1, "error -> errors");
}

// -----------------------------------------------------------------------
// Results endpoint: event detail JSON mapped to ReadingStatusPushResultDto
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn results_mapped_from_event_detail(pool: sqlx::PgPool) {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'PushResultLib', '/libraries/push_result')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_push', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let detail = serde_json::json!({
        "anilist_id": 54321,
        "anilist_title": "Naruto",
        "anilist_url": "https://anilist.co/manga/54321",
        "anilist_status": "CURRENT",
        "progress": 42,
    });

    super::insert_event(
        &pool,
        job_id,
        "status_pushed",
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
        "status_pushed" => "pushed",
        "status_skipped" => "skipped",
        "status_no_books" => "no_books",
        "error" => "error",
        other => other,
    };

    assert_eq!(status, "pushed");

    let d = row_detail.as_ref().unwrap();
    let anilist_id = d["anilist_id"].as_i64().map(|v| v as i32);
    let anilist_title = d["anilist_title"].as_str().map(String::from);
    let anilist_url = d["anilist_url"].as_str().map(String::from);
    let anilist_status = d["anilist_status"].as_str().map(String::from);
    let progress = d["progress"].as_i64().map(|v| v as i32);

    assert_eq!(anilist_id, Some(54321));
    assert_eq!(anilist_title.as_deref(), Some("Naruto"));
    assert_eq!(
        anilist_url.as_deref(),
        Some("https://anilist.co/manga/54321")
    );
    assert_eq!(anilist_status.as_deref(), Some("CURRENT"));
    assert_eq!(progress, Some(42));

    let entity_name: Option<String> = row.get("entity_name");
    assert_eq!(entity_name.as_deref(), Some("Naruto"));

    let error_message: Option<String> = row.get("message");
    assert!(
        error_message.is_none(),
        "pushed event should have no error message"
    );
}
