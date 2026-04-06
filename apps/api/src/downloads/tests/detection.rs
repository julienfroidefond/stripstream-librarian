use super::*;
use sqlx::Row;

#[sqlx::test(migrations = "../../infra/migrations")]
async fn failed_download_count_query(pool: sqlx::PgPool) {
    // Setup: library + series
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Test Lib', '/libraries/test')")
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

    // Insert an available_download for this series
    let ad_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO available_downloads (id, library_id, series_id, missing_count, available_releases, updated_at) \
         VALUES ($1, $2, $3, 5, '[]'::jsonb, NOW())",
    )
    .bind(ad_id)
    .bind(library_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    // Insert torrent_downloads with status='error' matching the series name (case-insensitive)
    sqlx::query(
        "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, status, error_message) \
         VALUES ($1, $2, 'naruto', '{1,2}', 'error', 'stalled')",
    )
    .bind(Uuid::new_v4())
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, status, error_message) \
         VALUES ($1, $2, 'Naruto', '{3}', 'error', 'timeout')",
    )
    .bind(Uuid::new_v4())
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    // Also insert a non-error torrent to verify it's NOT counted
    sqlx::query(
        "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, status) \
         VALUES ($1, $2, 'Naruto', '{4}', 'imported')",
    )
    .bind(Uuid::new_v4())
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    // Execute: run the same query used in get_latest_found
    let rows = sqlx::query(
        "SELECT ad.id, ad.library_id, s.name AS series_name, ad.series_id, ad.missing_count, ad.available_releases, ad.updated_at, \
                l.name as library_name, \
                COALESCE(td_err.failed_count, 0) AS failed_download_count \
         FROM available_downloads ad \
         JOIN libraries l ON l.id = ad.library_id \
         JOIN series s ON s.id = ad.series_id \
         LEFT JOIN LATERAL ( \
             SELECT COUNT(*) AS failed_count \
             FROM torrent_downloads td \
             WHERE td.library_id = ad.library_id \
               AND LOWER(td.series_name) = LOWER(s.name) \
               AND td.status = 'error' \
         ) td_err ON TRUE \
         ORDER BY l.name, s.name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    // Assert
    assert_eq!(rows.len(), 1, "should return one available_download row");
    let row = &rows[0];
    let failed_count: i64 = row.get("failed_download_count");
    assert_eq!(failed_count, 2, "should count only the 2 error torrent_downloads, not the imported one");
    let series_name: String = row.get("series_name");
    assert_eq!(series_name, "Naruto");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn has_failed_flag_on_releases(pool: sqlx::PgPool) {
    // Setup: library + series
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Test', '/libraries/test')")
        .bind(library_id).execute(&pool).await.unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Blacksad')")
        .bind(series_id).bind(library_id).execute(&pool).await.unwrap();

    // Two releases: one matching failed volumes, one not
    let releases = serde_json::json!([
        { "title": "Blacksad T01-T03", "size": 100, "seeders": 10, "matched_missing_volumes": [1, 2, 3], "all_volumes": [1, 2, 3] },
        { "title": "Blacksad T05", "size": 50, "seeders": 5, "matched_missing_volumes": [5], "all_volumes": [5] }
    ]);

    sqlx::query(
        "INSERT INTO available_downloads (id, library_id, series_id, missing_count, available_releases, updated_at) \
         VALUES ($1, $2, $3, 5, $4, NOW())",
    )
    .bind(Uuid::new_v4()).bind(library_id).bind(series_id).bind(&releases)
    .execute(&pool).await.unwrap();

    // Failed torrent had volumes 2, 3 — overlaps with release 1 but not release 2
    sqlx::query(
        "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, status, error_message) \
         VALUES ($1, $2, 'Blacksad', '{2,3}', 'error', 'stalled')",
    )
    .bind(Uuid::new_v4()).bind(library_id)
    .execute(&pool).await.unwrap();

    // Run the actual query from get_latest_found
    let rows = sqlx::query(
        "SELECT ad.available_releases, \
                COALESCE(td_err.failed_volumes, ARRAY[]::integer[]) AS failed_volumes \
         FROM available_downloads ad \
         JOIN series s ON s.id = ad.series_id \
         LEFT JOIN LATERAL ( \
             SELECT array_agg(DISTINCT vol) FILTER (WHERE vol IS NOT NULL) AS failed_volumes \
             FROM torrent_downloads td, unnest(td.expected_volumes) AS vol \
             WHERE td.library_id = ad.library_id \
               AND LOWER(td.series_name) = LOWER(s.name) \
               AND td.status = 'error' \
         ) td_err ON TRUE \
         WHERE ad.series_id = $1",
    )
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let failed_volumes: Vec<i32> = rows.get("failed_volumes");
    assert!(failed_volumes.contains(&2));
    assert!(failed_volumes.contains(&3));
    assert!(!failed_volumes.contains(&5), "volume 5 was not in a failed torrent");

    // Simulate the Rust-side enrichment
    let releases_json: serde_json::Value = rows.get("available_releases");
    let mut releases: Vec<AvailableReleaseDto> = serde_json::from_value(releases_json).unwrap();
    for r in &mut releases {
        r.has_failed = r.matched_missing_volumes.iter().any(|v| failed_volumes.contains(v));
    }

    assert!(releases[0].has_failed, "release T01-T03 should be flagged (volumes 2,3 overlap)");
    assert!(!releases[1].has_failed, "release T05 should NOT be flagged (volume 5 not failed)");
}

/// Regression: series_id must be returned from the detection results query.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_id_returned_when_series_exists(pool: sqlx::PgPool) {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestLib', '/libraries/test')")
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'OnePiece')")
        .bind(series_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'download_detection', 'success', NOW())",
    )
    .bind(job_id)
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_name, detail) \
         VALUES ($1, 'downloads_found', 'info', 'OnePiece', $2)",
    )
    .bind(job_id)
    .bind(serde_json::json!({"missing_count": 3}))
    .execute(&pool)
    .await
    .unwrap();

    // Run the actual query from get_detection_results (no status filter)
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
    assert_eq!(returned_series_id, Some(series_id), "series_id should match the created series");
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
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Ast\u{00e9}rix')")
        .bind(series_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'download_detection', 'success', NOW())",
    )
    .bind(job_id)
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    // Insert event with entity_name WITHOUT accent
    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_name) \
         VALUES ($1, 'downloads_found', 'info', 'Asterix')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    // Run the actual query from get_detection_results
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
        "series_id should be resolved despite accent difference (Ast\u{00e9}rix vs Asterix)"
    );
}

