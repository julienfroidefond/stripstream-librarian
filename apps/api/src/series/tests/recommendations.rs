//! Tests for the personalised series recommendation engine.
//!
//! `get_recommendations` is a single large SQL query whose scoring bands,
//! exclusions and bonus rules are easy to regress. These tests pin them down
//! against a real migrated temporary database.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::Extension;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::series::recommendations::{
    get_recommendations, RecommendationsQuery, RecommendedSeriesItem,
};
use crate::state::test_state;

use super::{create_book, create_test_library};

// ── helpers ─────────────────────────────────────────────────────────────────

async fn create_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
    create_test_library(pool, name).await
}

async fn create_series(
    pool: &sqlx::PgPool,
    lib_id: Uuid,
    name: &str,
    authors: &[&str],
    genres: &[&str],
    publishers: &[&str],
) -> Uuid {
    let authors: Vec<String> = authors.iter().map(|s| s.to_string()).collect();
    let genres: Vec<String> = genres.iter().map(|s| s.to_string()).collect();
    let publishers: Vec<String> = publishers.iter().map(|s| s.to_string()).collect();

    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO series (id, library_id, name, authors, genres, publishers) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(lib_id)
    .bind(name)
    .bind(&authors)
    .bind(&genres)
    .bind(&publishers)
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

/// Record reading progress for `book` (which puts its series in the source set).
async fn mark_progress(pool: &sqlx::PgPool, book: Uuid, user: Uuid, status: &str) {
    sqlx::query(
        "INSERT INTO book_reading_progress (book_id, user_id, status, last_read_at, updated_at) \
         VALUES ($1, $2, $3, NOW(), NOW())",
    )
    .bind(book)
    .bind(user)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

async fn call(
    pool: &sqlx::PgPool,
    user: Option<Uuid>,
    sources: Option<i64>,
    limit: Option<i64>,
) -> Vec<RecommendedSeriesItem> {
    let state = test_state(pool.clone());
    let auth = user.map(|user_id| Extension(AuthUser { user_id }));

    get_recommendations(
        State(state),
        auth,
        Query(RecommendationsQuery { sources, limit }),
    )
    .await
    .unwrap()
    .0
}

/// Index the results by series name for order-independent assertions.
fn by_name(items: &[RecommendedSeriesItem]) -> HashMap<String, &RecommendedSeriesItem> {
    items.iter().map(|item| (item.name.clone(), item)).collect()
}

// ── tests ───────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn recommendations_are_empty_without_an_authenticated_user(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "reco_anon").await;
    let src = create_series(&pool, lib, "Source", &["Alice"], &["Fantasy"], &[]).await;
    let book = create_book(&pool, lib, src, "T1").await;
    let user = create_user(&pool, "reco_anon_user").await;
    mark_progress(&pool, book, user, "read").await;

    let items = call(&pool, None, None, None).await;
    assert!(
        items.is_empty(),
        "recommendations are personal and require a user"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn recommendations_weight_authors_above_common_genres(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "reco_weight").await;
    let src = create_series(&pool, lib, "Source", &["Alice"], &["Mid"], &[]).await;
    let book = create_book(&pool, lib, src, "T1").await;
    let user = create_user(&pool, "reco_weight_user").await;
    mark_progress(&pool, book, user, "read").await;

    // Six series share the "Mid" genre, pushing its frequency band to 3 points.
    for i in 0..5 {
        create_series(&pool, lib, &format!("Filler {i}"), &[], &["Mid"], &[]).await;
    }

    let author_match = create_series(&pool, lib, "AuthorMatch", &["Alice"], &[], &[]).await;
    let genre_match = create_series(&pool, lib, "GenreMatch", &[], &["Mid"], &[]).await;
    create_book(&pool, lib, author_match, "T1").await;
    create_book(&pool, lib, genre_match, "T1").await;

    let items = call(&pool, Some(user), None, None).await;
    let index = by_name(&items);

    let author = index.get("AuthorMatch").expect("author match missing");
    let genre = index.get("GenreMatch").expect("genre match missing");

    assert_eq!(author.match_reasons, vec!["same_author".to_string()]);
    assert_eq!(genre.match_reasons, vec!["same_genre".to_string()]);

    // source_weight = 6 (read) + (5 - 1) = 10; a shared author scores 8, and a
    // genre shared by 7 series lands in the `<= 15` band, weighted at 3.
    let source_weight = 10;
    assert_eq!(author.author_pts, 8 * source_weight);
    assert_eq!(author.genre_pts, 0);
    assert_eq!(genre.author_pts, 0);
    assert_eq!(genre.genre_pts, 3 * 2 * source_weight);

    assert!(
        author.score > genre.score,
        "a shared author must outweigh a common shared genre ({} vs {})",
        author.score,
        genre.score
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn recommendations_exclude_series_the_user_already_started(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "reco_started").await;
    let src = create_series(&pool, lib, "Source", &["Alice"], &[], &[]).await;
    let book = create_book(&pool, lib, src, "T1").await;
    let user = create_user(&pool, "reco_started_user").await;
    mark_progress(&pool, book, user, "read").await;

    let started = create_series(&pool, lib, "Started", &["Alice"], &[], &[]).await;
    let started_book = create_book(&pool, lib, started, "T1").await;
    mark_progress(&pool, started_book, user, "reading").await;

    let fresh = create_series(&pool, lib, "Fresh", &["Alice"], &[], &[]).await;
    create_book(&pool, lib, fresh, "T1").await;

    let items = call(&pool, Some(user), None, None).await;
    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();

    assert!(names.contains(&"Fresh"), "a new series must be recommended");
    assert!(
        !names.contains(&"Started"),
        "a series the user already started must be excluded"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn recommendations_honour_genre_restrictions(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "reco_restrict").await;
    let src = create_series(&pool, lib, "Source", &["Alice"], &["Fantasy"], &[]).await;
    let book = create_book(&pool, lib, src, "T1").await;
    let user = create_user(&pool, "reco_restrict_user").await;
    mark_progress(&pool, book, user, "read").await;

    let candidate = create_series(&pool, lib, "Restricted", &[], &["Fantasy"], &[]).await;
    create_book(&pool, lib, candidate, "T1").await;

    // Without the restriction the genre match is recommended ...
    let before = call(&pool, Some(user), None, None).await;
    assert!(by_name(&before).contains_key("Restricted"));

    sqlx::query("INSERT INTO user_genre_restrictions (user_id, genre) VALUES ($1, 'Fantasy')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();

    // ... and disappears once the user restricts that genre.
    let after = call(&pool, Some(user), None, None).await;
    assert!(
        by_name(&after).is_empty(),
        "a restricted genre must be filtered out of the recommendations"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn recommendations_are_boosted_by_a_shared_reading_list(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "reco_list").await;
    let src = create_series(&pool, lib, "Source", &["Alice"], &[], &[]).await;
    let book = create_book(&pool, lib, src, "T1").await;
    let user = create_user(&pool, "reco_list_user").await;
    mark_progress(&pool, book, user, "read").await;

    // The candidate shares no author/genre/publisher: only the reading list.
    let candidate = create_series(&pool, lib, "ListMate", &["Zed"], &["Western"], &[]).await;
    create_book(&pool, lib, candidate, "T1").await;

    let list_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO reading_lists (id, name) VALUES (gen_random_uuid(), 'Shared') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for (position, series_id) in [(0, src), (1, candidate)] {
        sqlx::query(
            "INSERT INTO reading_list_items (list_id, series_id, position) VALUES ($1, $2, $3)",
        )
        .bind(list_id)
        .bind(series_id)
        .bind(position)
        .execute(&pool)
        .await
        .unwrap();
    }

    let items = call(&pool, Some(user), None, None).await;
    let index = by_name(&items);
    let mate = index.get("ListMate").expect("reading-list mate missing");

    assert_eq!(mate.match_reasons, vec!["same_reading_list".to_string()]);
    assert!(mate.reading_list_pts > 0);
    assert_eq!(mate.because_of, vec!["Source".to_string()]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn recommendations_add_a_community_score_bonus(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "reco_community").await;
    let src = create_series(&pool, lib, "Source", &["Alice"], &[], &[]).await;
    let book = create_book(&pool, lib, src, "T1").await;
    let user = create_user(&pool, "reco_community_user").await;
    mark_progress(&pool, book, user, "read").await;

    let candidate = create_series(&pool, lib, "Hype", &["Alice"], &[], &[]).await;
    create_book(&pool, lib, candidate, "T1").await;

    // An approved link rated 8/10 => community score 4.0 => +40 bonus.
    sqlx::query(
        "INSERT INTO external_metadata_links \
             (library_id, series_id, provider, external_id, status, provider_rating, provider_rating_scale, is_primary) \
         VALUES ($1, $2, 'google_books', 'ext', 'approved', 8.0, 10.0, TRUE)",
    )
    .bind(lib)
    .bind(candidate)
    .execute(&pool)
    .await
    .unwrap();

    let items = call(&pool, Some(user), None, None).await;
    let index = by_name(&items);
    let hype = index.get("Hype").expect("candidate missing");

    assert_eq!(hype.community_bonus, 40);
    assert_eq!(hype.metadata_provider.as_deref(), Some("google_books"));
    let score = hype.community_score.expect("community score missing");
    assert!(
        (score - 4.0).abs() < 1e-4,
        "unexpected community score {score}"
    );
    assert_eq!(hype.score, hype.similarity_score + hype.community_bonus);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn recommendations_respect_the_limit(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "reco_limit").await;
    let src = create_series(&pool, lib, "Source", &["Alice"], &[], &[]).await;
    let book = create_book(&pool, lib, src, "T1").await;
    let user = create_user(&pool, "reco_limit_user").await;
    mark_progress(&pool, book, user, "read").await;

    for i in 0..4 {
        let candidate =
            create_series(&pool, lib, &format!("Candidate {i}"), &["Alice"], &[], &[]).await;
        create_book(&pool, lib, candidate, "T1").await;
    }

    let items = call(&pool, Some(user), None, Some(2)).await;
    assert_eq!(items.len(), 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn recommendations_only_use_the_n_most_recent_sources(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "reco_sources").await;
    let user = create_user(&pool, "reco_sources_user").await;

    let recent = create_series(&pool, lib, "Recent", &["Alice"], &[], &[]).await;
    let recent_book = create_book(&pool, lib, recent, "T1").await;
    mark_progress(&pool, recent_book, user, "read").await;

    let old = create_series(&pool, lib, "Old", &["Bob"], &[], &[]).await;
    let old_book = create_book(&pool, lib, old, "T1").await;
    sqlx::query(
        "INSERT INTO book_reading_progress (book_id, user_id, status, last_read_at, updated_at) \
         VALUES ($1, $2, 'read', NOW() - INTERVAL '30 days', NOW())",
    )
    .bind(old_book)
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();

    let from_recent = create_series(&pool, lib, "FromRecent", &["Alice"], &[], &[]).await;
    let from_old = create_series(&pool, lib, "FromOld", &["Bob"], &[], &[]).await;
    create_book(&pool, lib, from_recent, "T1").await;
    create_book(&pool, lib, from_old, "T1").await;

    // Only one source: the most recently read series ("Recent").
    let items = call(&pool, Some(user), Some(1), None).await;
    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();

    assert!(names.contains(&"FromRecent"));
    assert!(
        !names.contains(&"FromOld"),
        "only the most recent source must be used when sources = 1"
    );
}
