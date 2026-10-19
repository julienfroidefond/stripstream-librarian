//! Tests for the Komga read-status sync integration.
//!
//! The Komga server is faked with `wiremock`; everything else (book lookup,
//! match de-duplication, reading-progress upsert, report persistence) runs
//! against a real migrated temporary database.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::state::test_state;

async fn create_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO libraries (id, name, root_path) VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(name)
    .bind(format!("/libraries/{name}"))
    .fetch_one(pool)
    .await
    .unwrap()
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

async fn create_user(pool: &sqlx::PgPool, username: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (id, username) VALUES (gen_random_uuid(), $1) RETURNING id",
    )
    .bind(username)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Build a Komga `/api/v1/books/list` page payload.
fn komga_page(books: &[(&str, &str)], number: i32, total_pages: i32) -> serde_json::Value {
    let content: Vec<serde_json::Value> = books
        .iter()
        .map(|(series_title, title)| {
            serde_json::json!({
                "name": "book.cbz",
                "seriesTitle": series_title,
                "metadata": { "title": title },
            })
        })
        .collect();

    serde_json::json!({
        "content": content,
        "totalPages": total_pages,
        "number": number,
    })
}

async fn mount_komga_page(server: &MockServer, page: usize, body: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path("/api/v1/books/list"))
        .and(query_param("page", page.to_string()))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn sync(
    pool: &sqlx::PgPool,
    url: &str,
    user_id: Uuid,
) -> Result<KomgaSyncResponse, ApiError> {
    let state = test_state(pool.clone());
    let response = sync_komga_read_books(
        State(state),
        Json(KomgaSyncRequest {
            url: url.to_string(),
            username: "komga-user".to_string(),
            password: "komga-pass".to_string(),
            user_id,
        }),
    )
    .await?;
    Ok(response.0)
}

