use sqlx::Row;
use uuid::Uuid;

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

async fn create_link(pool: &sqlx::PgPool, lib_id: Uuid, series_id: Uuid, provider: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id, status, confidence) \
         VALUES ($1, $2, $3, 'ext_123', 'approved', 0.9) RETURNING id",
    ).bind(lib_id).bind(series_id).bind(provider).fetch_one(pool).await.unwrap()
}

/// Test that `auto_apply` with ON CONFLICT replaces the link when same provider.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn auto_apply_upserts_same_provider(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "test").await;
    let series_id = create_series(&pool, lib_id, "Blacksad").await;
    let old_link = create_link(&pool, lib_id, series_id, "bedetheque").await;

    // Simulate auto_apply: insert with same provider -> ON CONFLICT updates
    let new_link_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO external_metadata_links
            (library_id, series_id, provider, external_id, status, confidence)
        VALUES ($1, $2, 'bedetheque', 'new_ext_456', 'approved', 0.95)
        ON CONFLICT (series_id, provider) DO UPDATE SET
            external_id = EXCLUDED.external_id, confidence = EXCLUDED.confidence,
            status = 'approved', updated_at = NOW()
        RETURNING id"#,
    )
    .bind(lib_id)
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        new_link_id, old_link,
        "upsert should return the same link id"
    );
    let ext_id: String =
        sqlx::query_scalar("SELECT external_id FROM external_metadata_links WHERE id = $1")
            .bind(old_link)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ext_id, "new_ext_456", "external_id should be updated");
}

/// Test that rematch deletes old link from different provider after new match.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn rematch_deletes_old_provider_link(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "test").await;
    let series_id = create_series(&pool, lib_id, "Naruto").await;
    let _old_link = create_link(&pool, lib_id, series_id, "bedetheque").await;

    // New link created by auto_apply with different provider
    let new_link: Uuid = sqlx::query_scalar(
        "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id, status, confidence) \
         VALUES ($1, $2, 'senscritique', 'sc_123', 'approved', 0.85) RETURNING id",
    ).bind(lib_id).bind(series_id).fetch_one(&pool).await.unwrap();

    // Simulate the rematch cleanup: delete old links from other providers
    sqlx::query(
        "DELETE FROM external_metadata_links WHERE library_id = $1 AND id != $2 AND series_id = (\
         SELECT series_id FROM external_metadata_links WHERE id = $2)",
    )
    .bind(lib_id)
    .bind(new_link)
    .execute(&pool)
    .await
    .unwrap();

    // Should have only the new senscritique link
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "old bedetheque link should be deleted");

    let provider: String =
        sqlx::query_scalar("SELECT provider FROM external_metadata_links WHERE series_id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(provider, "senscritique");
}

/// Test that without rematch, already_linked series are skipped regardless of provider.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn normal_mode_skips_any_linked_series(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "test").await;
    let series_id = create_series(&pool, lib_id, "Asterix").await;
    let _link = create_link(&pool, lib_id, series_id, "bedetheque").await;

    let linked_rows = sqlx::query(
        "SELECT s.name, eml.provider FROM external_metadata_links eml JOIN series s ON s.id = eml.series_id WHERE eml.library_id = $1 AND eml.status = 'approved'",
    ).bind(lib_id).fetch_all(&pool).await.unwrap();
    let already_linked: std::collections::HashMap<String, String> = linked_rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("name"),
                row.get::<String, _>("provider"),
            )
        })
        .collect();

    let force_rematch = false;
    let linked_provider = already_linked.get("Asterix");
    let should_skip = if force_rematch {
        linked_provider
            .map(|p| p == "senscritique")
            .unwrap_or(false) // target provider
    } else {
        linked_provider.is_some()
    };
    assert!(should_skip, "normal mode should skip any linked series");
}

