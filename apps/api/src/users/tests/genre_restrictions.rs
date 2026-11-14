//! Genre restrictions on direct-access endpoints: a reader gets a 404 on a
//! book (or series) whose series carries one of their blocked genres, an
//! unrestricted caller (admin token, `None`) is never filtered.

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use sqlx::PgPool;
use uuid::Uuid;

use super::{ensure_book_visible, ensure_series_visible, series_visible};
use crate::auth::AuthUser;
use crate::reading::{get_reading_progress, update_reading_progress, UpdateReadingProgressRequest};
use crate::state::test_state;

struct Fixture {
    reader: Uuid,
    blocked_series: Uuid,
    blocked_book: Uuid,
    open_series: Uuid,
    open_book: Uuid,
    loose_book: Uuid,
}

async fn insert_series(pool: &PgPool, lib: Uuid, name: &str, genres: &[&str]) -> Uuid {
    let genres: Vec<String> = genres.iter().map(|g| g.to_string()).collect();
    sqlx::query_scalar(
        "INSERT INTO series (library_id, name, genres) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(lib)
    .bind(name)
    .bind(&genres)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_book(pool: &PgPool, lib: Uuid, series_id: Option<Uuid>) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO books (id, library_id, series_id, kind, title, format) \
         VALUES (gen_random_uuid(), $1, $2, 'bd', 'Book', 'cbz') RETURNING id",
    )
    .bind(lib)
    .bind(series_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn fixture(pool: &PgPool) -> Fixture {
    let lib = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'L', '/libraries/l')")
        .bind(lib)
        .execute(pool)
        .await
        .unwrap();
    let reader: Uuid =
        sqlx::query_scalar("INSERT INTO users (username) VALUES ('reader') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO user_genre_restrictions (user_id, genre) VALUES ($1, 'Mature')")
        .bind(reader)
        .execute(pool)
        .await
        .unwrap();

    let blocked_series = insert_series(pool, lib, "Blocked", &["Action", "Mature"]).await;
    let open_series = insert_series(pool, lib, "Open", &["Action"]).await;
    Fixture {
        reader,
        blocked_series,
        blocked_book: insert_book(pool, lib, Some(blocked_series)).await,
        open_series,
        open_book: insert_book(pool, lib, Some(open_series)).await,
        loose_book: insert_book(pool, lib, None).await,
    }
}

fn as_reader(f: &Fixture) -> Option<Extension<AuthUser>> {
    Some(Extension(AuthUser { user_id: f.reader }))
}

fn status_of<T: IntoResponse>(r: Result<T, crate::error::ApiError>) -> StatusCode {
    match r {
        Ok(v) => v.into_response().status(),
        Err(e) => e.into_response().status(),
    }
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn helpers_apply_the_listing_rule(pool: PgPool) {
    let f = fixture(&pool).await;

    assert!(!series_visible(&pool, Some(f.reader), f.blocked_series)
        .await
        .unwrap());
    assert!(series_visible(&pool, Some(f.reader), f.open_series)
        .await
        .unwrap());
    assert!(series_visible(&pool, None, f.blocked_series).await.unwrap());
    assert!(!series_visible(&pool, None, Uuid::new_v4()).await.unwrap());

    assert!(
        ensure_series_visible(&pool, Some(f.reader), f.blocked_series)
            .await
            .is_err()
    );
    assert!(ensure_book_visible(&pool, Some(f.reader), f.blocked_book)
        .await
        .is_err());
    assert!(ensure_book_visible(&pool, Some(f.reader), f.open_book)
        .await
        .is_ok());
    assert!(ensure_book_visible(&pool, Some(f.reader), f.loose_book)
        .await
        .is_ok());
    assert!(ensure_book_visible(&pool, None, f.blocked_book)
        .await
        .is_ok());
    assert!(ensure_book_visible(&pool, None, Uuid::new_v4())
        .await
        .is_err());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_book_hides_restricted_book(pool: PgPool) {
    let f = fixture(&pool).await;
    let state = test_state(pool);

    let r = crate::books::get_book(State(state.clone()), Path(f.blocked_book), as_reader(&f)).await;
    assert_eq!(status_of(r), StatusCode::NOT_FOUND);

    let r = crate::books::get_book(State(state.clone()), Path(f.open_book), as_reader(&f)).await;
    assert_eq!(status_of(r), StatusCode::OK);

    let r = crate::books::get_book(State(state), Path(f.blocked_book), None).await;
    assert_eq!(status_of(r), StatusCode::OK);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn media_endpoints_hide_restricted_book(pool: PgPool) {
    let f = fixture(&pool).await;
    let state = test_state(pool);

    let r = crate::books::get_thumbnail(
        State(state.clone()),
        as_reader(&f),
        Path(f.blocked_book),
        HeaderMap::new(),
    )
    .await;
    assert_eq!(status_of(r), StatusCode::NOT_FOUND);

    let r = crate::books::get_book_file(State(state.clone()), as_reader(&f), Path(f.blocked_book))
        .await;
    assert_eq!(status_of(r), StatusCode::NOT_FOUND);

    let r = crate::books::get_page(
        State(state),
        as_reader(&f),
        Path((f.blocked_book, 1)),
        axum::extract::Query(crate::books::PageQuery {
            format: None,
            quality: None,
            width: None,
        }),
        HeaderMap::new(),
    )
    .await;
    assert_eq!(status_of(r), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn progress_endpoints_hide_restricted_book(pool: PgPool) {
    let f = fixture(&pool).await;
    let state = test_state(pool.clone());

    let r = get_reading_progress(State(state.clone()), as_reader(&f), Path(f.blocked_book)).await;
    assert_eq!(status_of(r), StatusCode::NOT_FOUND);

    let r = update_reading_progress(
        State(state.clone()),
        as_reader(&f),
        Path(f.blocked_book),
        Json(UpdateReadingProgressRequest {
            status: "read".to_string(),
            current_page: None,
        }),
    )
    .await;
    assert_eq!(status_of(r), StatusCode::NOT_FOUND);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM book_reading_progress")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);

    let r = get_reading_progress(State(state), as_reader(&f), Path(f.open_book)).await;
    assert_eq!(status_of(r), StatusCode::OK);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_metadata_hides_restricted_series(pool: PgPool) {
    let f = fixture(&pool).await;
    let state = test_state(pool);

    let r = crate::series::get_series_metadata_by_id(
        State(state.clone()),
        as_reader(&f),
        Path(f.blocked_series),
    )
    .await;
    assert_eq!(status_of(r), StatusCode::NOT_FOUND);

    let r = crate::series::get_series_metadata_by_id(
        State(state.clone()),
        as_reader(&f),
        Path(f.open_series),
    )
    .await;
    assert_eq!(status_of(r), StatusCode::OK);

    let r =
        crate::series::get_series_metadata_by_id(State(state), None, Path(f.blocked_series)).await;
    assert_eq!(status_of(r), StatusCode::OK);
}
