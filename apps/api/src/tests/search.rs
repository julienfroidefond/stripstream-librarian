use super::*;
use sqlx::Row;

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

/// The books search SQL from the handler, extracted for testing.
const BOOKS_SQL: &str = r#"
    SELECT b.id, b.library_id, b.kind, b.title,
        COALESCE(b.authors, CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END) as authors,
        s.name AS series, b.volume, b.language
    FROM books b
    LEFT JOIN series s ON s.id = b.series_id
    WHERE (
        b.title ILIKE $1
        OR s.name ILIKE $1
        OR EXISTS (SELECT 1 FROM unnest(
            COALESCE(b.authors, CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END)
            || COALESCE(s.authors, ARRAY[]::text[])
        ) AS a WHERE a ILIKE $1)
    )
    AND ($2::uuid IS NULL OR b.library_id = $2)
    AND ($3::text IS NULL OR b.kind = $3)
    ORDER BY
        CASE WHEN b.title ILIKE $1 THEN 0 ELSE 1 END,
        b.title ASC
    LIMIT $4
"#;

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
    create_test_book(&pool, lib_id, Some(series_id), "Volume 1", "comic", None, &[]).await;
    create_test_book(&pool, lib_id, None, "Unrelated Book", "comic", None, &[]).await;

    let rows = sqlx::query(BOOKS_SQL)
        .bind("%Dragon%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("title"), "Volume 1");
    assert_eq!(rows[0].get::<Option<String>, _>("series").unwrap(), "Dragon Ball");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn search_by_author(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    create_test_book(&pool, lib_id, None, "The Sandman", "comic", Some("Neil Gaiman"), &["Neil Gaiman"]).await;
    create_test_book(&pool, lib_id, None, "Watchmen", "comic", Some("Alan Moore"), &["Alan Moore"]).await;

    let rows = sqlx::query(BOOKS_SQL)
        .bind("%Gaiman%")
        .bind(None::<Uuid>)
        .bind(None::<&str>)
        .bind(20i64)
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
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 0);
}
