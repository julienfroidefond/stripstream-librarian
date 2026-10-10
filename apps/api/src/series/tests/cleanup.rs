use sqlx::Row;
use uuid::Uuid;

use crate::series::update::{archive_series_row, prune_series_if_empty};

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

async fn series_exists(pool: &sqlx::PgPool, series_id: Uuid) -> bool {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
        .bind(series_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn is_archived(pool: &sqlx::PgPool, series_id: Uuid) -> bool {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM archived_series WHERE id = $1)")
        .bind(series_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn prune_removes_empty_unlinked_series(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "prune").await;
    let series_id = create_series(&pool, lib_id, "Empty").await;

    let removed = prune_series_if_empty(&pool, series_id).await.unwrap();

    assert!(removed, "empty unlinked series should be removed");
    assert!(!series_exists(&pool, series_id).await);
    assert!(is_archived(&pool, series_id).await);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn prune_archives_series_metadata(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "prune_archive").await;
    let series_id = create_series(&pool, lib_id, "Archived").await;
    sqlx::query("UPDATE series SET description = $2, authors = $3 WHERE id = $1")
        .bind(series_id)
        .bind("A great series")
        .bind(vec!["Author A".to_string()])
        .execute(&pool)
        .await
        .unwrap();

    assert!(prune_series_if_empty(&pool, series_id).await.unwrap());

    let row = sqlx::query("SELECT name, description, authors FROM archived_series WHERE id = $1")
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("name"), "Archived");
    assert_eq!(
        row.get::<Option<String>, _>("description").as_deref(),
        Some("A great series")
    );
    assert_eq!(
        row.get::<Vec<String>, _>("authors"),
        vec!["Author A".to_string()]
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn prune_keeps_series_that_still_has_books(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "prune_keep").await;
    let series_id = create_series(&pool, lib_id, "Not empty").await;
    create_book(&pool, lib_id, series_id, "Vol 1").await;

    let removed = prune_series_if_empty(&pool, series_id).await.unwrap();

    assert!(!removed, "series with a book must not be removed");
    assert!(series_exists(&pool, series_id).await);
    assert!(!is_archived(&pool, series_id).await);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn prune_keeps_series_with_metadata_link(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "prune_link").await;
    let series_id = create_series(&pool, lib_id, "Discovery").await;
    sqlx::query(
        "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id, status) \
         VALUES ($1, $2, 'senscritique', '123', 'approved')",
    )
    .bind(lib_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    let removed = prune_series_if_empty(&pool, series_id).await.unwrap();

    assert!(!removed, "linked Discovery series must be preserved");
    assert!(series_exists(&pool, series_id).await);
    assert!(
        !is_archived(&pool, series_id).await,
        "preserved series must not be archived"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn prune_keeps_series_with_available_download(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "prune_dl").await;
    let series_id = create_series(&pool, lib_id, "Wishlist").await;
    sqlx::query("INSERT INTO available_downloads (library_id, series_id) VALUES ($1, $2)")
        .bind(lib_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    let removed = prune_series_if_empty(&pool, series_id).await.unwrap();

    assert!(
        !removed,
        "series with a wishlist download must be preserved"
    );
    assert!(series_exists(&pool, series_id).await);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn archive_series_row_preserves_anilist_link(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "prune_anilist").await;
    let series_id = create_series(&pool, lib_id, "AniListed").await;
    sqlx::query(
        "INSERT INTO anilist_series_links (library_id, series_id, anilist_id, anilist_title, anilist_url) \
         VALUES ($1, $2, 42, 'AniListed', 'https://anilist.co/manga/42')",
    )
    .bind(lib_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    archive_series_row(&pool, series_id).await.unwrap();

    let row = sqlx::query(
        "SELECT anilist_id, anilist_title, anilist_url FROM archived_series WHERE id = $1",
    )
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<Option<i32>, _>("anilist_id"), Some(42));
    assert_eq!(
        row.get::<Option<String>, _>("anilist_title").as_deref(),
        Some("AniListed")
    );
    assert_eq!(
        row.get::<Option<String>, _>("anilist_url").as_deref(),
        Some("https://anilist.co/manga/42")
    );
}