async fn reading_status(pool: &sqlx::PgPool, book_id: Uuid, user_id: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT status FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
    )
    .bind(book_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_matches_local_books_and_marks_them_read(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "komga_main").await;
    let series = create_series(&pool, lib, "Astérix").await;
    let book = create_book(&pool, lib, series, "Le Gaulois").await;
    let user = create_user(&pool, "komga_reader").await;

    let server = MockServer::start().await;
    mount_komga_page(&server, 0, komga_page(&[("Astérix", "Le Gaulois")], 0, 1)).await;

    let report = sync(&pool, &server.uri(), user).await.unwrap();

    assert_eq!(report.total_komga_read, 1);
    assert_eq!(report.matched, 1);
    assert_eq!(report.already_read, 0);
    assert_eq!(report.newly_marked, 1);
    assert!(report.unmatched.is_empty());
    assert_eq!(
        report.matched_books,
        vec!["Astérix - Le Gaulois".to_string()]
    );
    assert_eq!(report.user_id, Some(user));

    // The local book is now flagged read for that user.
    assert_eq!(
        reading_status(&pool, book, user).await.as_deref(),
        Some("read")
    );

    // A report row was persisted.
    let reports: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM komga_sync_reports")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(reports, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_is_idempotent_for_already_read_books(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "komga_idem").await;
    let series = create_series(&pool, lib, "Blacksad").await;
    let book = create_book(&pool, lib, series, "Quelque part entre les ombres").await;
    let user = create_user(&pool, "komga_idem_user").await;

    sqlx::query(
        "INSERT INTO book_reading_progress (book_id, user_id, status) VALUES ($1, $2, 'read')",
    )
    .bind(book)
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();

    let server = MockServer::start().await;
    mount_komga_page(
        &server,
        0,
        komga_page(&[("Blacksad", "Quelque part entre les ombres")], 0, 1),
    )
    .await;

    let report = sync(&pool, &server.uri(), user).await.unwrap();

    assert_eq!(report.matched, 1);
    assert_eq!(report.already_read, 1);
    assert_eq!(report.newly_marked, 0);
    assert!(report.newly_marked_books.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_reports_unmatched_books(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "komga_unmatched").await;
    let series = create_series(&pool, lib, "Astérix").await;
    create_book(&pool, lib, series, "Le Gaulois").await;
    let user = create_user(&pool, "komga_unmatched_user").await;

    let server = MockServer::start().await;
    mount_komga_page(
        &server,
        0,
        komga_page(
            &[
                ("Astérix", "Le Gaulois"),
                ("Naruto", "Tome 1"),
                ("", "Orphelin"),
            ],
            0,
            1,
        ),
    )
    .await;

    let report = sync(&pool, &server.uri(), user).await.unwrap();

    assert_eq!(report.total_komga_read, 3);
    assert_eq!(report.matched, 1);
    assert_eq!(
        report.unmatched,
        vec!["Naruto - Tome 1".to_string(), "Orphelin".to_string()]
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_falls_back_to_title_match_when_series_differs(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "komga_fallback").await;
    let series = create_series(&pool, lib, "Astérix").await;
    let book = create_book(&pool, lib, series, "Le Gaulois").await;
    let user = create_user(&pool, "komga_fallback_user").await;

    let server = MockServer::start().await;
    // Komga reports a different series title, but the book title matches.
    mount_komga_page(
        &server,
        0,
        komga_page(&[("Unknown Series", "Le Gaulois")], 0, 1),
    )
    .await;

    let report = sync(&pool, &server.uri(), user).await.unwrap();

    assert_eq!(report.matched, 1);
    // The display label uses the local series name, not Komga's.
    assert_eq!(
        report.matched_books,
        vec!["Astérix - Le Gaulois".to_string()]
    );
    assert_eq!(
        reading_status(&pool, book, user).await.as_deref(),
        Some("read")
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_paginates_through_all_komga_pages(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "komga_pages").await;
    let series = create_series(&pool, lib, "One Piece").await;
    let first = create_book(&pool, lib, series, "Tome 1").await;
    let second = create_book(&pool, lib, series, "Tome 2").await;
    let user = create_user(&pool, "komga_pages_user").await;

    let server = MockServer::start().await;
    mount_komga_page(&server, 0, komga_page(&[("One Piece", "Tome 1")], 0, 2)).await;
    mount_komga_page(&server, 1, komga_page(&[("One Piece", "Tome 2")], 1, 2)).await;

    let report = sync(&pool, &server.uri(), user).await.unwrap();

    assert_eq!(report.total_komga_read, 2);
    assert_eq!(report.matched, 2);
    assert_eq!(
        reading_status(&pool, first, user).await.as_deref(),
        Some("read")
    );
    assert_eq!(
        reading_status(&pool, second, user).await.as_deref(),
        Some("read")
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_errors_when_komga_returns_error_status(pool: sqlx::PgPool) {
    let user = create_user(&pool, "komga_error_user").await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/books/list"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let err = match sync(&pool, &server.uri(), user).await {
        Ok(_) => panic!("expected an error when Komga returns 500"),
        Err(err) => err,
    };

    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        err.message.contains("Komga returned"),
        "unexpected: {}",
        err.message
    );

    // Nothing was persisted.
    let reports: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM komga_sync_reports")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(reports, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_rejects_empty_url(pool: sqlx::PgPool) {
    let user = create_user(&pool, "komga_no_url").await;
    let state = test_state(pool.clone());

    let err = sync_komga_read_books(
        State(state),
        Json(KomgaSyncRequest {
            url: String::new(),
            username: "u".to_string(),
            password: "p".to_string(),
            user_id: user,
        }),
    )
    .await
    .map(|_| ())
    .expect_err("an empty url must be rejected");

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn reports_are_listed_newest_first_and_can_be_fetched(pool: sqlx::PgPool) {
    let user = create_user(&pool, "komga_reports_user").await;

    let older = insert_report(&pool, "https://komga-old", user, 1, 1, 1, 0).await;
    let newer = insert_report(&pool, "https://komga-new", user, 3, 2, 1, 1).await;

    let state = test_state(pool.clone());
    let reports = list_sync_reports(State(state.clone())).await.unwrap().0;
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].id, newer);
    assert_eq!(reports[0].komga_url, "https://komga-new");
    assert_eq!(reports[0].unmatched_count, 1);
    assert_eq!(reports[1].id, older);

    let fetched = get_sync_report(State(state.clone()), axum::extract::Path(newer))
        .await
        .unwrap()
        .0;
    assert_eq!(fetched.id, newer);
    assert_eq!(
        fetched.matched_books,
        vec!["Astérix - Le Gaulois".to_string()]
    );
    assert_eq!(fetched.unmatched, vec!["Naruto - Tome 1".to_string()]);

    let missing = get_sync_report(State(state), axum::extract::Path(Uuid::new_v4()))
        .await
        .map(|_| ())
        .expect_err("an unknown report id must 404");
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[allow(clippy::too_many_arguments)]
async fn insert_report(
    pool: &sqlx::PgPool,
    url: &str,
    user_id: Uuid,
    total: i64,
    matched: i64,
    already_read: i64,
    newly_marked: i64,
) -> Uuid {
    let unmatched = serde_json::json!(["Naruto - Tome 1"]);
    let matched_books = serde_json::json!(["Astérix - Le Gaulois"]);
    let newly_marked_books = serde_json::json!([]);

    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO komga_sync_reports \
             (komga_url, user_id, total_komga_read, matched, already_read, newly_marked, \
              matched_books, newly_marked_books, unmatched) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
    )
    .bind(url)
    .bind(user_id)
    .bind(total)
    .bind(matched)
    .bind(already_read)
    .bind(newly_marked)
    .bind(&matched_books)
    .bind(&newly_marked_books)
    .bind(&unmatched)
    .fetch_one(pool)
    .await
    .unwrap()
}
