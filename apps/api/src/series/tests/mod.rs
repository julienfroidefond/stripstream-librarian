mod create;
mod ratings;
mod related;

use super::helpers::get_or_create_series;
use super::update::UpdateSeriesResponse;
use super::*;

#[test]
fn series_item_has_series_id() {
    let item = SeriesItem {
        name: "Dragon Ball".to_string(),
        series_id: Uuid::new_v4(),
        book_count: 42,
        books_read_count: 10,
        first_book_id: Some(Uuid::new_v4()),
        first_book_updated_at: None,
        library_id: Uuid::new_v4(),
        series_status: Some("ended".to_string()),
        missing_count: Some(0),
        metadata_provider: None,
        anilist_id: None,
        anilist_url: None,
        cover_url: None,
        start_year: Some(1984),
        genres: vec![],
        authors: vec![],
        description: None,
    };
    let json = serde_json::to_value(&item).unwrap();
    assert!(json["series_id"].is_string());
    assert_eq!(json["name"], "Dragon Ball");
    assert_eq!(json["book_count"], 42);
    assert_eq!(json["start_year"], 1984);
}

#[test]
fn series_metadata_serializes() {
    let meta = SeriesMetadata {
        series_name: "Naruto".to_string(),
        description: Some("A ninja story".to_string()),
        authors: vec!["Kishimoto".to_string()],
        genres: vec![],
        publishers: vec![],
        book_author: None,
        book_language: None,
        start_year: Some(1999),
        total_volumes: Some(72),
        status: Some("ended".to_string()),
        locked_fields: serde_json::json!({}),
    };
    let json = serde_json::to_value(&meta).unwrap();
    assert_eq!(json["total_volumes"], 72);
    assert_eq!(json["authors"][0], "Kishimoto");
    assert_eq!(json["status"], "ended");
}

#[test]
fn update_series_response_serializes() {
    let resp = UpdateSeriesResponse { updated: 5 };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["updated"], 5);
}

#[test]
fn series_item_includes_library_id() {
    let lib_id = Uuid::new_v4();
    let item = SeriesItem {
        name: "One Piece".to_string(),
        series_id: Uuid::new_v4(),
        book_count: 100,
        books_read_count: 50,
        first_book_id: Some(Uuid::new_v4()),
        first_book_updated_at: None,
        library_id: lib_id,
        series_status: Some("ongoing".to_string()),
        missing_count: Some(5),
        metadata_provider: Some("google_books".to_string()),
        anilist_id: Some(12345),
        anilist_url: Some("https://anilist.co/manga/12345".to_string()),
        cover_url: None,
        start_year: Some(1997),
        genres: vec![],
        authors: vec![],
        description: None,
    };
    let json = serde_json::to_value(&item).unwrap();
    assert_eq!(json["library_id"], lib_id.to_string());
    assert_eq!(json["series_status"], "ongoing");
    assert_eq!(json["missing_count"], 5);
    assert_eq!(json["metadata_provider"], "google_books");
    assert_eq!(json["anilist_id"], 12345);
}

// ─── Integration tests (require PostgreSQL) ─────────────────────────

