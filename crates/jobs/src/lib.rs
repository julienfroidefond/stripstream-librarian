//! Shared lifecycle for background jobs stored in `index_jobs`.
//!
//! Both the API and the indexer own background job types but must agree on the
//! same rules: which types can be claimed by whom, how a job transitions from
//! `pending` to a terminal status, and what cleanup may touch. This crate is the
//! single place where those rules live.
//!
//! Job *creation* (the `INSERT INTO index_jobs ...` statements) intentionally
//! stays in the owning service: the registry describes runtime behaviour, not
//! scheduling policy.

pub mod claim;
pub mod cleanup;
pub mod helpers;
pub mod lifecycle;
pub mod registry;
pub mod status;

pub use claim::{claim_next, ClaimedJob};
pub use cleanup::{cleanup_stale_jobs, fail_orphaned_jobs, purge_terminal_jobs, CleanupReport};
pub use helpers::{
    insert_entity_event, insert_event, is_job_cancelled, set_current_file, set_progress,
    set_total_files, update_progress, update_progress_checked,
};
pub use lifecycle::{
    begin_analysis, begin_extraction, begin_thumbnail_generation, begin_thumbnail_job, cancel_job,
    classify_job, complete_job, fail_job, finish_job, start_job, JobScope, JobStatus,
};
pub use registry::{
    claimable_types, definition, definitions, exclusive_types, optional_library_types,
    types_for_owner, JobCategory, JobDefinition, JobLibraryPolicy, JobOwner,
};
pub use status::JobState;

#[cfg(test)]
mod tests;
