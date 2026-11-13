use axum::extract::{Path, Query, State};
use axum::Json;
use sqlx::Row;
use uuid::Uuid;

use stripstream_thumbnails::{EncodedThumbnail, ThumbnailExt, ThumbnailSettings};

use super::{list_books, update_book, ListBooksQuery, UpdateBookRequest, GET_BOOK_SQL};
use crate::state::test_state;

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
    assert!(
        row.get::<Option<i64>, _>("size_bytes").is_none(),
        "size_bytes should be null without a book file"
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
    assert_eq!(row.get::<Option<i64>, _>("size_bytes").unwrap(), 1024);
}

fn query_all() -> Query<ListBooksQuery> {
    Query(ListBooksQuery {
        q: None,
        library_id: None,
        kind: None,
        format: None,
        series: None,
        reading_status: None,
        author: None,
        page: None,
        limit: None,
        sort: None,
        metadata_provider: None,
        gap: None,
    })
}

fn query_q(q: &str) -> Query<ListBooksQuery> {
    Query(ListBooksQuery {
        q: Some(q.to_string()),
        ..query_all().0
    })
}

/// The `/books` text filter normalizes both sides via `norm_text()`, so an
/// unaccented query must match an accented series name.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_books_query_matches_series_name_accent_insensitively(pool: sqlx::PgPool) {
    let state = test_state(pool.clone());
    let lib_id = create_test_library(&pool, "comics").await;
    let series_id = create_test_series(&pool, lib_id, "Astérix").await;
    create_test_book(&pool, lib_id, Some(series_id), "Le Devin", "comic").await;
    create_test_book(&pool, lib_id, None, "Tintin au Tibet", "comic").await;

    let Json(page) = list_books(State(state), query_q("asterix"), None)
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].title, "Le Devin");
}

/// Same for the legacy scalar `author` column.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_books_query_matches_scalar_author_accent_insensitively(pool: sqlx::PgPool) {
    let state = test_state(pool.clone());
    let lib_id = create_test_library(&pool, "comics").await;

    let book_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, title, kind, author, authors) VALUES ($1, $2, $3, $4, $5, '{}'::text[])",
    )
    .bind(book_id)
    .bind(lib_id)
    .bind("Le Devin")
    .bind("comic")
    .bind("René Goscinny")
    .execute(&pool)
    .await
    .unwrap();
    create_test_book(&pool, lib_id, None, "Tintin au Tibet", "comic").await;

    let Json(page) = list_books(State(state), query_q("rene goscinny"), None)
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].title, "Le Devin");
}

