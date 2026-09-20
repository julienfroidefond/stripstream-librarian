use super::*;
use sqlx::PgPool;

#[sqlx::test(migrations = "../../infra/migrations")]
async fn fail_orphaned_api_jobs_only_touches_api_types(pool: PgPool) {
    sqlx::query(
        "INSERT INTO index_jobs (id, type, status) VALUES
         ('00000000-0000-0000-0000-0000000000a1', 'metadata_batch', 'running'),
         ('00000000-0000-0000-0000-0000000000a2', 'scan', 'running'),
         ('00000000-0000-0000-0000-0000000000a3', 'metadata_batch', 'success')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let affected = fail_orphaned_api_jobs(&pool).await.unwrap();
    assert_eq!(affected, 1);

    let api_status: String = sqlx::query_scalar("SELECT status FROM index_jobs WHERE id = $1")
        .bind(uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000a1").unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(api_status, "failed");

    let indexer_status: String = sqlx::query_scalar("SELECT status FROM index_jobs WHERE id = $1")
        .bind(uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000a2").unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(indexer_status, "running");
}

#[test]
fn api_job_types_contains_metadata_refresh_all() {
    assert!(
        API_JOB_TYPES.contains(&"metadata_refresh_all"),
        "API_JOB_TYPES must include metadata_refresh_all"
    );
}

#[test]
fn api_job_types_contains_all_expected_types() {
    let expected = &[
        "metadata_batch",
        "metadata_refresh",
        "metadata_refresh_all",
        "reading_status_push",
        "download_detection",
    ];
    for t in expected {
        assert!(API_JOB_TYPES.contains(t), "API_JOB_TYPES is missing: {t}");
    }
}

#[test]
fn global_metadata_refresh_job_types_contains_both_variants() {
    assert!(
        GLOBAL_METADATA_REFRESH_JOB_TYPES.contains(&"metadata_refresh"),
        "GLOBAL_METADATA_REFRESH_JOB_TYPES must include metadata_refresh"
    );
    assert!(
        GLOBAL_METADATA_REFRESH_JOB_TYPES.contains(&"metadata_refresh_all"),
        "GLOBAL_METADATA_REFRESH_JOB_TYPES must include metadata_refresh_all"
    );
}
