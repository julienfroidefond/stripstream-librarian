//! Tests for `GET /genres/browse`: query parsing (unit) and the browse SQL
//! (`#[sqlx::test]`, ephemeral migrated database).

use sqlx::PgPool;
use uuid::Uuid;

use super::*;

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn query() -> GenreBrowseQuery {
    GenreBrowseQuery::from_pairs(Vec::new()).unwrap()
}

fn names(page: &GenreBrowsePage) -> Vec<&str> {
    page.items.iter().map(|s| s.name.as_str()).collect()
}

/// Names regardless of order (accented names sort differently across collations).
fn sorted_names(page: &GenreBrowsePage) -> Vec<&str> {
    let mut names = names(page);
    names.sort_unstable();
    names
}

fn facet<'a>(page: &'a GenreBrowsePage, name: &str) -> Option<&'a GenreFacet> {
    page.facets.genres.iter().find(|g| g.name == name)
}

async fn create_library(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO libraries (id, name, root_path) VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(name)
    .bind(format!("/libraries/{}-{}", name, Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_series(pool: &PgPool, library_id: Uuid, name: &str, genres: &[&str]) -> Uuid {
    let genres: Vec<String> = genres.iter().map(|g| g.to_string()).collect();
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, genres) VALUES (gen_random_uuid(), $1, $2, $3) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .bind(genres)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_book(pool: &PgPool, library_id: Uuid, series_id: Uuid, volume: i32) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO books (id, library_id, kind, title, series_id, volume, volume_type, authors, page_count) \
         VALUES (gen_random_uuid(), $1, 'comic', $2, $3, $4, 'regular', '{}', 10) RETURNING id",
    )
    .bind(library_id)
    .bind(format!("T{volume}"))
    .bind(series_id)
    .bind(volume)
    .fetch_one(pool)
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// Query parsing
// ---------------------------------------------------------------------------

#[test]
fn parses_repeated_and_bracketed_keys() {
    let lib = Uuid::new_v4();
    let parsed = GenreBrowseQuery::from_pairs(pairs(&[
        ("genres", "Action"),
        ("genres[]", "Drama"),
        ("genres", "Action"),
        ("exclude[]", "Horror"),
        ("library_ids", &lib.to_string()),
        ("library_ids[]", &lib.to_string()),
        ("untagged", "false"),
        ("q", "  dragon "),
        ("page", "3"),
        ("limit", "10"),
        ("unknown", "ignored"),
    ]))
    .unwrap();

    assert_eq!(
        parsed,
        GenreBrowseQuery {
            genres: vec!["Action".into(), "Drama".into()],
            exclude: vec!["Horror".into()],
            library_ids: vec![lib],
            untagged: false,
            q: Some("dragon".into()),
            page: 3,
            limit: 10,
        }
    );
}

#[test]
fn defaults_and_clamps() {
    assert_eq!(query().page, 1);
    assert_eq!(query().limit, 100);
    let parsed = GenreBrowseQuery::from_pairs(pairs(&[
        ("page", "0"),
        ("limit", "9999"),
        ("q", "   "),
        ("genres", " "),
    ]))
    .unwrap();
    assert_eq!(parsed.page, 1);
    assert_eq!(parsed.limit, 500);
    assert_eq!(parsed.q, None);
    assert!(parsed.genres.is_empty());
}

#[test]
fn rejects_invalid_values() {
    assert!(GenreBrowseQuery::from_pairs(pairs(&[("library_ids", "nope")])).is_err());
    assert!(GenreBrowseQuery::from_pairs(pairs(&[("untagged", "maybe")])).is_err());
    assert!(GenreBrowseQuery::from_pairs(pairs(&[("page", "x")])).is_err());
}

#[test]
fn huge_page_is_rejected_instead_of_overflowing_the_offset() {
    let max =
        GenreBrowseQuery::from_pairs(pairs(&[("page", &MAX_PAGE.to_string()), ("limit", "500")]))
            .unwrap();
    assert_eq!(max.offset(), (MAX_PAGE - 1) * 500);

    for page in [&(MAX_PAGE + 1).to_string(), "9223372036854775807"] {
        assert!(GenreBrowseQuery::from_pairs(pairs(&[("page", page)])).is_err());
    }

    // `offset` never wraps even if a query is built by hand.
    let forged = GenreBrowseQuery {
        page: i64::MAX,
        limit: MAX_LIMIT,
        ..Default::default()
    };
    assert_eq!(forged.offset(), i64::MAX);
}

// ---------------------------------------------------------------------------
// SQL
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn genres_are_combined_with_and_and_exclusions_apply(pool: PgPool) {
    let lib = create_library(&pool, "main").await;
    create_series(&pool, lib, "Both", &["Action", "Drama"]).await;
    create_series(&pool, lib, "Action only", &["Action"]).await;
    create_series(&pool, lib, "Both scary", &["Action", "Drama", "Horror"]).await;
    create_series(&pool, lib, "Untagged", &[]).await;

    let mut q = query();
    q.genres = vec!["Action".into(), "Drama".into()];
    let page = browse(&pool, &q).await.unwrap();
    assert_eq!(names(&page), vec!["Both", "Both scary"]);
    assert_eq!(page.total, 2);

    q.exclude = vec!["Horror".into()];
    let page = browse(&pool, &q).await.unwrap();
    assert_eq!(names(&page), vec!["Both"]);

    // Exclusion alone keeps untagged series.
    let mut q = query();
    q.exclude = vec!["Drama".into()];
    let page = browse(&pool, &q).await.unwrap();
    assert_eq!(names(&page), vec!["Action only", "Untagged"]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn untagged_name_and_library_filters_combine(pool: PgPool) {
    let first = create_library(&pool, "first").await;
    let second = create_library(&pool, "second").await;
    create_series(&pool, first, "Éclair", &[]).await;
    create_series(&pool, first, "Orage", &[]).await;
    create_series(&pool, first, "Eclipse", &["Action"]).await;
    create_series(&pool, second, "Ecume", &[]).await;

    let mut q = query();
    q.untagged = true;
    q.library_ids = vec![first];
    let page = browse(&pool, &q).await.unwrap();
    assert_eq!(sorted_names(&page), vec!["Orage", "Éclair"]);

    q.q = Some("ecl".into());
    let page = browse(&pool, &q).await.unwrap();
    assert_eq!(names(&page), vec!["Éclair"]);

    q.library_ids = vec![first, second];
    q.q = Some("ec".into());
    let page = browse(&pool, &q).await.unwrap();
    assert_eq!(sorted_names(&page), vec!["Ecume", "Éclair"]);

    // Untagged combined with a genre can never match.
    q.genres = vec!["Action".into()];
    let page = browse(&pool, &q).await.unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.total, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn paginates_beyond_the_old_client_cap(pool: PgPool) {
    let lib = create_library(&pool, "main").await;
    for i in 0..7 {
        create_series(&pool, lib, &format!("S{i}"), &["Action"]).await;
    }

    let mut q = query();
    q.genres = vec!["Action".into()];
    q.limit = 3;
    q.page = 3;
    let page = browse(&pool, &q).await.unwrap();
    assert_eq!(names(&page), vec!["S6"]);
    assert_eq!(page.total, 7);
    assert_eq!((page.page, page.limit), (3, 3));

    // An empty page still reports the total and the facets.
    q.page = 4;
    let page = browse(&pool, &q).await.unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.total, 7);
    assert_eq!(page.facets.total, 7);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn facets_follow_library_scope_only(pool: PgPool) {
    let first = create_library(&pool, "first").await;
    let second = create_library(&pool, "second").await;
    create_series(&pool, first, "A", &["Action", "Drama"]).await;
    create_series(&pool, first, "B", &["Action"]).await;
    create_series(&pool, first, "C", &[]).await;
    create_series(&pool, second, "D", &["Action", "Horror"]).await;
    create_series(&pool, second, "E", &[]).await;

    // Genre, exclusion and name filters do not change the facets.
    let mut q = query();
    q.genres = vec!["Drama".into()];
    q.exclude = vec!["Horror".into()];
    q.q = Some("zzz".into());
    let page = browse(&pool, &q).await.unwrap();
    assert_eq!(page.facets.total, 5);
    assert_eq!(page.facets.untagged, 2);
    let facet_names: Vec<&str> = page.facets.genres.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(facet_names, vec!["Action", "Drama", "Horror"]);
    assert_eq!(facet(&page, "Action").unwrap().series_count, 3);

    // The library scope does.
    let mut q = query();
    q.library_ids = vec![first];
    let page = browse(&pool, &q).await.unwrap();
    assert_eq!(page.facets.total, 3);
    assert_eq!(page.facets.untagged, 1);
    assert_eq!(facet(&page, "Action").unwrap().series_count, 2);
    assert!(facet(&page, "Horror").is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn facets_carry_a_cover_preferring_series_with_an_image(pool: PgPool) {
    let lib = create_library(&pool, "main").await;
    create_series(&pool, lib, "Aaa no image", &["Action"]).await;
    let with_book = create_series(&pool, lib, "Zzz with book", &["Action"]).await;
    let book = create_book(&pool, lib, with_book, 1).await;
    create_series(&pool, lib, "Bare", &["Drama"]).await;

    let page = browse(&pool, &query()).await.unwrap();

    let cover = facet(&page, "Action").unwrap().cover.as_ref().unwrap();
    assert_eq!(cover.series_id, with_book);
    assert_eq!(cover.first_book_id, Some(book));
    assert!(facet(&page, "Drama").unwrap().cover.is_none());

    let item = page
        .items
        .iter()
        .find(|s| s.series_id == with_book)
        .unwrap();
    assert_eq!(item.book_count, 1);
    assert_eq!(item.first_book_id, Some(book));
}
