mod auth;
mod books;
mod channels;
mod jobs;
mod types;

pub use auth::{disconnect, get_status, save_settings, start_auth, verify_auth};
pub use books::{
    dismiss_book, download_book, list_available_by_series, list_books, list_downloads, live_search,
    search_available_books,
};
pub use channels::{add_source, delete_source, list_sources, search_channels, sync_sources};
pub use jobs::{
    process_telegram_sync, process_telegram_sync_incremental, start_incremental_sync_job,
    start_sync_job,
};
pub use types::*;

#[cfg(test)]
mod tests {
    use parsers::{extract_series_name_from_filename, extract_volume};

    fn e(filename: &str) -> String {
        extract_series_name_from_filename(filename)
    }

    fn v(filename: &str) -> Option<i32> {
        extract_volume(filename)
    }

    // --- existing patterns ---
    #[test]
    fn tome_dash() {
        assert_eq!(e("One Piece - Tome 47.cbz"), "One Piece");
    }
    #[test]
    fn tome_prefix() {
        assert_eq!(e("Toriko T12.cbz"), "Toriko");
    }
    #[test]
    fn vol_dash() {
        assert_eq!(e("Naruto - Vol. 3.cbz"), "Naruto");
    }
    #[test]
    fn bare_number() {
        assert_eq!(e("Berserk 08.cbz"), "Berserk");
    }

    // --- chapter markers (the new cases) ---
    #[test]
    fn ch_dash_number() {
        assert_eq!(
            e("Boruto - Two Blue Vortex - Ch11.cbz"),
            "Boruto - Two Blue Vortex"
        );
    }
    #[test]
    fn ch_dash_lowercase() {
        assert_eq!(
            e("Boruto - Two Blue Vortex - ch12.cbz"),
            "Boruto - Two Blue Vortex"
        );
    }
    #[test]
    fn ch_no_dash() {
        assert_eq!(
            e("Boruto - Two Blue Vortex Ch09.cbz"),
            "Boruto - Two Blue Vortex"
        );
    }
    #[test]
    fn hash_ch_dash() {
        assert_eq!(
            e("Boruto - two blue vortex - #Ch03.cbz"),
            "Boruto - two blue vortex"
        );
    }
    #[test]
    fn hash_ch_no_dash() {
        assert_eq!(e("Dandadan #Ch05.cbz"), "Dandadan");
    }
    #[test]
    fn hash_digit() {
        assert_eq!(e("Dandadan #15.cbz"), "Dandadan");
    }
    #[test]
    fn ch_dot() {
        assert_eq!(e("Gachiakuta - Ch. 38.cbz"), "Gachiakuta");
    }

    // --- series names with dashes must NOT be stripped ---
    #[test]
    fn series_name_with_dash() {
        assert_eq!(
            e("Boruto - Two Blue Vortex - Tome 01.cbz"),
            "Boruto - Two Blue Vortex"
        );
    }
    #[test]
    fn dragon_ball_z() {
        assert_eq!(e("Dragon Ball Z - Tome 01.cbz"), "Dragon Ball Z");
    }

    // --- Telegram @channel attribution ---
    #[test]
    fn tg_series_channel_tag() {
        assert_eq!(e("Berserk - 32@BD_fr.cbz"), "Berserk");
    }
    #[test]
    fn tg_volume_channel_tag() {
        assert_eq!(v("Berserk - 32@BD_fr.cbz"), Some(32));
    }
    #[test]
    fn tg_volume_channel_tag_spaced() {
        assert_eq!(v("One Piece - 47 @BD_fr.cbz"), Some(47));
    }
}
