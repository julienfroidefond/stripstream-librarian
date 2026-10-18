pub mod alldebrid;
mod blacklist;
pub mod ddl_provider;
pub mod detection;
mod http_error;
pub mod http_range;
mod import_pipeline;
mod missing;

pub mod prowlarr;
pub mod qbittorrent;
pub mod ranged;
pub mod rss_poll;
pub mod settings;
mod store;
pub mod target;
pub mod telegram_monitor;
pub mod torrent_import;
pub mod worker;

// Re-export handler functions used in route registration
pub use alldebrid::{
    add_torrent as add_alldebrid_torrent, restart_torrent_download as restart_alldebrid_download,
    run_alldebrid_poller, test_alldebrid,
};
pub use ddl_provider::{
    dismiss_ddl_download, get_ddl_search_settings, list_ddl_downloads, list_ddl_providers,
    search_ddl_provider, start_ddl_download, start_ddl_search_all, test_ddl_provider,
    toggle_ddl_provider, update_ddl_search_settings,
};
pub use detection::{
    blacklist_release, delete_available_download, get_detection_report, get_detection_results,
    get_latest_found, list_blacklisted_releases, start_detection, unblacklist_release,
};
pub use prowlarr::{search_prowlarr, test_prowlarr};
pub use qbittorrent::{add_torrent, test_qbittorrent};
pub use rss_poll::start_rss_poll;
pub use telegram_monitor::{
    add_source as tg_monitor_add_source, delete_source as tg_monitor_delete_source,
    disconnect as tg_monitor_disconnect, dismiss_book as tg_monitor_dismiss_book,
    download_book as tg_monitor_download_book, get_status as tg_monitor_status,
    list_available_by_series as tg_monitor_list_available, list_books as tg_monitor_list_books,
    list_downloads as tg_monitor_list_downloads, list_sources as tg_monitor_list_sources,
    live_search as tg_monitor_live_search, save_settings as tg_monitor_save_settings,
    search_available_books as tg_monitor_search, search_channels as tg_monitor_search_channels,
    start_auth as tg_monitor_start_auth,
    start_incremental_sync_job as tg_monitor_start_incremental_sync_job,
    start_sync_job as tg_monitor_start_sync_job, sync_sources as tg_monitor_sync,
    verify_auth as tg_monitor_verify_auth,
};
pub use torrent_import::{
    delete_torrent_download, list_torrent_downloads, notify_torrent_done, retry_torrent_import,
    run_torrent_poller,
};
