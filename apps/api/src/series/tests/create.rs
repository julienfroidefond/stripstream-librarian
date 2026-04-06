use sqlx::Row;
use uuid::Uuid;

async fn create_test_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
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

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_series_without_metadata(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "create_test").await;

    let series_id = super::helpers::get_or_create_series(&pool, lib_id, "New Series")
        .await
        .unwrap();

    // Series exists
    let name: String =
        sqlx::query_scalar("SELECT name FROM series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(name, "New Series");

    // No metadata link
    let link_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1",
    )
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(link_count, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_series_with_metadata_link(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "create_meta").await;

    let series_id = super::helpers::get_or_create_series(&pool, lib_id, "Naruto")
        .await
        .unwrap();

    // Create approved metadata link
    let link_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO external_metadata_links
            (library_id, series_id, provider, external_id, external_url, status, confidence, metadata_json, total_volumes_external)
        VALUES ($1, $2, 'senscritique', 'franchise:817:edition:test', 'https://senscritique.com/test', 'approved', 0.95, '{}', 72)
        RETURNING id
        "#,
    )
    .bind(lib_id)
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Verify link exists and is approved
    let status: String = sqlx::query_scalar(
        "SELECT status FROM external_metadata_links WHERE id = $1",
    )
    .bind(link_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "approved");

    // Verify link is attached to the series
    let link_series_id: Uuid = sqlx::query_scalar(
        "SELECT series_id FROM external_metadata_links WHERE id = $1",
    )
    .bind(link_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(link_series_id, series_id);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_series_metadata_updates_series_fields(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "create_sync").await;

    let series_id = super::helpers::get_or_create_series(&pool, lib_id, "Test Series")
        .await
        .unwrap();

    // Simulate syncing metadata to the series
    let metadata_json = serde_json::json!({
        "description": "A great manga",
        "authors": ["Author A", "Author B"],
        "publishers": ["Publisher X"],
        "start_year": 2000,
        "status": "ongoing",
        "genres": ["manga", "action"],
        "cover_url": "https://example.com/cover.jpg"
    });

    let description = metadata_json.get("description").and_then(|d| d.as_str()).map(String::from);
    let authors: Vec<String> = metadata_json.get("authors").and_then(|a| a.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let publishers: Vec<String> = metadata_json.get("publishers").and_then(|a| a.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    sqlx::query(
        r#"
        UPDATE series SET
            description = COALESCE(NULLIF($2, ''), description),
            authors = CASE WHEN array_length($3::text[], 1) > 0 THEN $3 ELSE authors END,
            publishers = CASE WHEN array_length($4::text[], 1) > 0 THEN $4 ELSE publishers END,
            start_year = COALESCE($5, start_year),
            total_volumes = COALESCE($6, total_volumes),
            status = COALESCE($7, status),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(series_id)
    .bind(&description)
    .bind(&authors)
    .bind(&publishers)
    .bind(2000_i32)
    .bind(72_i32)
    .bind("ongoing")
    .execute(&pool)
    .await
    .unwrap();

    // Verify fields were set
    let row = sqlx::query("SELECT description, authors, publishers, start_year, total_volumes, status FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let db_desc: Option<String> = row.get("description");
    let db_authors: Vec<String> = row.get("authors");
    let db_total: Option<i32> = row.get("total_volumes");
    let db_status: Option<String> = row.get("status");

    assert_eq!(db_desc.as_deref(), Some("A great manga"));
    assert_eq!(db_authors, vec!["Author A", "Author B"]);
    assert_eq!(db_total, Some(72));
    assert_eq!(db_status.as_deref(), Some("ongoing"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_series_idempotent(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "create_idempotent").await;

    let id1 = super::helpers::get_or_create_series(&pool, lib_id, "Same Series")
        .await
        .unwrap();
    let id2 = super::helpers::get_or_create_series(&pool, lib_id, "Same Series")
        .await
        .unwrap();

    assert_eq!(id1, id2, "creating the same series twice should return the same ID");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_series_metadata_link_upsert(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "create_upsert").await;

    let series_id = super::helpers::get_or_create_series(&pool, lib_id, "Upsert Test")
        .await
        .unwrap();

    // First link
    sqlx::query(
        "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id, status) \
         VALUES ($1, $2, 'senscritique', 'old_id', 'approved')",
    )
    .bind(lib_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    // Upsert with new external_id
    sqlx::query(
        r#"
        INSERT INTO external_metadata_links
            (library_id, series_id, provider, external_id, status)
        VALUES ($1, $2, 'senscritique', 'new_id', 'approved')
        ON CONFLICT (series_id, provider)
        DO UPDATE SET external_id = EXCLUDED.external_id, updated_at = NOW()
        "#,
    )
    .bind(lib_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    // Should have only 1 link with the new external_id
    let ext_id: String = sqlx::query_scalar(
        "SELECT external_id FROM external_metadata_links WHERE series_id = $1 AND provider = 'senscritique'",
    )
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ext_id, "new_id");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1",
    )
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "upsert should not create duplicate links");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_series_with_metadata_sets_cover_url(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "create_cover").await;

    let series_id = super::helpers::get_or_create_series(&pool, lib_id, "Cover Series")
        .await
        .unwrap();

    // Run the same INSERT...ON CONFLICT sync SQL that sync_series_metadata uses,
    // with cover_url set in the metadata.
    sqlx::query(
        r#"
        INSERT INTO series (id, library_id, name, description, publishers, start_year, total_volumes, status, authors, genres, cover_url, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW())
        ON CONFLICT (library_id, name)
        DO UPDATE SET
            description = COALESCE(NULLIF(EXCLUDED.description, ''), series.description),
            publishers = CASE WHEN array_length(EXCLUDED.publishers, 1) > 0 THEN EXCLUDED.publishers ELSE series.publishers END,
            start_year = COALESCE(EXCLUDED.start_year, series.start_year),
            total_volumes = COALESCE(EXCLUDED.total_volumes, series.total_volumes),
            status = COALESCE(EXCLUDED.status, series.status),
            authors = CASE WHEN array_length(EXCLUDED.authors, 1) > 0 THEN EXCLUDED.authors ELSE series.authors END,
            genres = CASE WHEN array_length(EXCLUDED.genres, 1) > 0 THEN EXCLUDED.genres ELSE series.genres END,
            cover_url = COALESCE(NULLIF(EXCLUDED.cover_url, ''), series.cover_url),
            updated_at = NOW()
        "#,
    )
    .bind(lib_id)
    .bind("Cover Series")
    .bind("A manga about covers")
    .bind(&Vec::<String>::new())
    .bind(Some(2021_i32))
    .bind(Some(5_i32))
    .bind(Some("ongoing"))
    .bind(&vec!["Author X".to_string()])
    .bind(&vec!["manga".to_string()])
    .bind(Some("https://example.com/series_cover.jpg"))
    .execute(&pool)
    .await
    .unwrap();

    let row = sqlx::query("SELECT cover_url FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let cover_url: Option<String> = row.get("cover_url");
    assert_eq!(
        cover_url,
        Some("https://example.com/series_cover.jpg".to_string()),
        "cover_url should be set after create_series + metadata sync"
    );
}
