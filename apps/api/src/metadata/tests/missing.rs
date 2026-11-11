//! Integration tests for `GET /metadata/missing/{id}`.
//!
//! The endpoint must use the same counted-volume rule as the series list, the
//! volume endpoint and the download availability.

use sqlx::PgPool;
use uuid::Uuid;

use super::*;
use crate::state::test_state;

async fn seed_series_with_link(
    pool: &PgPool,
    library_name: &str,
    series_name: &str,
    total_volumes: Option<i32>,
) -> (Uuid, Uuid, Uuid) {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(library_id)
        .bind(library_name)
        .bind(format!("/libraries/{library_name}"))
        .execute(pool)
        .await
        .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name, total_volumes) VALUES ($1, $2, $3, $4)")
        .bind(series_id)
        .bind(library_id)
        .bind(series_name)
        .bind(total_volumes)
        .execute(pool)
        .await
        .unwrap();

    let link_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO external_metadata_links \
            (id, library_id, series_id, provider, external_id, status) \
         VALUES ($1, $2, $3, 'test', $4, 'approved')",
    )
    .bind(link_id)
    .bind(library_id)
    .bind(series_id)
    .bind(format!("ext:{link_id}"))
    .execute(pool)
    .await
    .unwrap();

    (library_id, series_id, link_id)
}

async fn add_local_book(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Uuid,
    title: &str,
    volume: Option<i32>,
    volume_type: &str,
) {
    sqlx::query(
        "INSERT INTO books (id, library_id, series_id, title, kind, format, volume, volume_type) \
         VALUES ($1, $2, $3, $4, 'comic', 'cbz', $5, $6)",
    )
    .bind(Uuid::new_v4())
    .bind(library_id)
    .bind(series_id)
    .bind(title)
    .bind(volume)
    .bind(volume_type)
    .execute(pool)
    .await
    .unwrap();
}

async fn add_external_volume(pool: &PgPool, link_id: Uuid, external_id: &str, volume: i32) {
    sqlx::query(
        "INSERT INTO external_book_metadata \
            (id, link_id, external_book_id, title, volume_number, book_id) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4, NULL)",
    )
    .bind(link_id)
    .bind(external_id)
    .bind(format!("Volume {volume}"))
    .bind(volume)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_books_use_numbered_integral_coverage(pool: PgPool) {
    let state = test_state(pool.clone());
    let (library_id, series_id, link_id) =
        seed_series_with_link(&pool, "lib", "Series", Some(3)).await;

    add_local_book(&pool, library_id, series_id, "Vol 1", Some(1), "regular").await;
    add_local_book(&pool, library_id, series_id, "INT 2", Some(2), "integral").await;
    for volume in 1..=3 {
        add_external_volume(&pool, link_id, &format!("ext-{volume}"), volume).await;
    }

    let Json(dto) = get_missing_books(State(state), AxumPath(link_id))
        .await
        .unwrap();

    assert_eq!(dto.total_external, 3);
    assert_eq!(dto.total_local, 2);
    assert_eq!(dto.missing_count, 1);
    assert_eq!(dto.missing_books.len(), 1);
    assert_eq!(dto.missing_books[0].volume_number, Some(3));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn unnumbered_integral_closes_every_gap(pool: PgPool) {
    let state = test_state(pool.clone());
    let (library_id, series_id, link_id) =
        seed_series_with_link(&pool, "lib", "Series", Some(3)).await;

    add_local_book(&pool, library_id, series_id, "INT", None, "integral").await;
    add_external_volume(&pool, link_id, "ext-1", 1).await;
    add_external_volume(&pool, link_id, "ext-2", 2).await;

    let Json(dto) = get_missing_books(State(state), AxumPath(link_id))
        .await
        .unwrap();

    assert_eq!(dto.total_external, 3);
    assert_eq!(dto.total_local, 3, "the integral stands in for every tome");
    assert_eq!(dto.missing_count, 0);
    assert!(dto.missing_books.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_books_ignore_volume_zero_and_duplicate_editions(pool: PgPool) {
    let state = test_state(pool.clone());
    let (_, _, link_id) = seed_series_with_link(&pool, "lib", "Series", None).await;

    add_external_volume(&pool, link_id, "edition-a", 1).await;
    add_external_volume(&pool, link_id, "edition-b", 1).await;
    add_external_volume(&pool, link_id, "special", 0).await;

    let Json(dto) = get_missing_books(State(state), AxumPath(link_id))
        .await
        .unwrap();

    assert_eq!(dto.total_external, 1, "volume 0 is not a provider tome");
    assert_eq!(dto.missing_count, 1);
    assert_eq!(dto.missing_books.len(), 1);
    assert_eq!(dto.missing_books[0].volume_number, Some(1));
}
