//! One test per `links` transition, plus the one-primary invariant.

use axum::http::StatusCode;
use sqlx::PgPool;
use uuid::Uuid;

use super::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

async fn seed_series(pool: &PgPool, name: &str) -> (Uuid, Uuid) {
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(library_id)
        .bind(format!("lib-{name}"))
        .bind(format!("/libraries/{name}"))
        .execute(pool)
        .await
        .unwrap();
    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
        .bind(series_id)
        .bind(library_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    (library_id, series_id)
}

/// Seed a link directly; `approved_at` grows with `age_rank` so the oldest
/// approved link is deterministic.
#[allow(clippy::too_many_arguments)]
async fn seed_link(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Uuid,
    provider: &str,
    status: &str,
    is_primary: bool,
    description: &str,
    age_rank: i32,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO external_metadata_links \
             (library_id, series_id, provider, external_id, status, is_primary, metadata_json, approved_at) \
         VALUES ($1, $2, $3, 'ext', $4, $5, $6, \
                 CASE WHEN $4 = 'approved' THEN '2020-01-01'::timestamptz + make_interval(days => $7) END) \
         RETURNING id",
    )
    .bind(library_id)
    .bind(series_id)
    .bind(provider)
    .bind(status)
    .bind(is_primary)
    .bind(serde_json::json!({ "description": description }))
    .bind(age_rank)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[derive(Debug, sqlx::FromRow)]
struct LinkRow {
    status: String,
    is_primary: bool,
    external_id: String,
    external_url: Option<String>,
    provider_rating: Option<f32>,
    provider_rating_count: Option<i32>,
    approved_at: Option<chrono::DateTime<chrono::Utc>>,
    synced_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn link(pool: &PgPool, id: Uuid) -> LinkRow {
    sqlx::query_as(
        "SELECT status, is_primary, external_id, external_url, provider_rating, \
                provider_rating_count, approved_at, synced_at \
         FROM external_metadata_links WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn primaries(pool: &PgPool, series_id: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar(
        "SELECT id FROM external_metadata_links WHERE series_id = $1 AND is_primary ORDER BY id",
    )
    .bind(series_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn description(pool: &PgPool, series_id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT description FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn candidate<'a>(
    library_id: Uuid,
    series_id: Uuid,
    provider: &'a str,
    external_id: &'a str,
    metadata_json: &'a serde_json::Value,
) -> LinkCandidate<'a> {
    LinkCandidate {
        library_id,
        series_id,
        provider,
        external_id,
        external_url: Some("https://provider/x"),
        confidence: Some(0.9),
        metadata_json,
        total_volumes: Some(12),
    }
}

// ---------------------------------------------------------------------------
// Notes and errors
// ---------------------------------------------------------------------------

#[test]
fn provider_notes_ignore_non_positive_ratings() {
    let notes = ProviderNotes::from_metadata(&serde_json::json!({
        "rating": 0.0, "rating_count": 3, "rating_scale": 5.0
    }));
    assert_eq!(notes.rating, None);
    assert_eq!(notes.rating_count, Some(3));
    assert_eq!(notes.rating_scale, Some(5.0));

    let notes = ProviderNotes::from_metadata(&serde_json::json!({ "rating": 4.5 }));
    assert_eq!(notes.rating, Some(4.5));
}

#[test]
fn link_errors_map_to_api_statuses() {
    assert_eq!(
        ApiError::from(LinkError::NotFound).status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        ApiError::from(LinkError::NotApproved).status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        ApiError::from(LinkError::Sync(ApiError::not_found("series not found"))).status,
        StatusCode::NOT_FOUND
    );
}

// ---------------------------------------------------------------------------
// propose
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn propose_creates_a_pending_link_with_notes(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "propose").await;
    let json = serde_json::json!({ "rating": 4.0, "rating_count": 10 });

    let id = propose(&pool, &candidate(lib, series, "bdphile", "b-1", &json))
        .await
        .unwrap();

    let row = link(&pool, id).await;
    assert_eq!(row.status, "pending");
    assert!(!row.is_primary);
    assert_eq!(row.provider_rating, Some(4.0));
    assert_eq!(row.provider_rating_count, Some(10));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn propose_over_the_approved_primary_demotes_it_and_promotes_the_next(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "repropose").await;
    let a = seed_link(
        &pool,
        lib,
        series,
        "google_books",
        "approved",
        true,
        "from A",
        1,
    )
    .await;
    let b = seed_link(
        &pool, lib, series, "bdtheque", "approved", false, "from B", 2,
    )
    .await;
    sqlx::query("UPDATE external_metadata_links SET synced_at = NOW() WHERE id = $1")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();

    let json = serde_json::json!({ "description": "new A" });
    let id = propose(&pool, &candidate(lib, series, "google_books", "a-2", &json))
        .await
        .unwrap();
    assert_eq!(id, a, "the upsert keeps the row");

    let row = link(&pool, a).await;
    assert_eq!(row.status, "pending");
    assert!(!row.is_primary, "a pending link is never primary");
    assert_eq!(row.external_id, "a-2");
    assert!(row.approved_at.is_none() && row.synced_at.is_none());

    assert_eq!(primaries(&pool, series).await, vec![b]);
    assert_eq!(description(&pool, series).await.as_deref(), Some("from B"));
}

// ---------------------------------------------------------------------------
// approve
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn approve_first_link_becomes_primary_second_is_secondary(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "approve").await;
    let json = serde_json::json!({});

    let first = approve(
        &pool,
        &candidate(lib, series, "google_books", "g", &json),
        OnConflict::Replace,
    )
    .await
    .unwrap();
    let second = approve(
        &pool,
        &candidate(lib, series, "bdtheque", "b", &json),
        OnConflict::Replace,
    )
    .await
    .unwrap();

    assert_eq!(primaries(&pool, series).await, vec![first]);
    let row = link(&pool, second).await;
    assert_eq!(row.status, "approved");
    assert!(row.approved_at.is_some());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn approve_replace_overwrites_provider_fields_and_keeps_the_primary(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "approve_conflict").await;
    let a = seed_link(
        &pool,
        lib,
        series,
        "senscritique",
        "approved",
        true,
        "old",
        1,
    )
    .await;

    let json = serde_json::json!({ "rating": 3.5, "rating_count": 7 });
    let id = approve(
        &pool,
        &candidate(lib, series, "senscritique", "sc-new", &json),
        OnConflict::Replace,
    )
    .await
    .unwrap();

    assert_eq!(id, a);
    let row = link(&pool, a).await;
    assert_eq!(row.external_id, "sc-new");
    assert_eq!(row.external_url.as_deref(), Some("https://provider/x"));
    assert_eq!(row.provider_rating, Some(3.5));
    assert_eq!(row.provider_rating_count, Some(7));
    assert!(row.is_primary, "an approved primary stays primary");
}

/// Regression: series creation kept the stored identity but overwrote its
/// data with the new entity's, mixing two provider entries on one row.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn approve_keep_existing_keeps_identity_and_its_data(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "approve_keep").await;
    let a = seed_link(
        &pool,
        lib,
        series,
        "senscritique",
        "pending",
        false,
        "original data",
        1,
    )
    .await;

    let json = serde_json::json!({ "description": "other entity", "rating": 3.5 });
    let id = approve(
        &pool,
        &candidate(lib, series, "senscritique", "sc-other", &json),
        OnConflict::KeepExisting,
    )
    .await
    .unwrap();

    assert_eq!(id, a);
    let row = link(&pool, a).await;
    assert_eq!(row.status, "approved");
    assert_eq!(row.external_id, "ext", "the stored identity is kept");
    assert_eq!(
        row.provider_rating, None,
        "the other entity's notes are not mixed in"
    );
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT metadata_json FROM external_metadata_links WHERE id = $1")
            .bind(a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored["description"], "original data");
    assert_eq!(primaries(&pool, series).await, vec![a]);
}

