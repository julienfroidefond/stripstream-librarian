pub mod detection;
mod import_pipeline;
pub mod prowlarr;
pub mod qbittorrent;
pub mod rss_poll;
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