/// Test that rematch skips series already linked to the TARGET provider.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn rematch_skips_if_already_on_target_provider(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "test").await;
    let series_id = create_series(&pool, lib_id, "Naruto").await;
    let _link = create_link(&pool, lib_id, series_id, "senscritique").await;

    let linked_rows = sqlx::query(
        "SELECT s.name, eml.provider FROM external_metadata_links eml JOIN series s ON s.id = eml.series_id WHERE eml.library_id = $1 AND eml.status = 'approved'",
    ).bind(lib_id).fetch_all(&pool).await.unwrap();
    let already_linked: std::collections::HashMap<String, String> = linked_rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("name"),
                row.get::<String, _>("provider"),
            )
        })
        .collect();

    let primary_name = "senscritique";
    let force_rematch = true;
    let linked_provider = already_linked.get("Naruto");
    let should_skip = if force_rematch {
        linked_provider.map(|p| p == primary_name).unwrap_or(false)
    } else {
        linked_provider.is_some()
    };
    assert!(
        should_skip,
        "rematch should skip if already linked to the target provider"
    );
}

/// Test that rematch does NOT skip series linked to a DIFFERENT provider.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn rematch_does_not_skip_if_linked_to_different_provider(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "test").await;
    let series_id = create_series(&pool, lib_id, "One Piece").await;
    let _link = create_link(&pool, lib_id, series_id, "anilist").await;

    let linked_rows = sqlx::query(
        "SELECT s.name, eml.provider FROM external_metadata_links eml JOIN series s ON s.id = eml.series_id WHERE eml.library_id = $1 AND eml.status = 'approved'",
    ).bind(lib_id).fetch_all(&pool).await.unwrap();
    let already_linked: std::collections::HashMap<String, String> = linked_rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("name"),
                row.get::<String, _>("provider"),
            )
        })
        .collect();

    let primary_name = "senscritique"; // target is senscritique, linked is anilist
    let force_rematch = true;
    let linked_provider = already_linked.get("One Piece");
    let should_skip = if force_rematch {
        linked_provider.map(|p| p == primary_name).unwrap_or(false)
    } else {
        linked_provider.is_some()
    };
    assert!(
        !should_skip,
        "rematch should NOT skip if linked to a different provider"
    );
}

/// Regression test: metadata_batch_rematch must be an allowed job type in DB.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn rematch_job_type_accepted_by_db(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "test").await;
    let job_id = Uuid::new_v4();
    // This INSERT would fail with a CHECK constraint violation if
    // 'metadata_batch_rematch' is not in the allowed job types.
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'metadata_batch_rematch', 'pending')",
    )
    .bind(job_id)
    .bind(lib_id)
    .execute(&pool)
    .await
    .expect("metadata_batch_rematch should be allowed by index_jobs_type_check constraint");
}

