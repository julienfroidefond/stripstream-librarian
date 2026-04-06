pub mod batch;
mod batch_sync;
pub mod config;
pub mod handlers;
pub mod refresh;
pub(crate) mod refresh_sync;
mod sync;

// Re-export handlers (used by main.rs routes and openapi.rs paths)
pub use handlers::{
    approve_metadata, create_metadata_match, delete_metadata_link, get_metadata_links,
    get_missing_books, reject_metadata, search_metadata,
};

// Re-export handler types (used by openapi.rs schemas)
pub use handlers::{
    ApproveRequest, ApproveResponse, BookSyncReport, ExternalMetadataLinkDto, FieldChange,
    MetadataMatchRequest, MetadataSearchRequest, MissingBookItem, MissingBooksDto,
    SeriesCandidateDto, SeriesSyncReport, SyncReport,
};

// Re-export batch (used by main.rs routes, openapi.rs paths/schemas, job_poller.rs)
pub use batch::{
    get_batch_report, get_batch_results, start_batch,
    MetadataBatchReportDto, MetadataBatchRequest, MetadataBatchResultDto,
};
pub(crate) use batch::process_metadata_batch;

// Re-export refresh (used by main.rs routes, openapi.rs paths/schemas, job_poller.rs, torrent_import.rs)
pub use refresh::{
    get_refresh_report, refresh_single_link, start_refresh, start_refresh_all,
    MetadataRefreshReportDto, MetadataRefreshRequest,
};
pub(crate) use refresh::{
    process_metadata_refresh, process_metadata_refresh_all, refresh_link,
};

// Re-export sync functions (used by series::create)
pub(crate) use sync::{sync_series_metadata, sync_books_metadata};

