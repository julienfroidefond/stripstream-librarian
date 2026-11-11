//! Admin catalog of every background job type.
//!
//! The backoffice uses it to decide which jobs can be replayed, whether a
//! library is required, and how to label badges. It is generated from the
//! shared `stripstream_jobs` registry, so the UI cannot drift from the runtime.

use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

use stripstream_jobs::{definitions, JobDefinition};

/// One row of the job-type catalog.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct JobTypeDto {
    /// Database value stored in `index_jobs.type`.
    pub job_type: String,
    /// Service that processes the type (`api`, `indexer` or `legacy`).
    pub owner: String,
    /// Functional family used for badges and notifications.
    pub category: String,
    /// Whether the owner's queue poller may claim it from `pending`.
    pub claimable: bool,
    /// Whether the admin UI can create a fresh job of this type.
    pub replayable: bool,
    /// Whether replaying or scheduling it needs a `library_id`.
    pub requires_library: bool,
}

impl From<&JobDefinition> for JobTypeDto {
    fn from(definition: &JobDefinition) -> Self {
        Self {
            job_type: definition.job_type.to_string(),
            owner: definition.owner.as_str().to_string(),
            category: definition.category.as_str().to_string(),
            claimable: definition.claimable,
            replayable: definition.replayable,
            requires_library: definition.library.requires_library(),
        }
    }
}

/// List every known job type with its runtime capabilities.
#[utoipa::path(
    get,
    path = "/index/job-types",
    tag = "indexing",
    responses(
        (status = 200, body = Vec<JobTypeDto>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_job_types() -> Json<Vec<JobTypeDto>> {
    Json(definitions().iter().map(JobTypeDto::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_exposes_every_registry_definition_in_order() {
        let catalog: Vec<JobTypeDto> = definitions().iter().map(JobTypeDto::from).collect();
        assert_eq!(catalog.len(), definitions().len());
        assert_eq!(
            catalog.first().map(|dto| dto.job_type.as_str()),
            definitions().first().map(|definition| definition.job_type)
        );
        assert_eq!(
            catalog.last().map(|dto| dto.job_type.as_str()),
            definitions().last().map(|definition| definition.job_type)
        );
    }

    #[test]
    fn catalog_derives_library_requirement_from_the_registry() {
        let replayable = catalog_entry("rebuild");
        assert!(replayable.replayable);
        assert!(!replayable.requires_library);

        let library_job = catalog_entry("metadata_batch");
        assert!(library_job.requires_library);

        let global_job = catalog_entry("ddl_search");
        assert!(!global_job.requires_library);
    }

    #[test]
    fn legacy_types_are_visible_but_not_replayable() {
        let legacy = catalog_entry("torrent_import");
        assert_eq!(legacy.owner, "legacy");
        assert!(!legacy.claimable);
        assert!(!legacy.replayable);
    }

    fn catalog_entry(job_type: &str) -> JobTypeDto {
        let definition = stripstream_jobs::definition(job_type)
            .unwrap_or_else(|| panic!("missing registry entry for {job_type}"));
        JobTypeDto::from(definition)
    }
}