/// Regression: series_id must be populated via LEFT JOIN series when the series exists.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_id_returned_when_series_exists(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "test_series_id").await;
    let series_id = create_series(&pool, lib_id, "Blacksad").await;

    // Create a job
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'metadata_batch', 'success', NOW())",
    )
    .bind(job_id)
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    // Insert an event with entity_name matching the series
    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_name, detail) \
         VALUES ($1, 'metadata_matched', 'info', 'series', 'Blacksad', '{\"provider\": \"bedetheque\", \"candidates_count\": 1}'::jsonb)",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    // Run the actual query from get_batch_results
    let rows = sqlx::query(
        r#"
        SELECT ije.id, ije.event_type, ije.entity_id, ije.entity_name, ije.message, ije.detail,
               s.id AS series_id
        FROM index_job_events ije
        LEFT JOIN series s ON s.library_id = $5 AND LOWER(unaccent(s.name)) = LOWER(unaccent(ije.entity_name))
        WHERE ije.job_id = $1
          AND (ije.event_type LIKE 'metadata_%' OR ije.event_type = 'error')
          AND ($2::text IS NULL OR ije.event_type = $2)
        ORDER BY ije.entity_name ASC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(job_id)
    .bind(None::<&str>)
    .bind(100i64)
    .bind(0i64)
    .bind(Some(lib_id))
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
    let lib_id = create_lib(&pool, "test_no_series").await;

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'metadata_batch', 'success', NOW())",
    )
    .bind(job_id)
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    // Insert an event with entity_name that does NOT exist in series table
    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_name) \
         VALUES ($1, 'metadata_no_results', 'info', 'series', 'NonExistentSeries')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    let rows = sqlx::query(
        r#"
        SELECT ije.id, ije.event_type, ije.entity_id, ije.entity_name, ije.message, ije.detail,
               s.id AS series_id
        FROM index_job_events ije
        LEFT JOIN series s ON s.library_id = $5 AND LOWER(unaccent(s.name)) = LOWER(unaccent(ije.entity_name))
        WHERE ije.job_id = $1
          AND (ije.event_type LIKE 'metadata_%' OR ije.event_type = 'error')
          AND ($2::text IS NULL OR ije.event_type = $2)
        ORDER BY ije.entity_name ASC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(job_id)
    .bind(None::<&str>)
    .bind(100i64)
    .bind(0i64)
    .bind(Some(lib_id))
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

// -----------------------------------------------------------------------
// insert_event tests
// -----------------------------------------------------------------------

async fn create_job(pool: &sqlx::PgPool, lib_id: Uuid, job_type: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, $3, 'running', NOW())",
    )
    .bind(id)
    .bind(lib_id)
    .bind(job_type)
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn event_metadata_matched_written(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "evt_matched").await;
    let job_id = create_job(&pool, lib_id, "metadata_batch").await;

    let detail = serde_json::json!({"provider": "bedetheque", "confidence": 0.92});
    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "metadata_matched",
        "info",
        Some("series"),
        Some("Blacksad"),
        None,
        Some(detail.clone()),
    )
    .await;

    let row = sqlx::query(
        "SELECT event_type, level, entity_type, entity_name, message, detail FROM index_job_events WHERE job_id = $1",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.get::<String, _>("event_type"), "metadata_matched");
    assert_eq!(row.get::<String, _>("level"), "info");
    assert_eq!(
        row.get::<Option<String>, _>("entity_type"),
        Some("series".to_string())
    );
    assert_eq!(
        row.get::<Option<String>, _>("entity_name"),
        Some("Blacksad".to_string())
    );
    assert!(row.get::<Option<String>, _>("message").is_none());
    let stored_detail: serde_json::Value = row.get("detail");
    assert_eq!(stored_detail["provider"], "bedetheque");
    assert_eq!(stored_detail["confidence"], 0.92);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn event_error_has_error_level(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "evt_error").await;
    let job_id = create_job(&pool, lib_id, "metadata_batch").await;

    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "error",
        "error",
        Some("series"),
        Some("Broken"),
        Some("provider timeout"),
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
        Some("Broken".to_string())
    );
    assert_eq!(
        row.get::<Option<String>, _>("message"),
        Some("provider timeout".to_string())
    );
}

/// Verify that best_candidate in event detail contains enriched fields for quick match.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn best_candidate_json_contains_enriched_fields(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "test").await;
    let job_id = Uuid::new_v4();
    sqlx::query("INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'metadata_batch', 'success', NOW())")
        .bind(job_id).bind(lib_id).execute(&pool).await.unwrap();

    let candidate_json = serde_json::json!({
        "title": "Blacksad",
        "external_id": "ext_123",
        "external_url": "https://example.com/blacksad",
        "authors": ["Juan Diaz Canales", "Juanjo Guarnido"],
        "description": "A noir detective story.",
        "cover_url": "https://example.com/cover.jpg",
        "total_volumes": 7,
        "start_year": 2000,
        "confidence": 0.65,
    });

    let detail = serde_json::json!({
        "provider": "bedetheque",
        "fallback_used": false,
        "candidates_count": 1,
        "confidence": 0.65,
        "best_candidate": candidate_json,
    });

    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_name, detail) \
         VALUES ($1, 'metadata_low_confidence', 'warning', 'series', 'Blacksad', $2)",
    )
    .bind(job_id).bind(&detail)
    .execute(&pool).await.unwrap();

    let row = sqlx::query(
        "SELECT detail FROM index_job_events WHERE job_id = $1 AND entity_name = 'Blacksad'",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let stored_detail: serde_json::Value = row.get("detail");
    let json = &stored_detail["best_candidate"];
    assert_eq!(json["title"], "Blacksad");
    assert_eq!(json["external_id"], "ext_123");
    assert_eq!(json["external_url"], "https://example.com/blacksad");
    assert_eq!(json["authors"].as_array().unwrap().len(), 2);
    assert_eq!(json["description"], "A noir detective story.");
    assert_eq!(json["cover_url"], "https://example.com/cover.jpg");
    assert_eq!(json["total_volumes"], 7);
    assert_eq!(json["start_year"], 2000);
    assert_eq!(json["confidence"], 0.65);
}