/// Helper to create a test library in the DB.
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
async fn get_or_create_series_new(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "test").await;
    let id = get_or_create_series(&pool, lib_id, "Dragon Ball")
        .await
        .unwrap();
    assert_ne!(id, Uuid::nil());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_or_create_series_idempotent(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "test").await;
    let id1 = get_or_create_series(&pool, lib_id, "Dragon Ball")
        .await
        .unwrap();
    let id2 = get_or_create_series(&pool, lib_id, "Dragon Ball")
        .await
        .unwrap();
    assert_eq!(id1, id2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_or_create_series_case_insensitive(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "test").await;
    let id1 = get_or_create_series(&pool, lib_id, "Dragon Ball")
        .await
        .unwrap();
    let id2 = get_or_create_series(&pool, lib_id, "dragon ball")
        .await
        .unwrap();
    assert_eq!(
        id1, id2,
        "same series with different casing should return same id"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_or_create_series_accent_insensitive(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "test").await;
    let id1 = get_or_create_series(&pool, lib_id, "Astérix")
        .await
        .unwrap();
    let id2 = get_or_create_series(&pool, lib_id, "Asterix")
        .await
        .unwrap();
    assert_eq!(id1, id2, "accented and unaccented names should match");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_or_create_series_different_libraries(pool: sqlx::PgPool) {
    let lib1 = create_test_library(&pool, "lib1").await;
    let lib2 = create_test_library(&pool, "lib2").await;
    let id1 = get_or_create_series(&pool, lib1, "Naruto").await.unwrap();
    let id2 = get_or_create_series(&pool, lib2, "Naruto").await.unwrap();
    assert_ne!(
        id1, id2,
        "same name in different libraries should be different series"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_or_create_series_finds_by_original_name(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "rename_test").await;

    let id1 = get_or_create_series(&pool, lib_id, "Dragon Ball")
        .await
        .unwrap();
    sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
        .bind("Dragon Ball Z")
        .bind("Dragon Ball")
        .bind(id1)
        .execute(&pool)
        .await
        .unwrap();

    let id2 = get_or_create_series(&pool, lib_id, "Dragon Ball")
        .await
        .unwrap();
    assert_eq!(
        id1, id2,
        "lookup by original_name should return the renamed series"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_or_create_series_original_name_case_insensitive(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "rename_case_test").await;

    let id1 = get_or_create_series(&pool, lib_id, "LES MYTHICS")
        .await
        .unwrap();
    sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
        .bind("Mythics")
        .bind("LES MYTHICS")
        .bind(id1)
        .execute(&pool)
        .await
        .unwrap();

    let id2 = get_or_create_series(&pool, lib_id, "les mythics")
        .await
        .unwrap();
    assert_eq!(id1, id2, "original_name lookup should be case-insensitive");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_or_create_series_chained_rename(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "chained_rename").await;

    let id1 = get_or_create_series(&pool, lib_id, "Series A")
        .await
        .unwrap();
    sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
        .bind("Series C")
        .bind("Series A")
        .bind(id1)
        .execute(&pool)
        .await
        .unwrap();

    let id2 = get_or_create_series(&pool, lib_id, "Series A")
        .await
        .unwrap();
    assert_eq!(id1, id2, "chained rename: original_name should still match");

    let id3 = get_or_create_series(&pool, lib_id, "Series C")
        .await
        .unwrap();
    assert_eq!(id1, id3, "current name should also match");
}

// ─── Merge series tests ────────────────────────────────────────────

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

async fn create_book(pool: &sqlx::PgPool, lib_id: Uuid, series_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO books (id, library_id, title, kind, format, series_id) \
         VALUES (gen_random_uuid(), $1, $2, 'comic', 'cbz', $3) RETURNING id",
    )
    .bind(lib_id)
    .bind(title)
    .bind(series_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_book_with_type(
    pool: &sqlx::PgPool,
    lib_id: Uuid,
    series_id: Uuid,
    title: &str,
    volume: Option<i32>,
    volume_type: &str,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO books (id, library_id, title, kind, format, series_id, volume, volume_type) \
         VALUES (gen_random_uuid(), $1, $2, 'comic', 'cbz', $3, $4, $5) RETURNING id",
    )
    .bind(lib_id)
    .bind(title)
    .bind(series_id)
    .bind(volume)
    .bind(volume_type)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn merge_moves_books_to_target(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "merge_books").await;
    let target = create_series(&pool, lib_id, "Target").await;
    let source = create_series(&pool, lib_id, "Source").await;

    create_book(&pool, lib_id, target, "Book A").await;
    create_book(&pool, lib_id, source, "Book B").await;
    create_book(&pool, lib_id, source, "Book C").await;

    let moved = sqlx::query("UPDATE books SET series_id = $1 WHERE series_id = $2")
        .bind(target)
        .bind(source)
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(moved, 2);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = $1")
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn merge_moves_metadata_links(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "merge_meta").await;
    let target = create_series(&pool, lib_id, "Target").await;
    let source = create_series(&pool, lib_id, "Source").await;

    sqlx::query(
        "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id) \
         VALUES ($1, $2, 'senscritique', '123')",
    )
    .bind(lib_id)
    .bind(source)
    .execute(&pool)
    .await
    .unwrap();

    let moved = sqlx::query(
        "UPDATE external_metadata_links SET series_id = $1 \
         WHERE series_id = $2 \
         AND provider NOT IN (SELECT provider FROM external_metadata_links WHERE series_id = $1)",
    )
    .bind(target)
    .bind(source)
    .execute(&pool)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(moved, 1);

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1")
            .bind(target)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn merge_keeps_target_metadata_on_conflict(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "merge_meta_conflict").await;
    let target = create_series(&pool, lib_id, "Target").await;
    let source = create_series(&pool, lib_id, "Source").await;

    for (sid, ext_id) in [(target, "target_123"), (source, "source_456")] {
        sqlx::query(
            "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id) \
             VALUES ($1, $2, 'senscritique', $3)",
        )
        .bind(lib_id)
        .bind(sid)
        .bind(ext_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let moved = sqlx::query(
        "UPDATE external_metadata_links SET series_id = $1 \
         WHERE series_id = $2 \
         AND provider NOT IN (SELECT provider FROM external_metadata_links WHERE series_id = $1)",
    )
    .bind(target)
    .bind(source)
    .execute(&pool)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(
        moved, 0,
        "source link should NOT be moved (target already has senscritique)"
    );

    sqlx::query("DELETE FROM external_metadata_links WHERE series_id = $1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();

    let ext_id: String = sqlx::query_scalar(
        "SELECT external_id FROM external_metadata_links WHERE series_id = $1 AND provider = 'senscritique'",
    )
    .bind(target).fetch_one(&pool).await.unwrap();
    assert_eq!(
        ext_id, "target_123",
        "target's original link should be preserved"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn merge_deletes_source_series(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "merge_delete").await;
    let target = create_series(&pool, lib_id, "Target").await;
    let source = create_series(&pool, lib_id, "Source").await;

    sqlx::query("UPDATE books SET series_id = $1 WHERE series_id = $2")
        .bind(target)
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("DELETE FROM series WHERE id = $1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
        .bind(source)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!exists, "source series should be deleted");

    let target_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
            .bind(target)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(target_exists, "target series should still exist");
}

// ─── Missing count tests ───────────────────────────────────────────

async fn query_missing_count(pool: &sqlx::PgPool, lib_id: Uuid, series_id: Uuid) -> i64 {
    sqlx::query_scalar::<_, i64>(
        &format!(
            "WITH {} SELECT COALESCE(mc.missing_count, 0) FROM missing_counts mc WHERE mc.series_id = $2",
            helpers::build_missing_counts_cte(Some("$1"))
        ),
    )
    .bind(lib_id)
    .bind(series_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_with_total_volumes_and_books(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_test").await;
    let sid = create_series(&pool, lib_id, "Naruto").await;

    sqlx::query("UPDATE series SET total_volumes = 10 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    for i in 1..=7 {
        create_book(&pool, lib_id, sid, &format!("Vol {i}")).await;
    }

    let missing = query_missing_count(&pool, lib_id, sid).await;
    assert_eq!(missing, 3, "10 total - 7 books = 3 missing");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_zero_when_no_total_volumes(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_null").await;
    let sid = create_series(&pool, lib_id, "Unknown").await;

    for i in 1..=5 {
        create_book(&pool, lib_id, sid, &format!("Vol {i}")).await;
    }

    let missing = query_missing_count(&pool, lib_id, sid).await;
    assert_eq!(missing, 0, "NULL total_volumes => 0 missing");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_zero_when_complete(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_complete").await;
    let sid = create_series(&pool, lib_id, "Complete").await;

    sqlx::query("UPDATE series SET total_volumes = 3 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    for i in 1..=3 {
        create_book(&pool, lib_id, sid, &format!("Vol {i}")).await;
    }

    let missing = query_missing_count(&pool, lib_id, sid).await;
    assert_eq!(missing, 0, "3 total - 3 books = 0 missing");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_zero_when_more_books_than_total(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_over").await;
    let sid = create_series(&pool, lib_id, "Overflow").await;

    sqlx::query("UPDATE series SET total_volumes = 2 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    for i in 1..=5 {
        create_book(&pool, lib_id, sid, &format!("Vol {i}")).await;
    }

    let missing = query_missing_count(&pool, lib_id, sid).await;
    assert_eq!(missing, 0, "GREATEST(2 - 5, 0) = 0, not negative");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_updates_after_manual_edit(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_edit").await;
    let sid = create_series(&pool, lib_id, "Edited").await;

    sqlx::query("UPDATE series SET total_volumes = 5 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    create_book(&pool, lib_id, sid, "Vol 1").await;
    create_book(&pool, lib_id, sid, "Vol 2").await;

    assert_eq!(
        query_missing_count(&pool, lib_id, sid).await,
        3,
        "5 - 2 = 3"
    );

    sqlx::query("UPDATE series SET total_volumes = 10 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        query_missing_count(&pool, lib_id, sid).await,
        8,
        "10 - 2 = 8"
    );
}

/// Regression: series with zero books should appear in list (LEFT JOIN, not INNER JOIN).
#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_includes_series_with_zero_books(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "zero_books").await;
    let series_id = create_series(&pool, lib_id, "Empty Series").await;

    // Do NOT insert any books for this series.
    // Run the series_counts CTE query (same LEFT JOIN as list.rs).
    let row = sqlx::query(
        r#"
        WITH series_counts AS (
            SELECT s.id as series_id, s.name,
                COUNT(b.id) as book_count,
                0::bigint as books_read_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            WHERE s.library_id = $1
            GROUP BY s.id, s.name
        )
        SELECT sc.series_id, sc.name, sc.book_count
        FROM series_counts sc
        WHERE sc.series_id = $2
        "#,
    )
    .bind(lib_id)
    .bind(series_id)
    .fetch_optional(&pool)
    .await
    .unwrap();

    assert!(
        row.is_some(),
        "series with zero books should appear in series_counts CTE"
    );
    let row = row.unwrap();
    let book_count: i64 = row.get("book_count");
    assert_eq!(
        book_count, 0,
        "book_count should be 0 for a series with no books"
    );
    let name: String = row.get("name");
    assert_eq!(name, "Empty Series");
}

// ─── Volume type tests ────────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_excludes_hs_books(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_hs").await;
    let sid = create_series(&pool, lib_id, "Boruto").await;

    sqlx::query("UPDATE series SET total_volumes = 5 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    // 3 regular + 1 HS = 4 books total, but only 3 count toward missing
    create_book_with_type(&pool, lib_id, sid, "Vol 1", Some(1), "regular").await;
    create_book_with_type(&pool, lib_id, sid, "Vol 2", Some(2), "regular").await;
    create_book_with_type(&pool, lib_id, sid, "Vol 3", Some(3), "regular").await;
    create_book_with_type(&pool, lib_id, sid, "HS 1", Some(1), "hs").await;

    let missing = query_missing_count(&pool, lib_id, sid).await;
    assert_eq!(missing, 2, "5 total - 3 regular = 2 missing (HS excluded)");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_excludes_oneshot_books(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_oneshot").await;
    let sid = create_series(&pool, lib_id, "Mixed").await;

    sqlx::query("UPDATE series SET total_volumes = 3 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    create_book_with_type(&pool, lib_id, sid, "Vol 1", Some(1), "regular").await;
    create_book_with_type(&pool, lib_id, sid, "Vol 2", Some(2), "regular").await;
    create_book_with_type(&pool, lib_id, sid, "One-shot", None, "oneshot").await;

    let missing = query_missing_count(&pool, lib_id, sid).await;
    assert_eq!(
        missing, 1,
        "3 total - 2 regular = 1 missing (oneshot excluded)"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_zero_with_hs_when_complete(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_hs_complete").await;
    let sid = create_series(&pool, lib_id, "Complete+HS").await;

    sqlx::query("UPDATE series SET total_volumes = 3 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    // 3 regular (complete) + 2 HS (bonus)
    for i in 1..=3 {
        create_book_with_type(&pool, lib_id, sid, &format!("Vol {i}"), Some(i), "regular").await;
    }
    create_book_with_type(&pool, lib_id, sid, "HS 1", Some(1), "hs").await;
    create_book_with_type(&pool, lib_id, sid, "HS 2", Some(2), "hs").await;

    let missing = query_missing_count(&pool, lib_id, sid).await;
    assert_eq!(
        missing, 0,
        "3 total - 3 regular = 0 missing (HS don't inflate)"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_zero_when_integral_present(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_int").await;
    let sid = create_series(&pool, lib_id, "La Rivière").await;

    sqlx::query("UPDATE series SET total_volumes = 2 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    // Only an intégrale — covers the whole series
    create_book_with_type(&pool, lib_id, sid, "La Rivière INT", None, "integral").await;

    let missing = query_missing_count(&pool, lib_id, sid).await;
    assert_eq!(missing, 0, "integral present → series complete → 0 missing");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_zero_when_integral_plus_regular(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_int_reg").await;
    let sid = create_series(&pool, lib_id, "Mixed INT").await;

    sqlx::query("UPDATE series SET total_volumes = 10 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    // 2 regular + 1 integral → integral makes it complete
    create_book_with_type(&pool, lib_id, sid, "Vol 1", Some(1), "regular").await;
    create_book_with_type(&pool, lib_id, sid, "Vol 2", Some(2), "regular").await;
    create_book_with_type(&pool, lib_id, sid, "INT 1", Some(1), "integral").await;

    let missing = query_missing_count(&pool, lib_id, sid).await;
    assert_eq!(
        missing, 0,
        "integral present → 0 missing regardless of total_volumes"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_count_normal_without_integral(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "missing_no_int").await;
    let sid = create_series(&pool, lib_id, "No INT").await;

    sqlx::query("UPDATE series SET total_volumes = 5 WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

    // 3 regular, no integral → normal missing count
    for i in 1..=3 {
        create_book_with_type(&pool, lib_id, sid, &format!("Vol {i}"), Some(i), "regular").await;
    }

    let missing = query_missing_count(&pool, lib_id, sid).await;
    assert_eq!(missing, 2, "5 total - 3 regular = 2 missing (no integral)");
}
