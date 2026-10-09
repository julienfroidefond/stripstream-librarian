use super::*;
use sqlx::Row;

async fn create_test_user(pool: &sqlx::PgPool, username: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (id, username) VALUES (gen_random_uuid(), $1) RETURNING id",
    )
    .bind(username)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn block_genre_for_user(pool: &sqlx::PgPool, user_id: Uuid, genre: &str) {
    sqlx::query("INSERT INTO user_genre_restrictions (user_id, genre) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(user_id)
        .bind(genre)
        .execute(pool)
        .await
        .unwrap();
}

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

async fn create_test_series(pool: &sqlx::PgPool, library_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, created_at, updated_at) VALUES (gen_random_uuid(), $1, $2, NOW(), NOW()) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_test_series_with_genres(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    name: &str,
    genres: &[&str],
) -> Uuid {
    let genres_vec: Vec<String> = genres.iter().map(|g| g.to_string()).collect();
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, genres, created_at, updated_at) VALUES (gen_random_uuid(), $1, $2, $3, NOW(), NOW()) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .bind(&genres_vec)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_test_series_with_authors(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    name: &str,
    authors: &[&str],
) -> Uuid {
    let authors_vec: Vec<String> = authors.iter().map(|a| a.to_string()).collect();
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, authors, created_at, updated_at) VALUES (gen_random_uuid(), $1, $2, $3, NOW(), NOW()) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .bind(&authors_vec)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_test_book(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    series_id: Option<Uuid>,
    title: &str,
    kind: &str,
    author: Option<&str>,
    authors: &[&str],
) -> Uuid {
    let id = Uuid::new_v4();
    let authors_vec: Vec<String> = authors.iter().map(|a| a.to_string()).collect();
    sqlx::query(
        "INSERT INTO books (id, library_id, series_id, title, kind, author, authors) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(library_id)
    .bind(series_id)
    .bind(title)
    .bind(kind)
    .bind(author)
    .bind(&authors_vec)
    .execute(pool)
    .await
    .unwrap();
    id
}

use crate::search::{BOOKS_SEARCH_SQL as BOOKS_SQL, SERIES_SEARCH_SQL as SERIES_SQL};

async fn set_reading_status(pool: &sqlx::PgPool, book_id: Uuid, user_id: Uuid, status: &str) {
    sqlx::query("INSERT INTO book_reading_progress (book_id, user_id, status) VALUES ($1, $2, $3)")
        .bind(book_id)
        .bind(user_id)
        .bind(status)
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_by_book_title(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    create_test_book(&pool, lib_id, None, "Batman Year One", "comic", None, &[]).await;
    create_test_book(&pool, lib_id, None, "Superman Returns", "comic", None, &[]).await;

    let rows = sqlx::query(BOOKS_SQL)
        .bind("%Batman%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("title"), "Batman Year One");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_by_series_name(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    let series_id = create_test_series(&pool, lib_id, "Dragon Ball").await;
    create_test_book(
        &pool,
        lib_id,
        Some(series_id),
        "Volume 1",
        "comic",
        None,
        &[],
    )
    .await;
    create_test_book(&pool, lib_id, None, "Unrelated Book", "comic", None, &[]).await;

    let rows = sqlx::query(BOOKS_SQL)
        .bind("%Dragon%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("title"), "Volume 1");
    assert_eq!(
        rows[0].get::<Option<String>, _>("series").unwrap(),
        "Dragon Ball"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_by_author(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    create_test_book(
        &pool,
        lib_id,
        None,
        "The Sandman",
        "comic",
        Some("Neil Gaiman"),
        &["Neil Gaiman"],
    )
    .await;
    create_test_book(
        &pool,
        lib_id,
        None,
        "Watchmen",
        "comic",
        Some("Alan Moore"),
        &["Alan Moore"],
    )
    .await;

    let rows = sqlx::query(BOOKS_SQL)
        .bind("%Gaiman%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("title"), "The Sandman");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_case_insensitive(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    create_test_book(&pool, lib_id, None, "Batman Year One", "comic", None, &[]).await;

    // Search with lowercase
    let rows = sqlx::query(BOOKS_SQL)
        .bind("%batman%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);

    // Search with uppercase
    let rows = sqlx::query(BOOKS_SQL)
        .bind("%BATMAN%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_library_scoped(pool: sqlx::PgPool) {
    let lib1 = create_test_library(&pool, "comics").await;
    let lib2 = create_test_library(&pool, "manga").await;
    create_test_book(&pool, lib1, None, "Batman Vol 1", "comic", None, &[]).await;
    create_test_book(&pool, lib2, None, "Batman Manga", "comic", None, &[]).await;

    // Search scoped to lib1 only
    let rows = sqlx::query(BOOKS_SQL)
        .bind("%Batman%")
        .bind(Some(lib1))
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("title"), "Batman Vol 1");
    assert_eq!(rows[0].get::<Uuid, _>("library_id"), lib1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_empty_query_returns_nothing(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    create_test_book(&pool, lib_id, None, "Batman", "comic", None, &[]).await;

    // Empty ILIKE pattern "%%" matches everything — but the handler rejects empty q.
    // With a truly empty pattern (just whitespace wrapped in %), it matches all.
    // This test verifies the SQL behavior with a pattern that matches nothing.
    let rows = sqlx::query(BOOKS_SQL)
        .bind("%zzz_no_match_zzz%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_series_returns_series_id(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    let series_id = create_test_series(&pool, lib_id, "Dragon Ball").await;
    create_test_book(
        &pool,
        lib_id,
        Some(series_id),
        "Dragon Ball Vol 1",
        "comic",
        None,
        &[],
    )
    .await;
    create_test_book(
        &pool,
        lib_id,
        Some(series_id),
        "Dragon Ball Vol 2",
        "comic",
        None,
        &[],
    )
    .await;

    let rows = sqlx::query(SERIES_SQL)
        .bind("%Dragon%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<Uuid, _>("series_id"), series_id);
    assert_eq!(rows[0].get::<String, _>("name"), "Dragon Ball");
    assert_eq!(rows[0].get::<i64, _>("book_count"), 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_series_excludes_unclassified(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    // Book without series — should NOT appear in series results
    create_test_book(&pool, lib_id, None, "Standalone Book", "comic", None, &[]).await;

    let rows = sqlx::query(SERIES_SQL)
        .bind("%Standalone%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_series_genre_restriction_hides_blocked(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;

    let shonen_id = create_test_series_with_genres(&pool, lib_id, "Dragon Ball", &["shonen"]).await;
    let mystery_id =
        create_test_series_with_genres(&pool, lib_id, "Dragon Mystery", &["mystere"]).await;

    create_test_book(
        &pool,
        lib_id,
        Some(shonen_id),
        "Dragon Ball Vol 1",
        "comic",
        None,
        &[],
    )
    .await;
    create_test_book(
        &pool,
        lib_id,
        Some(mystery_id),
        "Dragon Mystery Vol 1",
        "comic",
        None,
        &[],
    )
    .await;

    let user_id = create_test_user(&pool, "alice").await;
    block_genre_for_user(&pool, user_id, "mystere").await;

    // With restriction: only shonen series visible
    let rows = sqlx::query(SERIES_SQL)
        .bind("%Dragon%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(Some(user_id))
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<Uuid, _>("series_id"), shonen_id);

    // Without restriction (no user): both series visible
    let rows = sqlx::query(SERIES_SQL)
        .bind("%Dragon%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_series_book_count_not_inflated_by_multiple_readers(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    let series_id = create_test_series(&pool, lib_id, "Dragon Ball").await;
    let book1 = create_test_book(
        &pool,
        lib_id,
        Some(series_id),
        "Dragon Ball Vol 1",
        "comic",
        None,
        &[],
    )
    .await;
    let book2 = create_test_book(
        &pool,
        lib_id,
        Some(series_id),
        "Dragon Ball Vol 2",
        "comic",
        None,
        &[],
    )
    .await;

    let user1 = create_test_user(&pool, "alice").await;
    let user2 = create_test_user(&pool, "bob").await;
    set_reading_status(&pool, book1, user1, "read").await;
    set_reading_status(&pool, book2, user1, "read").await;
    set_reading_status(&pool, book1, user2, "read").await;

    let rows = sqlx::query(SERIES_SQL)
        .bind("%Dragon%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(Some(user1))
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<i64, _>("book_count"), 2);
    assert_eq!(rows[0].get::<i64, _>("books_read_count"), 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_series_read_count_scoped_to_user(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    let series_id = create_test_series(&pool, lib_id, "Dragon Ball").await;
    let book1 = create_test_book(
        &pool,
        lib_id,
        Some(series_id),
        "Dragon Ball Vol 1",
        "comic",
        None,
        &[],
    )
    .await;
    create_test_book(
        &pool,
        lib_id,
        Some(series_id),
        "Dragon Ball Vol 2",
        "comic",
        None,
        &[],
    )
    .await;

    let user1 = create_test_user(&pool, "alice").await;
    let user2 = create_test_user(&pool, "bob").await;
    set_reading_status(&pool, book1, user1, "read").await;
    set_reading_status(&pool, book1, user2, "read").await;

    let rows = sqlx::query(SERIES_SQL)
        .bind("%Dragon%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(Some(user2))
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<i64, _>("book_count"), 2);
    assert_eq!(rows[0].get::<i64, _>("books_read_count"), 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_series_read_count_zero_for_anonymous(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    let series_id = create_test_series(&pool, lib_id, "Dragon Ball").await;
    let book1 = create_test_book(
        &pool,
        lib_id,
        Some(series_id),
        "Dragon Ball Vol 1",
        "comic",
        None,
        &[],
    )
    .await;

    let user1 = create_test_user(&pool, "alice").await;
    set_reading_status(&pool, book1, user1, "read").await;

    let rows = sqlx::query(SERIES_SQL)
        .bind("%Dragon%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<i64, _>("book_count"), 1);
    assert_eq!(rows[0].get::<i64, _>("books_read_count"), 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_by_series_author(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    let series_id =
        create_test_series_with_authors(&pool, lib_id, "Sandman", &["Neil Gaiman"]).await;
    create_test_book(
        &pool,
        lib_id,
        Some(series_id),
        "Preludes and Nocturnes",
        "comic",
        None,
        &[],
    )
    .await;
    create_test_book(&pool, lib_id, None, "Unrelated Book", "comic", None, &[]).await;

    let rows = sqlx::query(BOOKS_SQL)
        .bind("%Gaiman%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get::<Option<String>, _>("series").unwrap(),
        "Sandman"
    );
}

/// The `candidate_ids` CTE matches authors on the concatenated text, which can
/// spuriously match across element boundaries. The exact per-element predicate
/// re-applied on the candidate set must reject those false positives.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_author_split_elements_not_matched_across_boundary(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    // No single author element contains "Neil Gaiman", but the concatenation does.
    create_test_book(
        &pool,
        lib_id,
        None,
        "Split Authors",
        "comic",
        None,
        &["Neil", "Gaiman"],
    )
    .await;

    let rows = sqlx::query(BOOKS_SQL)
        .bind("%Neil Gaiman%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .bind(None::<Uuid>)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 0);
}