// -----------------------------------------------------------------------
// insert_event tests
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn event_downloads_found_written(pool: sqlx::PgPool) {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'DlEvtLib', '/libraries/dl_evt')")
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'download_detection', 'running', NOW())",
    )
    .bind(job_id)
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    let detail = serde_json::json!({"release_count": 3, "missing_count": 5});
    super::insert_event(&pool, job_id, "downloads_found", "info", Some("Naruto"), None, Some(detail.clone())).await;

    let row = sqlx::query(
        "SELECT event_type, level, entity_name, message, detail FROM index_job_events WHERE job_id = $1",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.get::<String, _>("event_type"), "downloads_found");
    assert_eq!(row.get::<String, _>("level"), "info");
    assert_eq!(row.get::<Option<String>, _>("entity_name"), Some("Naruto".to_string()));
    let stored_detail: serde_json::Value = row.get("detail");
    assert_eq!(stored_detail["release_count"], 3);
    assert_eq!(stored_detail["missing_count"], 5);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn event_error_has_error_level(pool: sqlx::PgPool) {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'DlEvtErrLib', '/libraries/dl_evt_err')")
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'download_detection', 'running', NOW())",
    )
    .bind(job_id)
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    super::insert_event(&pool, job_id, "error", "error", Some("FailedSeries"), Some("search timeout"), None).await;

    let row = sqlx::query(
        "SELECT event_type, level, entity_name, message FROM index_job_events WHERE job_id = $1",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.get::<String, _>("event_type"), "error");
    assert_eq!(row.get::<String, _>("level"), "error");
    assert_eq!(row.get::<Option<String>, _>("entity_name"), Some("FailedSeries".to_string()));
    assert_eq!(row.get::<Option<String>, _>("message"), Some("search timeout".to_string()));
}

