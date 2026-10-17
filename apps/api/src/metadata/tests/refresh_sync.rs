use super::*;

async fn seed_library_and_series(pool: &PgPool) -> (Uuid, Uuid) {
    let lib_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'test', '/libraries/test')",
    )
    .bind(lib_id)
    .execute(pool)
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Gon')")
        .bind(series_id)
        .bind(lib_id)
        .execute(pool)
        .await
        .unwrap();

    (lib_id, series_id)
}

async fn seed_book(pool: &PgPool, lib_id: Uuid, series_id: Uuid, volume: i32) -> Uuid {
    let book_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, series_id, title, kind, format, volume) \
         VALUES ($1, $2, $3, $4, 'comic', 'cbz', $5)",
    )
    .bind(book_id)
    .bind(lib_id)
    .bind(series_id)
    .bind(format!("Gon T{volume:02}"))
    .bind(volume)
    .execute(pool)
    .await
    .unwrap();
    book_id
}

async fn seed_link(pool: &PgPool, lib_id: Uuid, series_id: Uuid) -> Uuid {
    let link_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO external_metadata_links \
             (id, library_id, series_id, provider, external_id, status) \
         VALUES ($1, $2, $3, 'senscritique', 'franchise:1', 'approved')",
    )
    .bind(link_id)
    .bind(lib_id)
    .bind(series_id)
    .execute(pool)
    .await
    .unwrap();
    link_id
}

async fn seed_external_book(
    pool: &PgPool,
    link_id: Uuid,
    external_book_id: &str,
    volume: i32,
    book_id: Option<Uuid>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO external_book_metadata \
             (id, link_id, book_id, external_book_id, volume_number, title) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(link_id)
    .bind(book_id)
    .bind(external_book_id)
    .bind(volume)
    .bind(format!("Gon T{volume:02} ({external_book_id})"))
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn rematch_links_at_most_one_row_per_book_and_link(pool: sqlx::PgPool) {
    let (lib_id, series_id) = seed_library_and_series(&pool).await;
    let book_id = seed_book(&pool, lib_id, series_id, 1).await;
    let link_id = seed_link(&pool, lib_id, series_id).await;

    // Two unlinked editions of the same volume. Linking both to the same book
    // makes the merge pick a random row on every refresh (issue #65).
    seed_external_book(&pool, link_id, "1", 1, None).await;
    seed_external_book(&pool, link_id, "2", 1, None).await;

    let count = rematch_unlinked_books(&pool, lib_id).await.unwrap();

    assert_eq!(count, 1);
    let linked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM external_book_metadata WHERE link_id = $1 AND book_id = $2",
    )
    .bind(link_id)
    .bind(book_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, 1);

    // Re-running is a no-op: the link already has a row for this book.
    assert_eq!(rematch_unlinked_books(&pool, lib_id).await.unwrap(), 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn rematch_picks_the_lowest_book_id_when_volumes_collide(pool: sqlx::PgPool) {
    let (lib_id, series_id) = seed_library_and_series(&pool).await;
    let first_book = seed_book(&pool, lib_id, series_id, 1).await;

    // A second local book with the same volume number (duplicate file).
    let duplicate_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, series_id, title, kind, format, volume) \
         VALUES ($1, $2, $3, 'Gon T01 bis', 'comic', 'cbz', 1)",
    )
    .bind(duplicate_id)
    .bind(lib_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    let link_id = seed_link(&pool, lib_id, series_id).await;
    let external_id = seed_external_book(&pool, link_id, "1", 1, None).await;

    assert_eq!(rematch_unlinked_books(&pool, lib_id).await.unwrap(), 1);

    let expected = first_book.min(duplicate_id);
    let linked: Uuid =
        sqlx::query_scalar("SELECT book_id FROM external_book_metadata WHERE id = $1")
            .bind(external_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(linked, expected);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn rematch_skips_volumes_already_linked_in_the_same_link(pool: sqlx::PgPool) {
    let (lib_id, series_id) = seed_library_and_series(&pool).await;
    let book_id = seed_book(&pool, lib_id, series_id, 1).await;
    let link_id = seed_link(&pool, lib_id, series_id).await;

    seed_external_book(&pool, link_id, "1", 1, Some(book_id)).await;
    seed_external_book(&pool, link_id, "2", 1, None).await;

    assert_eq!(rematch_unlinked_books(&pool, lib_id).await.unwrap(), 0);

    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_book_metadata WHERE book_id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(total, 1);
}
