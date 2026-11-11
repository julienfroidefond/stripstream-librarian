//! Registry of every background job type and the runtime rules attached to it.
//!
//! The registry is the single source of truth for:
//! - which service owns a job type (and may claim or clean it),
//! - whether the type can be claimed from the `pending` queue,
//! - whether it may run concurrently with other job types,
//! - whether the admin UI can replay it and whether replay needs a library,
//! - the category used for badges and notifications.

/// Service that processes a job type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOwner {
    Api,
    Indexer,
    /// Historical type kept in the database constraint without a live creation path.
    Legacy,
}

impl JobOwner {
    pub const fn as_str(self) -> &'static str {
        match self {
            JobOwner::Api => "api",
            JobOwner::Indexer => "indexer",
            JobOwner::Legacy => "legacy",
        }
    }
}

/// Functional family used by notifications and the admin UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobCategory {
    Scan,
    Thumbnail,
    Conversion,
    Metadata,
    Reading,
    Download,
    Search,
    Other,
}

impl JobCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            JobCategory::Scan => "scan",
            JobCategory::Thumbnail => "thumbnail",
            JobCategory::Conversion => "conversion",
            JobCategory::Metadata => "metadata",
            JobCategory::Reading => "reading",
            JobCategory::Download => "download",
            JobCategory::Search => "search",
            JobCategory::Other => "other",
        }
    }

    /// Category for a job type, defaulting to [`JobCategory::Other`] for unknown values.
    pub fn for_job_type(job_type: &str) -> Self {
        definition(job_type)
            .map(|definition| definition.category)
            .unwrap_or(JobCategory::Other)
    }
}

/// How the `library_id` column is used by a job type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobLibraryPolicy {
    /// A library id is mandatory.
    Required,
    /// The job may target one library or all libraries.
    Optional,
    /// The job is global and never carries a library id.
    Global,
}