/// Regression: series_id should be None when no matching series exists.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_id_none_when_series_missing(pool: sqlx::PgPool) {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestLib', '/libraries/test')")
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'download_detection', 'success', NOW())",
    )
    .bind(job_id)
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    // Insert event with entity_name that does NOT exist in series table
    sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_name) \
         VALUES ($1, 'downloads_not_found', 'info', 'NonExistentSeries')",
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
    assert!(returned_series_id.is_none(), "series_id should be None when no series is linked");
}

// -----------------------------------------------------------------------
// Report endpoint: GROUP BY event_type counts from index_job_events
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn report_counts_from_events(pool: sqlx::PgPool) {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'DlReportLib', '/libraries/dl_report')")
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at, total_files) VALUES ($1, $2, 'download_detection', 'success', NOW(), 8)",
    )
    .bind(job_id)
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    // Insert events with different event_types
    super::insert_event(&pool, job_id, "downloads_found", "info", Some("Naruto"), None, Some(serde_json::json!({"missing_count": 3}))).await;
    super::insert_event(&pool, job_id, "downloads_found", "info", Some("Bleach"), None, Some(serde_json::json!({"missing_count": 1}))).await;
    super::insert_event(&pool, job_id, "downloads_not_found", "info", Some("Obscure"), None, None).await;
    super::insert_event(&pool, job_id, "no_missing_volumes", "info", Some("Complete"), None, None).await;
    super::insert_event(&pool, job_id, "no_missing_volumes", "info", Some("Complete2"), None, None).await;
    super::insert_event(&pool, job_id, "no_missing_volumes", "info", Some("Complete3"), None, None).await;
    super::insert_event(&pool, job_id, "no_metadata_link", "info", Some("Unlinked"), None, None).await;
    super::insert_event(&pool, job_id, "error", "error", Some("BrokenSeries"), Some("search timeout"), None).await;

    // Run the same report query used in get_detection_report
    let counts = sqlx::query(
        "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
    )
    .bind(job_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    let mut found = 0i64;
    let mut not_found = 0i64;
    let mut no_missing = 0i64;
    let mut no_metadata = 0i64;
    let mut errors = 0i64;

    for r in &counts {
        let event_type: String = r.get("event_type");
        let cnt: i64 = r.get("cnt");
        match event_type.as_str() {
            "downloads_found" => found = cnt,
            "downloads_not_found" => not_found = cnt,
            "no_missing_volumes" => no_missing = cnt,
            "no_metadata_link" => no_metadata = cnt,
            "error" => errors = cnt,
            _ => {}
        }
    }

    assert_eq!(found, 2, "downloads_found -> found");
    assert_eq!(not_found, 1, "downloads_not_found -> not_found");
    assert_eq!(no_missing, 3, "no_missing_volumes -> no_missing");
    assert_eq!(no_metadata, 1, "no_metadata_link -> no_metadata");
    assert_eq!(errors, 1, "error -> errors");
}

#[test]
fn filter_volume_zero_from_missing() {
    let raw_volumes: Vec<Option<i32>> = vec![Some(0), Some(1), Some(2), None, Some(3)];
    let missing: Vec<i32> = raw_volumes
        .iter()
        .filter_map(|v| *v)
        .filter(|&v| v > 0)
        .collect();
    assert_eq!(missing, vec![1, 2, 3], "volume 0 and NULL should be excluded");
}