/// Regression: LEFT JOIN with LOWER(unaccent()) resolves series_id even when
/// the event entity_name has different accents/casing than the series name.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_id_resolved_with_unaccent_in_results(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "test_unaccent").await;
    // Series with accented name
    let series_id = create_series(&pool, lib_id, "Asterix").await;

    let job_id = create_job(&pool, lib_id, "metadata_batch").await;

    // Insert event with entity_name WITHOUT accent
    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_name) \
         VALUES ($1, 'metadata_matched', 'info', 'series', 'Asterix')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    // Run the actual query from get_batch_results
    let rows = sqlx::query(
        r#"
        SELECT ije.id, ije.event_type, ije.entity_id, ije.entity_name, ije.message, ije.detail,
               s.id AS series_id
        FROM index_job_events ije
        LEFT JOIN series s ON s.library_id = $5 AND LOWER(unaccent(s.name)) = LOWER(unaccent(ije.entity_name))
        WHERE ije.job_id = $1
          AND (ije.event_type LIKE 'metadata_%' OR ije.event_type = 'error')
          AND ($2::text IS NULL OR ije.event_type = $2)
        ORDER BY ije.entity_name ASC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(job_id)
    .bind(None::<&str>)
    .bind(100i64)
    .bind(0i64)
    .bind(Some(lib_id))
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    let returned_series_id: Option<Uuid> = rows[0].get("series_id");
    assert_eq!(
        returned_series_id,
        Some(series_id),
        "series_id should be resolved despite accent difference (Asterix vs Asterix)"
    );
}

// -----------------------------------------------------------------------
// Report endpoint: GROUP BY event_type counts from index_job_events
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn report_counts_from_events(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "report_test").await;
    let job_id = create_job(&pool, lib_id, "metadata_batch").await;

    // Set total_files so the report has a total_series value
    sqlx::query("UPDATE index_jobs SET total_files = 10, processed_files = 8 WHERE id = $1")
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert events with different event_types
    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "metadata_matched",
        "info",
        Some("series"),
        Some("S1"),
        None,
        Some(serde_json::json!({"provider": "google_books"})),
    )
    .await;
    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "metadata_matched",
        "info",
        Some("series"),
        Some("S2"),
        None,
        Some(serde_json::json!({"provider": "google_books"})),
    )
    .await;
    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "metadata_no_results",
        "info",
        Some("series"),
        Some("S3"),
        Some("No results"),
        None,
    )
    .await;
    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "metadata_too_many",
        "warning",
        Some("series"),
        Some("S4"),
        Some("5 results"),
        None,
    )
    .await;
    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "metadata_low_confidence",
        "warning",
        Some("series"),
        Some("S5"),
        Some("Best: 40%"),
        None,
    )
    .await;
    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "metadata_already_linked",
        "info",
        Some("series"),
        Some("S6"),
        None,
        None,
    )
    .await;
    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "metadata_already_linked",
        "info",
        Some("series"),
        Some("S7"),
        None,
        None,
    )
    .await;
    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "error",
        "error",
        Some("series"),
        Some("S8"),
        Some("timeout"),
        None,
    )
    .await;

    // Run the same report query used in get_batch_report
    let counts = sqlx::query(
        "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
    )
    .bind(job_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    let mut auto_matched: i64 = 0;
    let mut no_results: i64 = 0;
    let mut too_many_results: i64 = 0;
    let mut low_confidence: i64 = 0;
    let mut already_linked: i64 = 0;
    let mut errors: i64 = 0;

    for row in &counts {
        let event_type: String = row.get("event_type");
        let cnt: i64 = row.get("cnt");
        match event_type.as_str() {
            "metadata_matched" => auto_matched = cnt,
            "metadata_no_results" => no_results = cnt,
            "metadata_too_many" => too_many_results = cnt,
            "metadata_low_confidence" => low_confidence = cnt,
            "metadata_already_linked" => already_linked = cnt,
            "error" => errors = cnt,
            _ => {}
        }
    }

    assert_eq!(auto_matched, 2, "metadata_matched -> auto_matched");
    assert_eq!(no_results, 1, "metadata_no_results -> no_results");
    assert_eq!(too_many_results, 1, "metadata_too_many -> too_many_results");
    assert_eq!(
        low_confidence, 1,
        "metadata_low_confidence -> low_confidence"
    );
    assert_eq!(
        already_linked, 2,
        "metadata_already_linked -> already_linked"
    );
    assert_eq!(errors, 1, "error -> errors");
}