/// The list payload carries what the backoffice list view renders on top of the
/// card view: the summary, the publish date and the size of the latest
/// `book_files` row.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_books_exposes_summary_date_and_file_size(pool: sqlx::PgPool) {
    let state = test_state(pool.clone());
    let lib_id = create_test_library(&pool, "comics").await;

    let book_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, title, kind, authors, summary, publish_date) VALUES ($1, $2, $3, $4, '{}'::text[], $5, $6)",
    )
    .bind(book_id)
    .bind(lib_id)
    .bind("Berserk")
    .bind("comic")
    .bind("A dark fantasy saga.")
    .bind("1989")
    .execute(&pool)
    .await
    .unwrap();
    create_test_book_file(&pool, book_id, "/libraries/comics/berserk_v1.cbz", "cbz").await;

    let Json(page) = list_books(
        State(state),
        Query(ListBooksQuery {
            library_id: Some(lib_id),
            ..query_all().0
        }),
        None,
    )
    .await
    .unwrap();

    assert_eq!(page.items.len(), 1);
    let item = &page.items[0];
    assert_eq!(item.summary.as_deref(), Some("A dark fantasy saga."));
    assert_eq!(item.publish_date.as_deref(), Some("1989"));
    assert_eq!(item.file_size, Some(1024));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_book_sanitizes_locked_fields(pool: sqlx::PgPool) {
    let state = test_state(pool.clone());
    let lib_id = create_test_library(&pool, "book_locks").await;
    let book_id = create_test_book(&pool, lib_id, None, "Berserk vol. 1", "comic").await;

    let Json(details) = update_book(
        State(state),
        Path(book_id),
        Json(UpdateBookRequest {
            title: "Berserk vol. 1".to_string(),
            author: None,
            authors: vec!["Kentaro Miura".to_string()],
            series: None,
            volume: None,
            language: None,
            summary: Some("A dark fantasy".to_string()),
            isbn: None,
            publish_date: None,
            locked_fields: Some(serde_json::json!({
                "summary": true,
                "language": true,
                "authors": "true",
                "title": true,
                "series": true,
                "volume": true,
            })),
        }),
    )
    .await
    .unwrap();

    assert_eq!(
        details.locked_fields,
        Some(serde_json::json!({"summary": true, "language": true})),
        "stale and non-boolean locks are dropped before the write"
    );

    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT locked_fields FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored,
        serde_json::json!({"summary": true, "language": true})
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn rendered_thumbnail_completes_only_cover_pending_books(pool: sqlx::PgPool) {
    let state = test_state(pool.clone());
    let directory = tempfile::tempdir().unwrap();
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('thumbnail', $1) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(serde_json::json!({ "directory": directory.path() }))
    .execute(&pool)
    .await
    .unwrap();

    let library = create_test_library(&pool, "thumbnail-state").await;
    let cover_pending = create_test_book(&pool, library, None, "Cover", "bd").await;
    let analyzing = create_test_book(&pool, library, None, "Analyzing", "bd").await;
    let failed = create_test_book(&pool, library, None, "Failed", "bd").await;

    sqlx::query("UPDATE books SET page_count = 3, analysis_status = 'cover_pending' WHERE id = $1")
        .bind(cover_pending)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE books SET analysis_status = 'analyzing' WHERE id = $1")
        .bind(analyzing)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE books SET analysis_status = 'failed' WHERE id = $1")
        .bind(failed)
        .execute(&pool)
        .await
        .unwrap();

    let store = ThumbnailSettings::load(&pool).await.unwrap().store();
    for book_id in [cover_pending, analyzing, failed] {
        let thumbnail = EncodedThumbnail {
            bytes: b"cover".to_vec(),
            ext: ThumbnailExt::Webp,
        };
        super::persist_rendered_thumbnail(&state, &store, book_id, thumbnail)
            .await
            .unwrap();
    }

    let statuses: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, analysis_status, thumbnail_path FROM books WHERE library_id = $1 ORDER BY title",
    )
    .bind(library)
    .fetch_all(&pool)
    .await
    .unwrap();

    // Ordered by title: Analyzing, Cover, Failed.
    assert_eq!(statuses[0].1, "analyzing");
    assert_eq!(statuses[1].1, "complete");
    assert_eq!(statuses[2].1, "failed");
    assert!(
        statuses.iter().all(|(_, _, path)| path.is_some()),
        "the rendered thumbnail must be stored for every book"
    );
}

fn png_page(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(width, height, image::Rgb([200, 10, 10]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

fn write_cbz(path: &std::path::Path, data: &[u8]) {
    use std::io::Write;
    let file = std::fs::File::create(path).unwrap();
    let mut zip = ::zip::ZipWriter::new(file);
    let opts = ::zip::write::SimpleFileOptions::default()
        .compression_method(::zip::CompressionMethod::Stored);
    zip.start_file("001.png", opts).unwrap();
    zip.write_all(data).unwrap();
    zip.finish().unwrap();
}

/// The on-the-fly fallback encodes with the shared thumbnail settings (format,
/// size, directory) instead of hard-coded values, and replaces a stale file.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn thumbnail_fallback_uses_shared_store_settings(pool: sqlx::PgPool) {
    let state = test_state(pool.clone());
    let thumbnails = tempfile::tempdir().unwrap();
    let books = tempfile::tempdir().unwrap();
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('thumbnail', $1) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(serde_json::json!({
        "format": "png",
        "width": 40,
        "height": 60,
        "directory": thumbnails.path(),
    }))
    .execute(&pool)
    .await
    .unwrap();

    let library = create_test_library(&pool, "thumbnail-fallback").await;
    let book_id = create_test_book(&pool, library, None, "Cover", "bd").await;
    let cbz = books.path().join("T01.cbz");
    write_cbz(&cbz, &png_page(400, 600));
    create_test_book_file(&pool, book_id, &cbz.to_string_lossy(), "cbz").await;
    // A stale thumbnail in another format, as left by an earlier configuration.
    let stale = thumbnails.path().join(format!("{book_id}.webp"));
    std::fs::write(&stale, b"stale").unwrap();

    let response = super::get_thumbnail(State(state), Path(book_id), axum::http::HeaderMap::new())
        .await
        .unwrap();

    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(response.headers()["x-thumbnail-source"], "rendered");
    let published = thumbnails.path().join(format!("{book_id}.png"));
    let decoded = image::load_from_memory(&std::fs::read(&published).unwrap()).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (40, 60));
    assert!(
        !stale.exists(),
        "the thumbnail of the previous format is replaced"
    );

    let stored: Option<String> =
        sqlx::query_scalar("SELECT thumbnail_path FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored.as_deref(),
        Some(published.to_string_lossy().as_ref())
    );
}