/// A legacy pending row still flagged primary must not create a second
/// approved primary when it is approved again.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn approve_drops_a_stale_primary_flag_on_a_pending_row(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "approve_stale").await;
    let primary = seed_link(&pool, lib, series, "google_books", "approved", true, "p", 1).await;
    let stale = seed_link(&pool, lib, series, "bdtheque", "pending", true, "s", 2).await;

    let json = serde_json::json!({});
    approve(
        &pool,
        &candidate(lib, series, "bdtheque", "b", &json),
        OnConflict::Replace,
    )
    .await
    .unwrap();

    assert_eq!(primaries(&pool, series).await, vec![primary]);
    assert_eq!(link(&pool, stale).await.status, "approved");
}

// ---------------------------------------------------------------------------
// approve_existing / set_primary
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn approve_existing_promotes_only_without_a_primary(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "approve_existing").await;
    let a = seed_link(&pool, lib, series, "google_books", "pending", false, "a", 1).await;
    let b = seed_link(&pool, lib, series, "bdtheque", "pending", false, "b", 2).await;

    let approved = approve_existing(&pool, a, false).await.unwrap();
    assert_eq!(approved.series_id, series);
    assert_eq!(approved.library_id, lib);
    assert_eq!(approved.provider, "google_books");
    approve_existing(&pool, b, false).await.unwrap();
    assert_eq!(primaries(&pool, series).await, vec![a]);

    // Forcing the primary switches it.
    approve_existing(&pool, b, true).await.unwrap();
    assert_eq!(primaries(&pool, series).await, vec![b]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn approve_existing_unknown_link_is_not_found(pool: PgPool) {
    let err = approve_existing(&pool, Uuid::new_v4(), false)
        .await
        .unwrap_err();
    assert!(matches!(err, LinkError::NotFound));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn set_primary_switches_and_refuses_unapproved_links(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "set_primary").await;
    let a = seed_link(&pool, lib, series, "google_books", "approved", true, "a", 1).await;
    let b = seed_link(&pool, lib, series, "bdtheque", "approved", false, "b", 2).await;
    let pending = seed_link(&pool, lib, series, "comicvine", "pending", false, "c", 3).await;

    assert_eq!(set_primary(&pool, b).await.unwrap(), series);
    assert_eq!(primaries(&pool, series).await, vec![b]);

    let err = set_primary(&pool, pending).await.unwrap_err();
    assert!(matches!(err, LinkError::NotApproved));
    assert_eq!(
        primaries(&pool, series).await,
        vec![b],
        "a refusal changes nothing"
    );

    let err = set_primary(&pool, Uuid::new_v4()).await.unwrap_err();
    assert!(matches!(err, LinkError::NotFound));
    let _ = a;
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn unique_index_blocks_two_approved_primaries(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "unique_index").await;
    seed_link(&pool, lib, series, "google_books", "approved", true, "a", 1).await;
    let b = seed_link(&pool, lib, series, "bdtheque", "approved", false, "b", 2).await;

    let err = sqlx::query("UPDATE external_metadata_links SET is_primary = true WHERE id = $1")
        .bind(b)
        .execute(&pool)
        .await;
    assert!(
        err.is_err(),
        "two approved primaries must violate the index"
    );
}

// ---------------------------------------------------------------------------
// remove: one after-removal policy for delete, reject and rematch cleanup
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_primary_promotes_the_oldest_and_resyncs(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "delete_primary").await;
    let a = seed_link(
        &pool,
        lib,
        series,
        "google_books",
        "approved",
        true,
        "from A",
        1,
    )
    .await;
    let c = seed_link(
        &pool,
        lib,
        series,
        "comicvine",
        "approved",
        false,
        "from C",
        3,
    )
    .await;
    let b = seed_link(
        &pool, lib, series, "bdtheque", "approved", false, "from B", 2,
    )
    .await;

    let after = remove(&pool, Removal::Delete(a)).await.unwrap();

    assert_eq!(
        after,
        AfterRemoval {
            series_id: series,
            removed: 1,
            promoted: Some(b),
            series_metadata: SeriesMetadata::Resynced,
        }
    );
    assert_eq!(primaries(&pool, series).await, vec![b]);
    assert_eq!(description(&pool, series).await.as_deref(), Some("from B"));
    let _ = c;
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_last_approved_link_clears_series_fields(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "delete_last").await;
    let a = seed_link(
        &pool,
        lib,
        series,
        "google_books",
        "approved",
        true,
        "from A",
        1,
    )
    .await;
    sqlx::query("UPDATE series SET description = 'from A' WHERE id = $1")
        .bind(series)
        .execute(&pool)
        .await
        .unwrap();

    let after = remove(&pool, Removal::Delete(a)).await.unwrap();

    assert_eq!(after.series_metadata, SeriesMetadata::Cleared);
    assert_eq!(after.promoted, None);
    assert_eq!(description(&pool, series).await, None);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_pending_link_leaves_series_untouched(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "delete_pending").await;
    let pending = seed_link(&pool, lib, series, "google_books", "pending", false, "p", 1).await;
    sqlx::query("UPDATE series SET description = 'manual' WHERE id = $1")
        .bind(series)
        .execute(&pool)
        .await
        .unwrap();

    let after = remove(&pool, Removal::Delete(pending)).await.unwrap();

    assert_eq!(after.series_metadata, SeriesMetadata::Untouched);
    assert_eq!(description(&pool, series).await.as_deref(), Some("manual"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_or_reject_unknown_link_is_not_found(pool: PgPool) {
    for removal in [
        Removal::Delete(Uuid::new_v4()),
        Removal::Reject(Uuid::new_v4()),
    ] {
        let err = remove(&pool, removal).await.unwrap_err();
        assert!(matches!(err, LinkError::NotFound));
    }
}

/// Reject used to promote without re-merging: the series kept the fields of
/// the rejected primary.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn reject_primary_keeps_the_row_promotes_and_resyncs(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "reject_primary").await;
    let a = seed_link(
        &pool,
        lib,
        series,
        "google_books",
        "approved",
        true,
        "from A",
        1,
    )
    .await;
    let b = seed_link(
        &pool, lib, series, "bdtheque", "approved", false, "from B", 2,
    )
    .await;
    sqlx::query("UPDATE series SET description = 'from A' WHERE id = $1")
        .bind(series)
        .execute(&pool)
        .await
        .unwrap();

    let after = remove(&pool, Removal::Reject(a)).await.unwrap();

    assert_eq!(after.promoted, Some(b));
    assert_eq!(after.series_metadata, SeriesMetadata::Resynced);
    let row = link(&pool, a).await;
    assert_eq!(row.status, "rejected");
    assert!(!row.is_primary);
    assert_eq!(description(&pool, series).await.as_deref(), Some("from B"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn reject_pending_link_changes_nothing_else(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "reject_pending").await;
    let a = seed_link(
        &pool,
        lib,
        series,
        "google_books",
        "approved",
        true,
        "from A",
        1,
    )
    .await;
    let p = seed_link(&pool, lib, series, "bdtheque", "pending", false, "p", 2).await;

    let after = remove(&pool, Removal::Reject(p)).await.unwrap();

    assert_eq!(after.series_metadata, SeriesMetadata::Untouched);
    assert_eq!(after.promoted, None);
    assert_eq!(primaries(&pool, series).await, vec![a]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn providers_not_in_drops_unconfigured_links_and_resyncs(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "rematch").await;
    seed_link(
        &pool,
        lib,
        series,
        "bedetheque",
        "approved",
        true,
        "stale",
        1,
    )
    .await;
    let kept = seed_link(
        &pool,
        lib,
        series,
        "senscritique",
        "approved",
        false,
        "fresh",
        2,
    )
    .await;
    sqlx::query("UPDATE series SET description = 'stale' WHERE id = $1")
        .bind(series)
        .execute(&pool)
        .await
        .unwrap();

    let keep = vec!["senscritique".to_string()];
    let after = remove(
        &pool,
        Removal::ProvidersNotIn {
            series_id: series,
            keep: &keep,
        },
    )
    .await
    .unwrap();

    assert_eq!(after.removed, 1);
    assert_eq!(after.promoted, Some(kept));
    assert_eq!(after.series_metadata, SeriesMetadata::Resynced);
    assert_eq!(description(&pool, series).await.as_deref(), Some("fresh"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn providers_not_in_with_nothing_to_drop_is_a_noop(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "rematch_noop").await;
    seed_link(
        &pool,
        lib,
        series,
        "senscritique",
        "approved",
        true,
        "kept",
        1,
    )
    .await;

    let keep = vec!["senscritique".to_string()];
    let after = remove(
        &pool,
        Removal::ProvidersNotIn {
            series_id: series,
            keep: &keep,
        },
    )
    .await
    .unwrap();

    assert_eq!(after.removed, 0);
    assert_eq!(after.series_metadata, SeriesMetadata::Untouched);
}

// ---------------------------------------------------------------------------
// reassign (series merge)
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn reassign_keeps_one_primary_and_drops_duplicate_providers(pool: PgPool) {
    let (lib, target) = seed_series(&pool, "reassign").await;
    let source = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Source')")
        .bind(source)
        .bind(lib)
        .execute(&pool)
        .await
        .unwrap();

    let target_primary =
        seed_link(&pool, lib, target, "google_books", "approved", true, "t", 1).await;
    seed_link(
        &pool,
        lib,
        source,
        "google_books",
        "approved",
        false,
        "dup",
        2,
    )
    .await;
    let moved = seed_link(&pool, lib, source, "bdtheque", "approved", true, "s", 3).await;

    let mut tx = pool.begin().await.unwrap();
    let count = reassign(&mut tx, source, target).await.unwrap();
    tx.commit().await.unwrap();

    assert_eq!(count, 1);
    assert_eq!(primaries(&pool, target).await, vec![target_primary]);
    assert!(!link(&pool, moved).await.is_primary);
    let left: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(left, 0);
}

// ---------------------------------------------------------------------------
// Refresh bookkeeping
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn record_provider_snapshot_and_mark_synced(pool: PgPool) {
    let (lib, series) = seed_series(&pool, "snapshot").await;
    let a = seed_link(&pool, lib, series, "bdphile", "approved", true, "a", 1).await;

    let json = serde_json::json!({ "rating": 4.2, "rating_count": 99 });
    record_provider_snapshot(&pool, a, &json, Some(20))
        .await
        .unwrap();
    mark_synced(&pool, a).await.unwrap();

    let row = link(&pool, a).await;
    assert_eq!(row.provider_rating, Some(4.2));
    assert_eq!(row.provider_rating_count, Some(99));
    assert!(row.synced_at.is_some());

    let err = record_provider_snapshot(&pool, Uuid::new_v4(), &json, None)
        .await
        .unwrap_err();
    assert!(matches!(err, LinkError::NotFound));
}