impl JobLibraryPolicy {
    pub const fn requires_library(self) -> bool {
        matches!(self, JobLibraryPolicy::Required)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            JobLibraryPolicy::Required => "required",
            JobLibraryPolicy::Optional => "optional",
            JobLibraryPolicy::Global => "global",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct JobDefinition {
    pub job_type: &'static str,
    pub owner: JobOwner,
    pub category: JobCategory,
    pub library: JobLibraryPolicy,
    /// Whether the owner's queue poller may claim this type from `pending`.
    pub claimable: bool,
    /// Whether the admin UI can create a fresh job of this type.
    pub replayable: bool,
    /// Whether this type excludes other exclusive jobs from running at the same time.
    pub exclusive: bool,
    /// Lower values are claimed first by the indexer.
    pub priority: i32,
}

#[allow(clippy::too_many_arguments)]
const fn job(
    job_type: &'static str,
    owner: JobOwner,
    category: JobCategory,
    library: JobLibraryPolicy,
    claimable: bool,
    replayable: bool,
    exclusive: bool,
    priority: i32,
) -> JobDefinition {
    JobDefinition {
        job_type,
        owner,
        category,
        library,
        claimable,
        replayable,
        exclusive,
        priority,
    }
}

/// Every job type accepted by the `index_jobs_type_check` constraint.
///
/// Keep this in sync with `infra/migrations`; `registry::tests` verifies the
/// database constraint and this list agree.
pub const JOB_DEFINITIONS: &[JobDefinition] = &[
    // API-owned jobs
    job(
        "metadata_batch",
        JobOwner::Api,
        JobCategory::Metadata,
        JobLibraryPolicy::Required,
        true,
        true,
        false,
        0,
    ),
    job(
        "metadata_batch_rematch",
        JobOwner::Api,
        JobCategory::Metadata,
        JobLibraryPolicy::Required,
        true,
        true,
        false,
        0,
    ),
    job(
        "metadata_refresh",
        JobOwner::Api,
        JobCategory::Metadata,
        JobLibraryPolicy::Required,
        true,
        true,
        true,
        0,
    ),
    job(
        "metadata_refresh_all",
        JobOwner::Api,
        JobCategory::Metadata,
        JobLibraryPolicy::Required,
        true,
        true,
        true,
        0,
    ),
    // Reading-status matching runs inline (created `running`), so it is never claimed.
    job(
        "reading_status_match",
        JobOwner::Api,
        JobCategory::Reading,
        JobLibraryPolicy::Required,
        false,
        true,
        false,
        0,
    ),
    // Dual mode: created `running` for an inline run, and `pending` by the scheduler.
    job(
        "reading_status_push",
        JobOwner::Api,
        JobCategory::Reading,
        JobLibraryPolicy::Required,
        true,
        true,
        false,
        0,
    ),
    job(
        "rating_pull",
        JobOwner::Api,
        JobCategory::Reading,
        JobLibraryPolicy::Global,
        false,
        true,
        false,
        0,
    ),
    job(
        "download_detection",
        JobOwner::Api,
        JobCategory::Download,
        JobLibraryPolicy::Required,
        true,
        true,
        false,
        0,
    ),
    job(
        "prowlarr_rss",
        JobOwner::Api,
        JobCategory::Download,
        JobLibraryPolicy::Optional,
        true,
        true,
        false,
        0,
    ),
    job(
        "telegram_sync",
        JobOwner::Api,
        JobCategory::Download,
        JobLibraryPolicy::Global,
        true,
        false,
        false,
        0,
    ),
    job(
        "telegram_sync_incremental",
        JobOwner::Api,
        JobCategory::Download,
        JobLibraryPolicy::Global,
        true,
        false,
        false,
        0,
    ),
    job(
        "ddl_search",
        JobOwner::Api,
        JobCategory::Search,
        JobLibraryPolicy::Global,
        true,
        true,
        false,
        0,
    ),
    // Indexer-owned jobs
    job(
        "scan",
        JobOwner::Indexer,
        JobCategory::Scan,
        JobLibraryPolicy::Optional,
        true,
        true,
        true,
        2,
    ),
    job(
        "rebuild",
        JobOwner::Indexer,
        JobCategory::Scan,
        JobLibraryPolicy::Optional,
        true,
        true,
        true,
        2,
    ),
    job(
        "full_rebuild",
        JobOwner::Indexer,
        JobCategory::Scan,
        JobLibraryPolicy::Optional,
        true,
        true,
        true,
        1,
    ),
    job(
        "rescan",
        JobOwner::Indexer,
        JobCategory::Scan,
        JobLibraryPolicy::Optional,
        true,
        true,
        true,
        3,
    ),
    job(
        "thumbnail_rebuild",
        JobOwner::Indexer,
        JobCategory::Thumbnail,
        JobLibraryPolicy::Optional,
        true,
        true,
        true,
        3,
    ),
    job(
        "thumbnail_regenerate",
        JobOwner::Indexer,
        JobCategory::Thumbnail,
        JobLibraryPolicy::Optional,
        true,
        true,
        true,
        3,
    ),
    job(
        "cbr_to_cbz",
        JobOwner::Indexer,
        JobCategory::Conversion,
        JobLibraryPolicy::Optional,
        true,
        false,
        false,
        3,
    ),
    // Historical type: still accepted by the database constraint, no creation path.
    job(
        "torrent_import",
        JobOwner::Legacy,
        JobCategory::Download,
        JobLibraryPolicy::Optional,
        false,
        false,
        false,
        0,
    ),
];

pub fn definitions() -> &'static [JobDefinition] {
    JOB_DEFINITIONS
}

pub fn definition(job_type: &str) -> Option<&'static JobDefinition> {
    JOB_DEFINITIONS
        .iter()
        .find(|definition| definition.job_type == job_type)
}

/// Every type owned by `owner`, used for owner-scoped cleanup.
pub fn types_for_owner(owner: JobOwner) -> Vec<String> {
    JOB_DEFINITIONS
        .iter()
        .filter(|definition| definition.owner == owner)
        .map(|definition| definition.job_type.to_string())
        .collect()
}

/// Types `owner` may claim from the `pending` queue.
pub fn claimable_types(owner: JobOwner) -> Vec<String> {
    JOB_DEFINITIONS
        .iter()
        .filter(|definition| definition.owner == owner && definition.claimable)
        .map(|definition| definition.job_type.to_string())
        .collect()
}

/// Types that may run with a NULL `library_id` (optional or global).
pub fn optional_library_types(owner: JobOwner) -> Vec<String> {
    JOB_DEFINITIONS
        .iter()
        .filter(|definition| {
            definition.owner == owner
                && definition.claimable
                && definition.library != JobLibraryPolicy::Required
        })
        .map(|definition| definition.job_type.to_string())
        .collect()
}

/// Exclusive types owned by `owner`.
pub fn exclusive_types(owner: JobOwner) -> Vec<String> {
    JOB_DEFINITIONS
        .iter()
        .filter(|definition| definition.owner == owner && definition.exclusive)
        .map(|definition| definition.job_type.to_string())
        .collect()
}
