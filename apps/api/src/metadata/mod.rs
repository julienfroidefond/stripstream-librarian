pub mod batch;
mod batch_sync;
pub mod config;
pub mod gaps;
pub mod handlers;
pub mod refresh;
pub(crate) mod refresh_sync;
pub(crate) mod shared_sync;
mod sync;

// Re-export handlers (used by main.rs routes and openapi.rs paths)
pub use handlers::{
    approve_metadata, create_metadata_match, delete_metadata_link, get_metadata_links,
    get_missing_books, list_metadata_providers, patch_metadata_link, reject_metadata,
    search_metadata,
};

// Re-export handler types (used by openapi.rs schemas)
pub use handlers::{
    ApproveRequest, ApproveResponse, BookSyncReport, ExternalMetadataLinkDto, FieldChange,
    MetadataMatchRequest, MetadataSearchRequest, MissingBookItem, MissingBooksDto,
    PatchLinkRequest, PatchLinkResponse, SeriesCandidateDto, SeriesSyncReport, SyncReport,
};

// Re-export metadata-gap summary (used by main.rs routes and openapi.rs paths/schemas)
pub use gaps::{get_gap_summary, GapSummary};

// Re-export batch (used by main.rs routes, openapi.rs paths/schemas, job_poller.rs)
pub(crate) use batch::process_metadata_batch;
pub use batch::{
    get_batch_report, get_batch_results, start_batch, MetadataBatchReportDto, MetadataBatchRequest,
    MetadataBatchResultDto,
};

// Re-export refresh (used by main.rs routes, openapi.rs paths/schemas, job_poller.rs, torrent_import.rs)
pub use refresh::{
    get_refresh_report, refresh_single_link, start_refresh, start_refresh_all,
    MetadataRefreshReportDto, MetadataRefreshRequest,
};
pub(crate) use refresh::{process_metadata_refresh, process_metadata_refresh_all, refresh_link};

// Re-export sync functions (used by series::create and metadata handlers)
pub(crate) use sync::sync_series_from_links;
