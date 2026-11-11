use sqlx::Row;
use uuid::Uuid;

use super::super::field_policy;
use super::super::shared_sync::LinkMetadata;

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

fn link(provider: &str, is_primary: bool, metadata_json: serde_json::Value) -> LinkMetadata {
    LinkMetadata {
        id: Uuid::new_v4(),
        is_primary,
        provider: provider.to_string(),
        external_id: format!("ext:{provider}"),
        metadata_json,
        total_volumes_external: None,
    }
}

async fn series_field<T>(pool: &sqlx::PgPool, series_id: Uuid, column: &str) -> T
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
{
    sqlx::query_scalar::<_, T>(sqlx::AssertSqlSafe(format!(
        "SELECT {column} FROM series WHERE id = $1"
    )))
    .bind(series_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// apply_series
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn apply_series_sets_cover_url_from_link(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_cover").await;
    let series_id = create_series(&pool, lib_id, "Cover Test").await;
    let links = vec![link(
        "google_books",
        true,
        serde_json::json!({"cover_url": "https://example.com/cover.jpg"}),
    )];

    let report = field_policy::apply_series(&pool, lib_id, "Cover Test", &links)
        .await
        .unwrap();

    assert!(
        report
            .fields_updated
            .iter()
            .any(|change| change.field == "cover_url"),
        "cover_url should be reported as updated"
    );
    let cover_url: Option<String> = series_field(&pool, series_id, "cover_url").await;
    assert_eq!(cover_url.as_deref(), Some("https://example.com/cover.jpg"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn apply_series_preserves_cover_url_when_link_has_none(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_cover_preserve").await;
    let series_id = create_series(&pool, lib_id, "Cover Preserve").await;
    sqlx::query("UPDATE series SET cover_url = $1 WHERE id = $2")
        .bind("https://example.com/original.jpg")
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    let links = vec![link("google_books", true, serde_json::json!({}))];
    field_policy::apply_series(&pool, lib_id, "Cover Preserve", &links)
        .await
        .unwrap();

    let cover_url: Option<String> = series_field(&pool, series_id, "cover_url").await;
    assert_eq!(
        cover_url.as_deref(),
        Some("https://example.com/original.jpg")
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn apply_series_overwrites_unlocked_description(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_description_refresh").await;
    let series_id = create_series(&pool, lib_id, "Description Refresh").await;
    sqlx::query("UPDATE series SET description = $1 WHERE id = $2")
        .bind("Previous description")
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    let links = vec![link(
        "google_books",
        true,
        serde_json::json!({"description": "Refreshed description"}),
    )];
    let report = field_policy::apply_series(&pool, lib_id, "Description Refresh", &links)
        .await
        .unwrap();

    let description: Option<String> = series_field(&pool, series_id, "description").await;
    assert_eq!(description.as_deref(), Some("Refreshed description"));
    let change = report
        .fields_updated
        .iter()
        .find(|change| change.field == "description")
        .expect("description should be reported as updated");
    assert_eq!(
        change.old_value,
        Some(serde_json::json!("Previous description"))
    );
    assert_eq!(
        change.new_value,
        Some(serde_json::json!("Refreshed description"))
    );
    assert!(report
        .fields_skipped
        .iter()
        .all(|change| change.field != "description"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn apply_series_preserves_locked_description_and_reports_it_as_skipped(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_description_locked").await;
    let series_id = create_series(&pool, lib_id, "Description Locked").await;
    sqlx::query("UPDATE series SET description = $1, locked_fields = $2 WHERE id = $3")
        .bind("Manual description")
        .bind(serde_json::json!({ "description": true }))
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    let links = vec![link(
        "google_books",
        true,
        serde_json::json!({"description": "Provider description"}),
    )];
    let report = field_policy::apply_series(&pool, lib_id, "Description Locked", &links)
        .await
        .unwrap();

    let description: Option<String> = series_field(&pool, series_id, "description").await;
    assert_eq!(description.as_deref(), Some("Manual description"));
    assert!(
        report
            .fields_skipped
            .iter()
            .any(|change| change.field == "description"),
        "locked description should be reported as skipped"
    );
    assert!(report
        .fields_updated
        .iter()
        .all(|change| change.field != "description"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn apply_series_seeds_genres_when_local_is_empty(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_genres_empty").await;
    let series_id = create_series(&pool, lib_id, "Genres Empty").await;
    let links = vec![link(
        "google_books",
        true,
        serde_json::json!({"genres": ["Action"]}),
    )];

    let report = field_policy::apply_series(&pool, lib_id, "Genres Empty", &links)
        .await
        .unwrap();

    let genres: Vec<String> = series_field(&pool, series_id, "genres").await;
    assert_eq!(genres, vec!["Action".to_string()]);
    assert!(
        report
            .fields_updated
            .iter()
            .any(|change| change.field == "genres"),
        "seeded genres should be reported"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn apply_series_never_overwrites_local_genres(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_genres_kept").await;
    let series_id = create_series(&pool, lib_id, "Genres Kept").await;
    sqlx::query("UPDATE series SET genres = ARRAY['Drame'], locked_fields = $1 WHERE id = $2")
        .bind(serde_json::json!({ "genres": true }))
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    let links = vec![link(
        "google_books",
        true,
        serde_json::json!({"genres": ["Action"]}),
    )];
    let report = field_policy::apply_series(&pool, lib_id, "Genres Kept", &links)
        .await
        .unwrap();

    let genres: Vec<String> = series_field(&pool, series_id, "genres").await;
    assert_eq!(genres, vec!["Drame".to_string()]);
    assert!(
        report
            .fields_updated
            .iter()
            .chain(report.fields_skipped.iter())
            .all(|change| change.field != "genres"),
        "local genres are the source of truth and are not reported"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn apply_series_merges_secondary_links_after_primary(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_merge").await;
    let series_id = create_series(&pool, lib_id, "Merge Test").await;
    let links = vec![
        link("bdtheque", false, serde_json::json!({"publishers": ["P"]})),
        link(
            "google_books",
            true,
            serde_json::json!({"description": "primary"}),
        ),
    ];

    field_policy::apply_series(&pool, lib_id, "Merge Test", &links)
        .await
        .unwrap();

    let description: Option<String> = series_field(&pool, series_id, "description").await;
    let publishers: Vec<String> = series_field(&pool, series_id, "publishers").await;
    assert_eq!(description.as_deref(), Some("primary"));
    assert_eq!(publishers, vec!["P".to_string()]);
}

// ---------------------------------------------------------------------------
// clear_series_metadata
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn clear_series_metadata_clears_unlocked_fields(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "clear_unlocked").await;
    let series_id = create_series(&pool, lib_id, "Clear Unlocked").await;
    sqlx::query(
        "UPDATE series SET description = 'd', authors = ARRAY['a'], publishers = ARRAY['p'], \
         start_year = 1999, total_volumes = 3, status = 'ongoing', genres = ARRAY['Drame'], \
         cover_url = 'http://cover' WHERE id = $1",
    )
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    field_policy::clear_series_metadata(&pool, series_id)
        .await
        .unwrap();

    let description: Option<String> = series_field(&pool, series_id, "description").await;
    let authors: Vec<String> = series_field(&pool, series_id, "authors").await;
    let publishers: Vec<String> = series_field(&pool, series_id, "publishers").await;
    let start_year: Option<i32> = series_field(&pool, series_id, "start_year").await;
    let total_volumes: Option<i32> = series_field(&pool, series_id, "total_volumes").await;
    let status: Option<String> = series_field(&pool, series_id, "status").await;
    let cover_url: Option<String> = series_field(&pool, series_id, "cover_url").await;
    let genres: Vec<String> = series_field(&pool, series_id, "genres").await;

    assert_eq!(description, None);
    assert!(authors.is_empty());
    assert!(publishers.is_empty());
    assert_eq!(start_year, None);
    assert_eq!(total_volumes, None);
    assert_eq!(status, None);
    assert_eq!(cover_url, None);
    assert_eq!(genres, vec!["Drame".to_string()], "genres are not cleared");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn clear_series_metadata_respects_locks(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "clear_locked").await;
    let series_id = create_series(&pool, lib_id, "Clear Locked").await;
    sqlx::query(
        "UPDATE series SET description = 'd', authors = ARRAY['a'], publishers = ARRAY['p'], \
         start_year = 1999, total_volumes = 3, status = 'ongoing', cover_url = 'http://cover', \
         locked_fields = '{\"description\": true, \"authors\": true, \"cover_url\": true}'::jsonb \
         WHERE id = $1",
    )
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    field_policy::clear_series_metadata(&pool, series_id)
        .await
        .unwrap();

    let description: Option<String> = series_field(&pool, series_id, "description").await;
    let authors: Vec<String> = series_field(&pool, series_id, "authors").await;
    let cover_url: Option<String> = series_field(&pool, series_id, "cover_url").await;
    let publishers: Vec<String> = series_field(&pool, series_id, "publishers").await;

    assert_eq!(description.as_deref(), Some("d"));
    assert_eq!(authors, vec!["a".to_string()]);
    assert_eq!(cover_url.as_deref(), Some("http://cover"));
    assert!(publishers.is_empty());
}

// ---------------------------------------------------------------------------
// apply_books
// ---------------------------------------------------------------------------

async fn seed_book_link(
    pool: &sqlx::PgPool,
    lib_id: Uuid,
    series_id: Uuid,
    book_id: Uuid,
    authors: &[&str],
) {
    let link_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO external_metadata_links \
         (id, library_id, series_id, provider, external_id, status, is_primary, approved_at) \
         VALUES ($1, $2, $3, 'google_books', 'ext:1', 'approved', true, NOW())",
    )
    .bind(link_id)
    .bind(lib_id)
    .bind(series_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO external_book_metadata \
         (link_id, book_id, external_book_id, volume_number, title, authors, isbn, summary, \
          publish_date, language) \
         VALUES ($1, $2, 'ext:1', 1, 'Gon T01', $3, '978-1', 'Provider summary', '2020-01-01', 'fr')",
    )
    .bind(link_id)
    .bind(book_id)
    .bind(authors.iter().map(|author| author.to_string()).collect::<Vec<_>>())
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_book(pool: &sqlx::PgPool, lib_id: Uuid, series_id: Uuid) -> Uuid {
    let book_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, series_id, title, kind, format, volume, authors) \
         VALUES ($1, $2, $3, 'Gon T01', 'comic', 'cbz', 1, '{}'::text[])",
    )
    .bind(book_id)
    .bind(lib_id)
    .bind(series_id)
    .execute(pool)
    .await
    .unwrap();
    book_id
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn apply_books_pushes_metadata_and_derives_author(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "book_push").await;
    let series_id = create_series(&pool, lib_id, "Book Push").await;
    let book_id = seed_book(&pool, lib_id, series_id).await;
    seed_book_link(&pool, lib_id, series_id, book_id, &["Alice", "Bob"]).await;

    let reports = field_policy::apply_books(&pool, series_id).await.unwrap();

    assert_eq!(reports.len(), 1);
    let row = sqlx::query("SELECT isbn, authors, author FROM books WHERE id = $1")
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.get::<Option<String>, _>("isbn").as_deref(),
        Some("978-1")
    );
    assert_eq!(
        row.get::<Vec<String>, _>("authors"),
        vec!["Alice".to_string(), "Bob".to_string()]
    );
    assert_eq!(
        row.get::<Option<String>, _>("author").as_deref(),
        Some("Alice")
    );
    assert!(reports[0]
        .fields_updated
        .iter()
        .any(|change| change.field == "authors"));
    assert!(
        reports[0]
            .fields_updated
            .iter()
            .all(|change| change.field != "author"),
        "the derived author column is not reported separately"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn apply_books_respects_locks_and_reports_skipped_fields(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "book_locked").await;
    let series_id = create_series(&pool, lib_id, "Book Locked").await;
    let book_id = seed_book(&pool, lib_id, series_id).await;
    sqlx::query(
        "UPDATE books SET summary = 'Manual summary', author = 'Manual', \
         locked_fields = '{\"summary\": true, \"authors\": true}'::jsonb WHERE id = $1",
    )
    .bind(book_id)
    .execute(&pool)
    .await
    .unwrap();
    seed_book_link(&pool, lib_id, series_id, book_id, &["Alice"]).await;

    let reports = field_policy::apply_books(&pool, series_id).await.unwrap();

    assert_eq!(reports.len(), 1);
    let skipped: Vec<&str> = reports[0]
        .fields_skipped
        .iter()
        .map(|change| change.field.as_str())
        .collect();
    assert!(skipped.contains(&"summary"));
    assert!(skipped.contains(&"authors"));

    let row = sqlx::query("SELECT summary, authors, author FROM books WHERE id = $1")
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.get::<Option<String>, _>("summary").as_deref(),
        Some("Manual summary")
    );
    assert!(row.get::<Vec<String>, _>("authors").is_empty());
    assert_eq!(
        row.get::<Option<String>, _>("author").as_deref(),
        Some("Manual")
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn apply_book_returns_none_when_nothing_changes(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "book_noop").await;
    let series_id = create_series(&pool, lib_id, "Book Noop").await;
    let book_id = seed_book(&pool, lib_id, series_id).await;

    let candidate = crate::metadata_providers::BookCandidate {
        external_book_id: "ext:1".to_string(),
        title: "Gon T01".to_string(),
        volume_number: Some(1),
        authors: vec![],
        isbn: None,
        summary: None,
        cover_url: None,
        page_count: None,
        language: None,
        publish_date: None,
        metadata_json: serde_json::json!({}),
    };

    let report = field_policy::apply_book(&pool, book_id, &candidate)
        .await
        .unwrap();
    assert!(report.is_none());
}

// ---------------------------------------------------------------------------
// Migration 0139: strict boolean locks
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn migration_0139_normalizes_legacy_lock_values(pool: sqlx::PgPool) {
    const MIGRATION: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../infra/migrations/0139_normalize_locked_fields.sql"
    ));

    let lib_id = create_test_library(&pool, "legacy_locks").await;
    let series_id = create_series(&pool, lib_id, "Legacy Locks").await;
    let invalid_series_id = create_series(&pool, lib_id, "Invalid Locks").await;
    let book_id = seed_book(&pool, lib_id, series_id).await;

    sqlx::query("UPDATE books SET locked_fields = $1 WHERE id = $2")
        .bind(serde_json::json!({
            "summary": "TRUE",
            "isbn": 1,
            "language": "no",
        }))
        .bind(book_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("UPDATE series SET locked_fields = $1 WHERE id = $2")
        .bind(serde_json::json!({
            "description": "true",
            "authors": 1,
            "status": "false",
            "cover_url": 0,
            "genres": "maybe",
        }))
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE series SET locked_fields = '[]'::jsonb WHERE id = $1")
        .bind(invalid_series_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();

    let locked: serde_json::Value =
        sqlx::query_scalar("SELECT locked_fields FROM series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        locked,
        serde_json::json!({
            "description": true,
            "authors": true,
            "status": false,
            "cover_url": false,
        }),
        "recognized legacy values become booleans, invalid ones are dropped"
    );

    let invalid: serde_json::Value =
        sqlx::query_scalar("SELECT locked_fields FROM series WHERE id = $1")
            .bind(invalid_series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(invalid, serde_json::json!({}));

    let book_locked: serde_json::Value =
        sqlx::query_scalar("SELECT locked_fields FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        book_locked,
        serde_json::json!({"summary": true, "isbn": true}),
        "book locks follow the same strict boolean normalization"
    );
}

/// Regression #112: a link stored with a discovery source id must be synced
/// through the provider that backs it, not rejected and not sent to Google
/// Books.
#[test]
fn resolve_link_provider_name_canonicalizes_discovery_sources() {
    assert_eq!(
        super::resolve_link_provider_name("senscritique_bd").as_deref(),
        Some("senscritique")
    );
    assert_eq!(
        super::resolve_link_provider_name("sc_best_manga").as_deref(),
        Some("senscritique")
    );
    assert_eq!(
        super::resolve_link_provider_name("anilist").as_deref(),
        Some("anilist")
    );
}

/// A link whose provider is not a metadata provider (e.g. a Prowlarr download)
/// keeps the historic Google Books fallback instead of erroring out.
#[test]
fn resolve_link_provider_name_falls_back_to_google_books() {
    assert_eq!(
        super::resolve_link_provider_name("prowlarr").as_deref(),
        Some("google_books")
    );
    assert_eq!(
        super::resolve_link_provider_name("").as_deref(),
        Some("google_books")
    );
}
