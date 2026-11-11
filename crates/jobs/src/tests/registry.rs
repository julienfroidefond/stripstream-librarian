use std::collections::HashSet;

use sqlx::PgPool;

use crate::registry::{definitions, JobCategory, JobLibraryPolicy, JobOwner, JOB_DEFINITIONS};
use crate::tests::support::insert_job;

#[test]
fn definitions_are_unique_and_complete() {
    let mut seen = HashSet::new();
    for definition in definitions() {
        assert!(
            seen.insert(definition.job_type),
            "duplicate job type in registry: {}",
            definition.job_type
        );
        assert!(!definition.job_type.is_empty());
        assert!(definition.priority >= 0);
    }
    assert_eq!(definitions().len(), 20);
}

#[test]
fn runtime_rules_preserve_service_boundaries() {
    let api = crate::registry::claimable_types(JobOwner::Api);
    assert!(api.contains(&"metadata_batch".to_string()));
    assert!(api.contains(&"reading_status_push".to_string()));
    assert!(!api.contains(&"reading_status_match".to_string()));
    assert!(!api.contains(&"rating_pull".to_string()));

    let indexer = crate::registry::claimable_types(JobOwner::Indexer);
    assert!(indexer.contains(&"scan".to_string()));
    assert!(indexer.contains(&"cbr_to_cbz".to_string()));
    assert!(!indexer.contains(&"metadata_batch".to_string()));

    let exclusive = crate::registry::exclusive_types(JobOwner::Api);
    assert!(exclusive.contains(&"metadata_refresh".to_string()));
    assert!(exclusive.contains(&"metadata_refresh_all".to_string()));
}

#[test]
fn legacy_torrent_import_has_no_live_path() {
    let definition = crate::registry::definition("torrent_import").unwrap();
    assert_eq!(definition.owner, JobOwner::Legacy);
    assert!(!definition.claimable);
    assert!(!definition.replayable);
}

#[test]
fn categories_match_the_ui_domains() {
    assert_eq!(JobCategory::for_job_type("scan"), JobCategory::Scan);
    assert_eq!(
        JobCategory::for_job_type("thumbnail_rebuild"),
        JobCategory::Thumbnail
    );
    assert_eq!(
        JobCategory::for_job_type("cbr_to_cbz"),
        JobCategory::Conversion
    );
    assert_eq!(
        JobCategory::for_job_type("metadata_refresh"),
        JobCategory::Metadata
    );
    assert_eq!(
        JobCategory::for_job_type("reading_status_push"),
        JobCategory::Reading
    );
    assert_eq!(
        JobCategory::for_job_type("prowlarr_rss"),
        JobCategory::Download
    );
    assert_eq!(JobCategory::for_job_type("ddl_search"), JobCategory::Search);
    assert_eq!(JobCategory::for_job_type("unknown"), JobCategory::Other);
}

#[test]
fn only_replayable_types_need_a_library() {
    for definition in definitions() {
        if definition.library == JobLibraryPolicy::Global {
            assert!(!definition.library.requires_library());
        }
        if definition.replayable && definition.job_type == "metadata_refresh" {
            assert!(definition.library.requires_library());
        }
    }
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn database_accepts_every_registry_type(pool: PgPool) {
    for definition in JOB_DEFINITIONS {
        insert_job(&pool, None, definition.job_type, "success").await;
    }
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn database_constraint_matches_registry(pool: PgPool) {
    let definition: String = sqlx::query_scalar(
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint \
         WHERE conname = 'index_jobs_type_check'",
    )
    .fetch_one(&pool)
    .await
    .expect("index_jobs_type_check must exist");

    let mut db_types = Vec::new();
    let mut rest = definition.as_str();
    while let Some(start) = rest.find('\'') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('\'') else {
            break;
        };
        db_types.push(after[..end].to_string());
        rest = &after[end + 1..];
    }

    let registry: HashSet<&str> = definitions()
        .iter()
        .map(|definition| definition.job_type)
        .collect();
    let database: HashSet<&str> = db_types.iter().map(String::as_str).collect();

    let missing: Vec<_> = registry.difference(&database).collect();
    let extra: Vec<_> = database.difference(&registry).collect();

    assert!(
        missing.is_empty(),
        "registry types missing from index_jobs_type_check: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "database types missing from the registry: {extra:?}"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn database_rejects_unregistered_types(pool: PgPool) {
    let result = sqlx::query(
        "INSERT INTO index_jobs (id, type, status) VALUES ($1, 'not_a_job', 'pending')",
    )
    .bind(uuid::Uuid::new_v4())
    .execute(&pool)
    .await;

    assert!(
        result.is_err(),
        "the database must reject unknown job types"
    );
}
