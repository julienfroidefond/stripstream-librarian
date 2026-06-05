pub mod detection;
mod import_pipeline;
pub mod prowlarr;
pub mod qbittorrent;
pub mod rss_poll;
pub mod telegram_monitor;
pub mod torrent_import;

// Re-export handler functions used in route registration
pub use detection::{
    blacklist_release, delete_available_download, get_detection_report, get_detection_results,
    get_latest_found, list_blacklisted_releases, start_detection, unblacklist_release,
};
pub use prowlarr::{search_prowlarr, test_prowlarr};
pub use qbittorrent::{add_torrent, test_qbittorrent};
pub use rss_poll::start_rss_poll;
pub use torrent_import::{
    delete_torrent_download, list_torrent_downloads, notify_torrent_done, retry_torrent_import,
    run_torrent_poller,
};
pub use telegram_monitor::{
    get_status as tg_monitor_status,
    save_settings as tg_monitor_save_settings,
    start_auth as tg_monitor_start_auth,
    verify_auth as tg_monitor_verify_auth,
    disconnect as tg_monitor_disconnect,
    list_sources as tg_monitor_list_sources,
    add_source as tg_monitor_add_source,
    delete_source as tg_monitor_delete_source,
    sync_sources as tg_monitor_sync,
    list_books as tg_monitor_list_books,
    download_book as tg_monitor_download_book,
    dismiss_book as tg_monitor_dismiss_book,
    search_channels as tg_monitor_search_channels,
    list_available_by_series as tg_monitor_list_available,
    list_downloads as tg_monitor_list_downloads,
    start_sync_job as tg_monitor_start_sync_job,
    search_available_books as tg_monitor_search,
    live_search as tg_monitor_live_search,
};