// -----------------------------------------------------------------------
// Results endpoint: event detail JSON fields mapped to MetadataBatchResultDto
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn results_mapped_from_event_detail(pool: sqlx::PgPool) {
    let lib_id = create_lib(&pool, "results_test").await;
    let _series_id = create_series(&pool, lib_id, "TestSeries").await;
    let job_id = create_job(&pool, lib_id, "metadata_batch").await;

    let detail = serde_json::json!({
        "provider": "bedetheque",
        "fallback_used": true,
        "candidates_count": 3,
        "confidence": 0.87,
        "best_candidate": {
            "title": "Test Series",
            "external_id": "ext_999",
            "external_url": "https://example.com/test",
            "authors": ["Author A"],
            "description": "A test series.",
            "cover_url": "https://example.com/cover.jpg",
            "total_volumes": 12,
            "start_year": 2015,
            "confidence": 0.87,
        },
        "link_id": Uuid::new_v4().to_string(),
    });

    crate::job_helpers::insert_event(
        &pool,
        job_id,
        "metadata_matched",
        "info",
        Some("series"),
        Some("TestSeries"),
        None,
        Some(detail.clone()),
    )
    .await;

    // Run the actual results query used by get_batch_results
    let rows = sqlx::query(
        r#"
        SELECT ije.id, ije.event_type, ije.entity_id, ije.entity_name, ije.message, ije.detail,
               s.id AS series_id
        FROM index_job_events ije
        LEFT JOIN series s ON s.library_id = $5 AND LOWER(unaccent(s.name)) = LOWER(unaccent(ije.entity_name))
        WHERE ije.job_id = $1
          AND (ije.event_type LIKE 'metadata_%' OR ije.event_type = 'error')
          AND ($2::text IS NULL OR ije.event_type = $2)
        ORDER BY ije.entity_name ASC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(job_id)
    .bind(None::<&str>)
    .bind(100i64)
    .bind(0i64)
    .bind(Some(lib_id))
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);

    // Map the same way the endpoint does
    let row = &rows[0];
    let event_type: String = row.get("event_type");
    let row_detail: Option<serde_json::Value> = row.get("detail");

    let status = match event_type.as_str() {
        "metadata_matched" => "auto_matched",
        "metadata_no_results" => "no_results",
        "metadata_too_many" => "too_many_results",
        "metadata_low_confidence" => "low_confidence",
        "metadata_already_linked" => "already_linked",
        "error" => "error",
        other => other,
    };

    assert_eq!(status, "auto_matched");

    let d = row_detail.as_ref().unwrap();
    let provider_used = d["provider"].as_str();
    let fallback_used = d["fallback_used"].as_bool().unwrap_or(false);
    let candidates_count = d["candidates_count"].as_i64().unwrap_or(0) as i32;
    let best_confidence = d["confidence"].as_f64().map(|f| f as f32);
    let best_candidate = d.get("best_candidate");
    let link_id = d["link_id"].as_str().and_then(|s| s.parse::<Uuid>().ok());

    assert_eq!(provider_used, Some("bedetheque"));
    assert!(fallback_used);
    assert_eq!(candidates_count, 3);
    assert!((best_confidence.unwrap() - 0.87).abs() < 0.01);
    assert!(best_candidate.is_some());
    assert_eq!(best_candidate.unwrap()["title"], "Test Series");
    assert_eq!(best_candidate.unwrap()["external_id"], "ext_999");
    assert!(link_id.is_some());

    // series_id should be resolved via LEFT JOIN
    let returned_series_id: Option<Uuid> = row.get("series_id");
    assert!(
        returned_series_id.is_some(),
        "series_id should be resolved via LEFT JOIN"
    );
}

// boost_confidence_by_book_count tests moved to super::config::tests
