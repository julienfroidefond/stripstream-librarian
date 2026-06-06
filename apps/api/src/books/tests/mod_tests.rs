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
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, series_id, title, kind, authors, locked_fields) VALUES ($1, $2, $3, $4, $5, '{}'::text[], '{}'::jsonb)",
    )
    .bind(id)
    .bind(library_id)
    .bind(series_id)
    .bind(title)
    .bind(kind)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn create_test_book_file(pool: &sqlx::PgPool, book_id: Uuid, abs_path: &str, format: &str) {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status) VALUES ($1, $2, $3, $4, 1024, NOW(), 'fp123', 'ok')",
    )
    .bind(id)
    .bind(book_id)
    .bind(format)
    .bind(abs_path)
    .execute(pool)
    .await
    .unwrap();
}

/// The get_book SQL from the handler.
const GET_BOOK_SQL: &str = r#"
    SELECT b.id, b.library_id, b.kind, b.title, b.author, b.authors, s.name AS series, b.series_id, b.volume, b.volume_type, b.language, b.page_count, b.thumbnail_path, b.locked_fields, b.summary, b.isbn, b.publish_date,
           bf.abs_path, bf.format, bf.parse_status,
           COALESCE(brp.status, 'unread') AS reading_status,
           brp.current_page AS reading_current_page,
           brp.last_read_at AS reading_last_read_at
    FROM books b
    LEFT JOIN series s ON s.id = b.series_id
    LEFT JOIN LATERAL (
      SELECT abs_path, format, parse_status
      FROM book_files
      WHERE book_id = b.id
      ORDER BY updated_at DESC
      LIMIT 1
    ) bf ON TRUE
    LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
    WHERE b.id = $1
"#;

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_book_returns_series_id_when_book_has_series(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    let series_id = create_test_series(&pool, lib_id, "Dragon Ball").await;
    let book_id =
        create_test_book(&pool, lib_id, Some(series_id), "Dragon Ball Vol 1", "comic").await;

    let row = sqlx::query(GET_BOOK_SQL)
        .bind(book_id)
        .bind(None::<Uuid>)
        .fetch_one(&pool)
        .await
        .unwrap();

    let returned_series_id: Option<Uuid> = row.get("series_id");
    assert_eq!(
        returned_series_id,
        Some(series_id),
        "series_id should be returned for book with series"
    );
    assert_eq!(
        row.get::<Option<String>, _>("series").unwrap(),
        "Dragon Ball"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_book_returns_null_series_id_when_no_series(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    let book_id = create_test_book(&pool, lib_id, None, "Standalone Book", "comic").await;

    let row = sqlx::query(GET_BOOK_SQL)
        .bind(book_id)
        .bind(None::<Uuid>)
        .fetch_one(&pool)
        .await
        .unwrap();

    let returned_series_id: Option<Uuid> = row.get("series_id");
    assert!(
        returned_series_id.is_none(),
        "series_id should be null for book without series"
    );
    assert!(
        row.get::<Option<String>, _>("series").is_none(),
        "series name should be null"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_book_basic_fields_populated(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "comics").await;
    let series_id = create_test_series(&pool, lib_id, "Naruto").await;
    let book_id = create_test_book(&pool, lib_id, Some(series_id), "Naruto Vol 1", "comic").await;
    create_test_book_file(&pool, book_id, "/libraries/comics/naruto_v1.cbz", "cbz").await;

    let row = sqlx::query(GET_BOOK_SQL)
        .bind(book_id)
        .bind(None::<Uuid>)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<Uuid, _>("id"), book_id);
    assert_eq!(row.get::<Uuid, _>("library_id"), lib_id);
    assert_eq!(row.get::<String, _>("kind"), "comic");
    assert_eq!(row.get::<String, _>("title"), "Naruto Vol 1");
    assert_eq!(row.get::<String, _>("reading_status"), "unread");
    // File info from book_files
    assert_eq!(
        row.get::<Option<String>, _>("abs_path").unwrap(),
        "/libraries/comics/naruto_v1.cbz"
    );
    assert_eq!(row.get::<Option<String>, _>("format").unwrap(), "cbz");
    assert_eq!(row.get::<Option<String>, _>("parse_status").unwrap(), "ok");
}
