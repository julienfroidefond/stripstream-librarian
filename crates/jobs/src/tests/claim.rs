use sqlx::PgPool;
use sqlx::Row;

use crate::claim::claim_next;
use crate::registry::JobOwner;
use crate::tests::support::{age_created_at, create_library, insert_job, status};
use crate::JobState;

#[sqlx::test(migrations = "../../infra/migrations")]
async fn api_claims_oldest_pending_job(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    let older = insert_job(&pool, Some(library), "metadata_batch", "pending").await;
    let newer = insert_job(&pool, Some(library), "metadata_batch", "pending").await;
    age_created_at(&pool, older, 60).await;

    let claimed = claim_next(&pool, JobOwner::Api).await.unwrap().unwrap();

    assert_eq!(claimed.id, older);
    assert_eq!(claimed.job_type, "metadata_batch");
    assert_eq!(claimed.library_id, Some(library));
    assert_eq!(status(&pool, older).await, "running");
    assert_eq!(status(&pool, newer).await, "pending");

    let row = sqlx::query("SELECT started_at, error_opt FROM index_jobs WHERE id = $1")
        .bind(older)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("started_at")
        .is_some());
    assert!(row.get::<Option<String>, _>("error_opt").is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn api_never_claims_indexer_jobs(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    insert_job(&pool, Some(library), "scan", "pending").await;

    assert!(claim_next(&pool, JobOwner::Api).await.unwrap().is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn indexer_never_claims_api_jobs(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    insert_job(&pool, Some(library), "metadata_batch", "pending").await;

    assert!(claim_next(&pool, JobOwner::Indexer)
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn inline_only_api_jobs_are_never_claimed(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    insert_job(&pool, Some(library), "reading_status_match", "pending").await;
    insert_job(&pool, None, "rating_pull", "pending").await;

    assert!(claim_next(&pool, JobOwner::Api).await.unwrap().is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn api_requires_library_for_library_scoped_types(pool: PgPool) {
    insert_job(&pool, None, "metadata_refresh", "pending").await;
    assert!(claim_next(&pool, JobOwner::Api).await.unwrap().is_none());

    let library = create_library(&pool, "lib").await;
    let job = insert_job(&pool, Some(library), "metadata_refresh", "pending").await;
    assert_eq!(
        claim_next(&pool, JobOwner::Api).await.unwrap().unwrap().id,
        job
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn api_claims_global_jobs_without_a_library(pool: PgPool) {
    let global = insert_job(&pool, None, "ddl_search", "pending").await;
    let optional = insert_job(&pool, None, "prowlarr_rss", "pending").await;
    age_created_at(&pool, global, 60).await;

    let first = claim_next(&pool, JobOwner::Api).await.unwrap().unwrap();
    assert_eq!(first.id, global);
    assert_eq!(first.library_id, None);

    let second = claim_next(&pool, JobOwner::Api).await.unwrap().unwrap();
    assert_eq!(second.id, optional);
    assert_eq!(second.library_id, None);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn api_metadata_refresh_is_mutually_exclusive_across_libraries(pool: PgPool) {
    let first = create_library(&pool, "first").await;
    let second = create_library(&pool, "second").await;
    let running = insert_job(&pool, Some(first), "metadata_refresh", "running").await;
    insert_job(&pool, Some(second), "metadata_refresh_all", "pending").await;

    assert!(claim_next(&pool, JobOwner::Api).await.unwrap().is_none());

    sqlx::query("UPDATE index_jobs SET status = 'success' WHERE id = $1")
        .bind(running)
        .execute(&pool)
        .await
        .unwrap();

    assert!(claim_next(&pool, JobOwner::Api).await.unwrap().is_some());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn api_non_exclusive_jobs_run_during_metadata_refresh(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    insert_job(&pool, Some(library), "metadata_refresh", "running").await;
    let ddl = insert_job(&pool, None, "ddl_search", "pending").await;

    let claimed = claim_next(&pool, JobOwner::Api).await.unwrap().unwrap();
    assert_eq!(claimed.id, ddl);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn indexer_prefers_full_rebuild_then_scan(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    let scan = insert_job(&pool, Some(library), "scan", "pending").await;
    let full = insert_job(&pool, Some(library), "full_rebuild", "pending").await;
    age_created_at(&pool, scan, 120).await;
    age_created_at(&pool, full, 60).await;

    let claimed = claim_next(&pool, JobOwner::Indexer).await.unwrap().unwrap();
    assert_eq!(claimed.id, full);

    let next = claim_next(&pool, JobOwner::Indexer).await.unwrap();
    assert!(next.is_none(), "exclusive jobs must not run concurrently");

    sqlx::query("UPDATE index_jobs SET status = 'success' WHERE id = $1")
        .bind(full)
        .execute(&pool)
        .await
        .unwrap();
    let next = claim_next(&pool, JobOwner::Indexer).await.unwrap().unwrap();
    assert_eq!(next.id, scan);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn indexer_exclusive_job_blocks_other_exclusive_jobs(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    insert_job(&pool, Some(library), "scan", "running").await;
    insert_job(&pool, Some(library), "rebuild", "pending").await;

    assert!(claim_next(&pool, JobOwner::Indexer)
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn indexer_still_claims_non_exclusive_conversion_during_scan(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    insert_job(&pool, Some(library), "scan", "running").await;
    let conversion = insert_job(&pool, None, "cbr_to_cbz", "pending").await;

    let claimed = claim_next(&pool, JobOwner::Indexer).await.unwrap().unwrap();
    assert_eq!(claimed.id, conversion);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn indexer_claim_covers_every_active_phase(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    for state in [
        JobState::Running,
        JobState::ExtractingPages,
        JobState::GeneratingThumbnails,
    ] {
        let active = insert_job(&pool, Some(library), "scan", state.as_str()).await;
        insert_job(&pool, Some(library), "rebuild", "pending").await;

        assert!(
            claim_next(&pool, JobOwner::Indexer)
                .await
                .unwrap()
                .is_none(),
            "{} must block exclusive jobs",
            state.as_str()
        );

        sqlx::query("DELETE FROM index_jobs WHERE id = $1")
            .bind(active)
            .execute(&pool)
            .await
            .unwrap();
    }
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn legacy_owner_never_claims(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    insert_job(&pool, Some(library), "torrent_import", "pending").await;

    assert!(claim_next(&pool, JobOwner::Legacy).await.unwrap().is_none());
}
