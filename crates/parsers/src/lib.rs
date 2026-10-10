use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use anyhow::Result;

pub mod matching;
pub use matching::{
    extract_series_name_from_filename, extract_series_name_from_release, fold_accents,
    is_integral_release, match_release_title, match_title_volumes, normalize_title,
    title_matches_series, MatchConfidence, MatchReason, ReleaseTitleMatch,
};

mod cbr;
mod epub;
mod metadata;
mod pdf;
mod volume;
mod zip;

pub use cbr::convert_cbr_to_cbz;
pub use metadata::{analyze_book, parse_metadata, parse_metadata_fast};
pub use volume::{
    extract_hs_info, extract_int_info, extract_metadata_volume, extract_volume, extract_volumes,
    read_bare_number, read_vol_prefix_number,
};

/// Cache of sorted image names per archive path. Avoids re-listing and sorting on every page request.
/// Keyed by (path, mtime) so the cache invalidates automatically when the file is replaced.
pub(crate) type ArchiveIndexCache = OnceLock<Mutex<HashMap<PathBuf, (SystemTime, Vec<String>)>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookFormat {
    Cbz,
    Cbr,
    Pdf,
    Epub,
}

impl BookFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Pdf => "pdf",
            Self::Epub => "epub",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeType {
    Regular,
    Hs,
    Oneshot,
    Integral,
}

impl VolumeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Hs => "hs",
            Self::Oneshot => "oneshot",
            Self::Integral => "integral",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedMetadata {
    pub title: String,
    pub series: Option<String>,
    pub volume: Option<i32>,
    pub volume_type: VolumeType,
    pub page_count: Option<i32>,
}

pub fn detect_format(path: &Path) -> Option<BookFormat> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "cbz" => Some(BookFormat::Cbz),
        "cbr" => Some(BookFormat::Cbr),
        "pdf" => Some(BookFormat::Pdf),
        "epub" => Some(BookFormat::Epub),
        _ => None,
    }
}

pub fn is_image_name(name: &str) -> bool {
    // Skip macOS metadata entries (__MACOSX/ prefix or AppleDouble ._* files)
    if name.starts_with("__macosx/") || name.contains("/._") || name.starts_with("._") {
        return false;
    }
    name.ends_with(".jpg")
        || name.ends_with(".jpeg")
        || name.ends_with(".png")
        || name.ends_with(".webp")
        || name.ends_with(".avif")
        || name.ends_with(".gif")
        || name.ends_with(".bmp")
        || name.ends_with(".tif")
        || name.ends_with(".tiff")
}

/// Returns the sorted list of image entry names in a CBZ or CBR archive.
/// Intended to be cached by the caller; pass the result to `extract_image_by_name`.
pub fn list_archive_images(path: &Path, format: BookFormat) -> Result<Vec<String>> {
    match format {
        BookFormat::Cbz => zip::list_cbz_images(path),
        BookFormat::Cbr => cbr::list_cbr_images(path),
        BookFormat::Pdf => Err(anyhow::anyhow!(
            "list_archive_images not applicable for PDF"
        )),
        BookFormat::Epub => epub::get_epub_image_index(path),
    }
}

/// Extract a specific image entry by name from a CBZ or CBR archive.
/// Use in combination with `list_archive_images` to avoid re-enumerating entries.
pub fn extract_image_by_name(path: &Path, format: BookFormat, image_name: &str) -> Result<Vec<u8>> {
    match format {
        BookFormat::Cbz => zip::extract_cbz_by_name(path, image_name),
        BookFormat::Cbr => cbr::extract_cbr_by_name(path, image_name),
        BookFormat::Pdf => Err(anyhow::anyhow!("use extract_page for PDF")),
        BookFormat::Epub => zip::extract_cbz_by_name(path, image_name),
    }
}

pub fn extract_first_page(path: &Path, format: BookFormat) -> Result<Vec<u8>> {
    extract_page(path, format, 1, 0)
}

/// Extract a specific page (1-based index) from a book archive.
/// `pdf_render_width`: max width for PDF rasterization; 0 means use default (1200).
pub fn extract_page(
    path: &Path,
    format: BookFormat,
    page_number: u32,
    pdf_render_width: u32,
) -> Result<Vec<u8>> {
    if page_number == 0 {
        return Err(anyhow::anyhow!("page index starts at 1"));
    }
    match format {
        BookFormat::Cbz => zip::extract_cbz_page(path, page_number, true),
        BookFormat::Cbr => cbr::extract_cbr_page(path, page_number, true),
        BookFormat::Pdf => {
            let width = if pdf_render_width == 0 {
                1200
            } else {
                pdf_render_width
            };
            pdf::render_pdf_page_n(path, page_number, width)
        }
        BookFormat::Epub => epub::extract_epub_page(path, page_number),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cbr::{is_not_rar_error, open_cbr_listing};
    use crate::volume::{clean_title, extract_series, is_hs_subfolder, parse_volume_markers};
    use crate::zip::get_cbz_image_index;

    #[test]
    fn extract_metadata_volume_requires_an_explicit_marker() {
        assert_eq!(extract_metadata_volume("One Piece - Tome 12"), Some(12));
        assert_eq!(extract_metadata_volume("Naruto Tome 12"), Some(12));
        assert_eq!(extract_metadata_volume("Astro Boy T.03"), Some(3));
        assert_eq!(extract_metadata_volume("T.007"), Some(7));
        assert_eq!(extract_metadata_volume("Saga Vol. 004"), Some(4));
        assert_eq!(extract_metadata_volume("Vol 5"), Some(5));
        assert_eq!(extract_metadata_volume("Series Volume 5"), Some(5));
        assert_eq!(extract_metadata_volume("Issue #42"), Some(42));
        assert_eq!(extract_metadata_volume("#5 something"), Some(5));

        assert_eq!(extract_metadata_volume("One Piece (12)"), None);
        assert_eq!(extract_metadata_volume("One Piece 12"), None);
        assert_eq!(extract_metadata_volume("Edition 2024"), None);
        assert_eq!(extract_metadata_volume("One Piece - Intégrale"), None);
    }

    #[test]
    fn volume_projections_share_a_deduplicated_parse() {
        let parsed = parse_volume_markers("Series T01-T03 T02");

        assert_eq!(parsed.volumes, vec![1, 2, 3]);
        assert_eq!(extract_volume(parsed.title), Some(1));
        assert_eq!(extract_volumes(parsed.title), parsed.volumes);
    }

    #[test]
    fn cleaning_a_title_does_not_create_a_volume_marker() {
        let title = "Series Vol. 02";
        let parsed = parse_volume_markers(title);
        let cleaned = parsed.cleaned_title();

        assert!(extract_volumes(&cleaned)
            .into_iter()
            .all(|volume| parsed.volumes.contains(&volume)));
    }

    #[test]
    fn is_not_rar_detects_not_rar_archive() {
        assert!(is_not_rar_error(
            "unrar listing failed for /path/file.cbr: Not a RAR archive"
        ));
    }

    #[test]
    fn is_not_rar_detects_bad_archive() {
        assert!(is_not_rar_error(
            "unrar listing failed for /path/file.cbr: bad archive"
        ));
    }

    #[test]
    fn is_not_rar_ignores_other_errors() {
        assert!(!is_not_rar_error(
            "unrar listing failed for /path/file.cbr: file not found"
        ));
        assert!(!is_not_rar_error("some other error"));
        assert!(!is_not_rar_error(""));
    }

    #[test]
    fn open_cbr_listing_nonexistent_file() {
        let result = open_cbr_listing(Path::new("/nonexistent/file.cbr"));
        assert!(result.is_err());
    }

    #[test]
    fn detect_format_cbz() {
        assert_eq!(detect_format(Path::new("test.cbz")), Some(BookFormat::Cbz));
        assert_eq!(detect_format(Path::new("test.CBZ")), Some(BookFormat::Cbz));
    }

    #[test]
    fn detect_format_cbr() {
        assert_eq!(detect_format(Path::new("test.cbr")), Some(BookFormat::Cbr));
    }

    #[test]
    fn detect_format_pdf() {
        assert_eq!(detect_format(Path::new("test.pdf")), Some(BookFormat::Pdf));
    }

    #[test]
    fn detect_format_epub() {
        assert_eq!(
            detect_format(Path::new("test.epub")),
            Some(BookFormat::Epub)
        );
    }

    #[test]
    fn detect_format_unknown() {
        assert_eq!(detect_format(Path::new("test.txt")), None);
        assert_eq!(detect_format(Path::new("no_extension")), None);
    }

    #[test]
    fn is_image_name_accepts_valid() {
        assert!(is_image_name("page01.jpg"));
        assert!(is_image_name("cover.png"));
        assert!(is_image_name("image.webp"));
    }

    #[test]
    fn is_image_name_rejects_non_images() {
        assert!(!is_image_name("readme.txt"));
        assert!(!is_image_name("metadata.xml"));
        assert!(!is_image_name(""));
    }

    #[test]
    fn is_image_name_rejects_macos_metadata() {
        assert!(!is_image_name("__macosx/page01.jpg"));
        assert!(!is_image_name("folder/._cover.png"));
        assert!(!is_image_name("._hidden.jpg"));
    }

    #[test]
    fn is_image_name_accepts_all_formats() {
        assert!(is_image_name("page.jpeg"));
        assert!(is_image_name("page.avif"));
        assert!(is_image_name("page.gif"));
        assert!(is_image_name("page.bmp"));
        assert!(is_image_name("page.tif"));
        assert!(is_image_name("page.tiff"));
    }

    // ─── extract_volume ──────────────────────────────────────────────────

    #[test]
    fn extract_volume_t_prefix() {
        assert_eq!(extract_volume("One Piece T01"), Some(1));
        assert_eq!(extract_volume("Naruto T12"), Some(12));
        assert_eq!(extract_volume("Series T100"), Some(100));
    }

    #[test]
    fn extract_volume_tome_prefix() {
        assert_eq!(extract_volume("Naruto Tome 3"), Some(3));
        assert_eq!(extract_volume("Asterix Tome 12"), Some(12));
    }

    #[test]
    fn extract_volume_tome_standalone() {
        // Just "Tome XX" without series name
        assert_eq!(extract_volume("Tome 05"), Some(5));
    }

    #[test]
    fn extract_volume_tome_with_series_and_dash() {
        // "Kaiju no8 - Tome 9"
        assert_eq!(extract_volume("Kaiju no8 - Tome 9"), Some(9));
    }

    #[test]
    fn extract_volume_tome_with_subtitle() {
        // "Tome 19 - Pas de Nol pour le père Grommel"
        assert_eq!(
            extract_volume("Tome 19 - Pas de Nol pour le père Grommel"),
            Some(19)
        );
    }

    #[test]
    fn extract_volume_vol_prefix() {
        assert_eq!(extract_volume("Vol.12"), Some(12));
        assert_eq!(extract_volume("Vol 5"), Some(5));
        assert_eq!(extract_volume("Volume 3"), Some(3));
        assert_eq!(extract_volume("Volume3"), Some(3));
    }

    #[test]
    fn extract_volume_hash_prefix() {
        assert_eq!(extract_volume("Issue #42"), Some(42));
        assert_eq!(extract_volume("#007"), Some(7));
    }

    #[test]
    fn extract_volume_trailing_dash_number() {
        assert_eq!(extract_volume("Series - 05"), Some(5));
    }

    #[test]
    fn extract_volume_zero_padded() {
        assert_eq!(extract_volume("T007"), Some(7));
        assert_eq!(extract_volume("T001"), Some(1));
    }

    #[test]
    fn extract_volume_no_match() {
        assert_eq!(extract_volume("Just a title"), None);
        assert_eq!(extract_volume("No numbers here"), None);
        assert_eq!(extract_volume(""), None);
    }

    // ─── extract_volumes ─────────────────────────────────────────────────

    fn sorted(mut v: Vec<i32>) -> Vec<i32> {
        v.sort_unstable();
        v
    }

    #[test]
    fn extract_volumes_individual() {
        assert_eq!(sorted(extract_volumes("One Piece T05")), vec![5]);
        assert_eq!(sorted(extract_volumes("Naruto Tome 12")), vec![12]);
        assert_eq!(sorted(extract_volumes("Vol.03")), vec![3]);
        assert_eq!(sorted(extract_volumes("v07")), vec![7]);
    }

    #[test]
    fn extract_volumes_trailing_bare_number() {
        // Series name + space + number + extension
        assert_eq!(
            sorted(extract_volumes("Shangri-La Frontier 18.cbz")),
            vec![18]
        );
        assert_eq!(
            sorted(extract_volumes("Shangri-la Frontier 01.cbz")),
            vec![1]
        );
        assert_eq!(sorted(extract_volumes("My Series 7.pdf")), vec![7]);
        assert_eq!(sorted(extract_volumes("Some manga 123.epub")), vec![123]);
        // Should NOT trigger when a prefix-based pattern already matched
        assert_eq!(sorted(extract_volumes("One Piece T05 18.cbz")), vec![5]);
        // No space before digits → reject (avoids matching ISBN-like junk)
        assert_eq!(sorted(extract_volumes("Series123.cbz")), Vec::<i32>::new());
        // Filename without extension — covers the rename bug where file_stem()
        // strips the extension before calling extract_volume.
        assert_eq!(sorted(extract_volumes("Shangri-La Frontier 18")), vec![18]);
        assert_eq!(sorted(extract_volumes("Some Series 7")), vec![7]);
    }

    #[test]
    fn extract_volume_works_after_file_stem() {
        // Reproduces the rename flow: filename with and without extension
        // must yield the same volume number.
        assert_eq!(extract_volume("Shangri-La Frontier 18.cbz"), Some(18));
        assert_eq!(extract_volume("Shangri-La Frontier 18"), Some(18));
        assert_eq!(extract_volume("Shangri-la Frontier 01"), Some(1));
        assert_eq!(extract_volume("Shangri-La Frontier 24"), Some(24));
    }

    #[test]
    fn extract_volume_underscore_delimited() {
        // Pattern D: _NN_ or _NN@ (Telegram channel filenames)
        assert_eq!(
            extract_volume("Black_Clover_29_Une_Nuit_Sans_Matin_Yûki_Tabata_2021@BD_fr.cbz"),
            Some(29)
        );
        assert_eq!(
            extract_volume("Black_Clover_30_Bonne_Nouvelle_Yûki_Tabata_2022@BD_fr.cbz"),
            Some(30)
        );
        assert_eq!(extract_volume("One_Piece_1_Romance_Dawn@ch.cbz"), Some(1));
        assert_eq!(
            extract_volume("My_Hero_Academia_42_La_Cavalerie_Est_Là_Kōhei_Horikoshi_2025@BD.cbz"),
            Some(42)
        );
        // 4-digit numbers (years) must not match as volume
        assert_eq!(extract_volume("Series_2021@channel.cbz"), None);
    }

    #[test]
    fn extract_volume_before_author_paren() {
        // Pattern E: " NN (" or " NN@" (author-in-parentheses format)
        assert_eq!(
            extract_volume("Détective Conan 02 (Gosho AOYAMA)@BD_fr.cbz"),
            Some(2)
        );
        assert_eq!(
            extract_volume("Hunter x Hunter 36 (Yoshihiro Togashi)@ch.cbz"),
            Some(36)
        );
        assert_eq!(
            extract_volume("Blacksad 1 (Juan Díaz Canales)@ch.cbz"),
            Some(1)
        );
        // Should not false-positive on series numbers that are not volumes
        assert_eq!(extract_volume("Les 7 Secrets (Author)@ch.cbz"), None);
    }

    #[test]
    fn extract_volumes_glued_t_prefix() {
        // T directly attached to the series name (no separator), 2+ digits
        assert_eq!(
            sorted(extract_volumes("Shangri-la FrontierT01.cbz")),
            vec![1]
        );
        assert_eq!(
            sorted(extract_volumes("Shangri-la Frontiert05.cbz")),
            vec![5]
        );
        assert_eq!(sorted(extract_volumes("NarutoT42")), vec![42]);
        // 1-digit glued form NOT matched (too risky for false positives)
        assert_eq!(sorted(extract_volumes("WordT5")), Vec::<i32>::new());
        // Pure word with no digits not affected
        assert_eq!(sorted(extract_volumes("Asterisk")), Vec::<i32>::new());
        // Digit-then-T-then-digit not matched (e.g., MP3T01 — looks suspicious)
        assert_eq!(sorted(extract_volumes("MP3T01")), Vec::<i32>::new());
    }

    #[test]
    fn extract_volumes_range_dot_separator() {
        let v = sorted(extract_volumes("One Piece T01.T15"));
        assert_eq!(v, (1..=15).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_range_dot_with_brackets() {
        let v = sorted(extract_volumes("Naruto [T001.T104]"));
        assert_eq!(v.len(), 104);
        assert_eq!(v[0], 1);
        assert_eq!(v[103], 104);
    }

    #[test]
    fn extract_volumes_range_dash_separator() {
        let v = sorted(extract_volumes("Dragon Ball T01-T10"));
        assert_eq!(v, (1..=10).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_range_french_a_grave() {
        let v = sorted(extract_volumes("Astérix Tome 01 à Tome 05"));
        assert_eq!(v, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn extract_volumes_range_long_prefix() {
        let v = sorted(extract_volumes("Naruto Tome01.Tome15"));
        assert_eq!(v, (1..=15).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_range_dash_bare_end() {
        let v = sorted(extract_volumes(
            "Compressé.Demon.Slayer.en.couleurs.T17-23.CBZ.Team.Chromatique",
        ));
        assert_eq!(v, (17..=23).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_no_false_positive_version_string() {
        let v = extract_volumes("tool v2.0 release");
        assert!(!v.contains(&0) || v.len() == 1);
    }

    #[test]
    fn extract_volumes_tome_hash_with_accented_chars() {
        let v = sorted(extract_volumes(
            "[Compressé] One Piece [Team Chromatique] - Tome #097 - [V2].cbz",
        ));
        assert!(v.contains(&97), "expected 97 in {:?}", v);
        assert!(
            !v.contains(&2),
            "[V2] should not be extracted as volume 2: {:?}",
            v
        );
    }

    #[test]
    fn extract_volumes_version_in_brackets_ignored() {
        let v = extract_volumes("Naruto T05 [V2].cbz");
        assert_eq!(v, vec![5]);
    }

    #[test]
    fn extract_volumes_tome_hash_single_digit() {
        let v = sorted(extract_volumes(
            "[Compressé] One Piece [Team Chromatique] - Tome #003 (Perfect Edition).cbz",
        ));
        assert!(v.contains(&3), "expected 3 in {:?}", v);
    }

    #[test]
    fn extract_volumes_bare_number_between_dashes() {
        let v = extract_volumes("Les Géants - 07 - Moon.cbz");
        assert_eq!(v, vec![7]);
    }

    #[test]
    fn extract_volumes_bare_number_dash_then_dot() {
        let v = extract_volumes("Les Géants - 07.cbz");
        assert_eq!(v, vec![7]);
    }

    #[test]
    fn extract_volumes_bare_number_at_start_dot() {
        let v = extract_volumes("06. yatho.cbz");
        assert_eq!(v, vec![6]);
    }

    #[test]
    fn extract_volumes_bare_number_at_start_dash() {
        let v = extract_volumes("07 - Moon.cbz");
        assert_eq!(v, vec![7]);
    }

    #[test]
    fn extract_volumes_bare_number_no_false_positive_with_prefix() {
        let v = extract_volumes("Naruto T05 - some 99 extra.cbz");
        assert_eq!(v, vec![5], "should only find T05, not bare 99");
    }

    #[test]
    fn extract_volumes_dash_number_dash_no_spaces() {
        assert_eq!(
            sorted(extract_volumes("Largo Winch -21- L'étoile du matin.cbr")),
            vec![21]
        );
        assert_eq!(sorted(extract_volumes("Largo Winch -05- H.cbr")), vec![5]);
        assert_eq!(
            sorted(extract_volumes("Largo winch -22- Les Voiles écarlates.cbz")),
            vec![22]
        );
    }

    #[test]
    fn extract_volumes_empty_string() {
        assert_eq!(extract_volumes(""), Vec::<i32>::new());
    }

    #[test]
    fn extract_volumes_no_volumes_plain_text() {
        assert_eq!(
            extract_volumes("Some random title without volumes"),
            Vec::<i32>::new()
        );
    }

    #[test]
    fn extract_volumes_hash_prefix() {
        assert_eq!(sorted(extract_volumes("Issue #42")), vec![42]);
    }

    #[test]
    fn extract_volumes_multiple_individual() {
        let v = sorted(extract_volumes("Pack Naruto T01 T05 T10"));
        assert_eq!(v, vec![1, 5, 10]);
    }

    #[test]
    fn extract_volumes_vol_space_prefix() {
        assert_eq!(sorted(extract_volumes("Vol 7 - Special")), vec![7]);
    }

    #[test]
    fn extract_volumes_vol_dot_prefix() {
        assert_eq!(sorted(extract_volumes("Vol.12 collector")), vec![12]);
    }

    #[test]
    fn extract_volumes_leading_zeros() {
        assert_eq!(sorted(extract_volumes("T0001")), vec![1]);
        assert_eq!(sorted(extract_volumes("Tome 007")), vec![7]);
    }

    #[test]
    fn extract_volumes_range_single_volume_not_range() {
        let v = extract_volumes("One Piece T05 [FR]");
        assert_eq!(v, vec![5]);
    }

    #[test]
    fn extract_volumes_range_large_gap_rejected() {
        let v = extract_volumes("Archive T001.T999");
        assert!(v.len() <= 2, "should not expand huge range, got {:?}", v);
    }

    #[test]
    fn extract_volumes_range_equal_numbers_not_expanded() {
        let v = sorted(extract_volumes("Pack T05.T05"));
        assert_eq!(v, vec![5]);
    }

    #[test]
    fn extract_volumes_range_reversed_not_expanded() {
        let v = sorted(extract_volumes("Pack T10.T05"));
        assert_eq!(v, vec![5, 10]);
    }

    #[test]
    fn extract_volumes_unicode_accented_series_name() {
        let v = sorted(extract_volumes("Série Éphémère T03 - Résumé.cbz"));
        assert_eq!(v, vec![3]);
    }

    #[test]
    fn extract_volumes_tome_with_dot_separator() {
        let v = sorted(extract_volumes("Series Tome.05.cbz"));
        assert_eq!(v, vec![5]);
    }

    #[test]
    fn extract_volumes_tome_with_underscore_separator() {
        assert_eq!(extract_volumes("Cyborgs_Tome_01_Ronin_fr.cbz"), vec![1]);
        assert_eq!(extract_volumes("Series_Tome_12.pdf"), vec![12]);
    }

    #[test]
    fn extract_volumes_t_with_underscore_separator() {
        assert_eq!(extract_volumes("Series_T_05.cbz"), vec![5]);
        assert_eq!(extract_volumes("Series_T05.cbz"), vec![5]);
    }

    #[test]
    fn extract_volumes_tome_space_standalone() {
        assert_eq!(extract_volumes("Tome 05.cbz"), vec![5]);
    }

    #[test]
    fn extract_volumes_tome_in_series_title() {
        assert_eq!(extract_volumes("Kaiju no8 - Tome 9.cbz"), vec![9]);
    }

    #[test]
    fn extract_volumes_tome_with_subtitle() {
        assert_eq!(
            extract_volumes("Tome 19 - Pas de Nol pour le père Grommel.pdf"),
            vec![19]
        );
    }

    #[test]
    fn extract_volumes_v_prefix_not_in_brackets() {
        assert_eq!(sorted(extract_volumes("Series v03 [1080p]")), vec![3]);
    }

    #[test]
    fn extract_volumes_bare_number_dash_end_of_string() {
        let v = extract_volumes("Series - 12");
        assert_eq!(v, vec![12]);
    }

    #[test]
    fn extract_volumes_bare_number_at_start_space() {
        let v = extract_volumes("03 title.cbz");
        assert_eq!(v, vec![3]);
    }

    #[test]
    fn extract_volumes_bare_number_at_start_underscore() {
        let v = extract_volumes("34_Increvables.pdf");
        assert_eq!(v, vec![34]);
    }

    #[test]
    fn extract_volumes_bare_number_underscore_various() {
        assert_eq!(extract_volumes("1_Tom_tom.pdf"), vec![1]);
        assert_eq!(extract_volumes("07_Dr_le_de_cirque.pdf"), vec![7]);
    }

    #[test]
    fn extract_volumes_range_with_spaces_around_dash() {
        let v = sorted(extract_volumes("Pack T01 - T10 [FR]"));
        assert_eq!(v, (1..=10).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_range_with_a_no_accent() {
        let v = sorted(extract_volumes("Tom Tom et Nana - T01 a T34 [PDF] Fr"));
        assert_eq!(v, (1..=34).collect::<Vec<_>>());
    }

    #[test]
    fn extract_volumes_range_with_a_grave_and_spaces() {
        let v = sorted(extract_volumes("Collection Tome 1 à Tome 5"));
        assert_eq!(v, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn extract_volumes_does_not_duplicate() {
        let v = extract_volumes("T05 - also Tome 05");
        assert_eq!(v.iter().filter(|&&x| x == 5).count(), 1);
    }

    // ─── read_bare_number ───────────────────────────────────────────────

    #[test]
    fn read_bare_number_at_start() {
        let chars: Vec<char> = "42abc".chars().collect();
        assert_eq!(read_bare_number(&chars, 0), Some((42, 2)));
    }

    #[test]
    fn read_bare_number_no_digits() {
        let chars: Vec<char> = "abc".chars().collect();
        assert_eq!(read_bare_number(&chars, 0), None);
    }

    #[test]
    fn read_bare_number_at_offset() {
        let chars: Vec<char> = "abc123def".chars().collect();
        assert_eq!(read_bare_number(&chars, 3), Some((123, 6)));
    }

    // ─── read_vol_prefix_number ─────────────────────────────────────────

    #[test]
    fn read_vol_prefix_number_tome() {
        let chars: Vec<char> = "tome 05 extra".chars().collect();
        assert_eq!(read_vol_prefix_number(&chars, 0), Some((5, 7)));
    }

    #[test]
    fn read_vol_prefix_number_t_prefix() {
        let chars: Vec<char> = "t12".chars().collect();
        assert_eq!(read_vol_prefix_number(&chars, 0), Some((12, 3)));
    }

    #[test]
    fn read_vol_prefix_number_boundary_check() {
        let chars: Vec<char> = "at12".chars().collect();
        assert_eq!(read_vol_prefix_number(&chars, 1), None);
    }

    #[test]
    fn read_vol_prefix_number_no_digits_after_prefix() {
        let chars: Vec<char> = "tome abc".chars().collect();
        assert_eq!(read_vol_prefix_number(&chars, 0), None);
    }

    #[test]
    fn read_vol_prefix_number_hash() {
        let chars: Vec<char> = "#007 extra".chars().collect();
        assert_eq!(read_vol_prefix_number(&chars, 0), Some((7, 4)));
    }

    // ─── extract_series ──────────────────────────────────────────────────

    #[test]
    fn extract_series_simple() {
        let path = Path::new("/libraries/manga/One Piece/T01.cbz");
        let root = Path::new("/libraries/manga");
        assert_eq!(extract_series(path, root), Some("One Piece".to_string()));
    }

    #[test]
    fn extract_series_nested_non_hs_subfolder() {
        let path = Path::new("/libraries/bd/Asterix/subfolder/file.cbz");
        let root = Path::new("/libraries/bd");
        // Non-HS subfolder: use immediate parent as series
        assert_eq!(extract_series(path, root), Some("subfolder".to_string()));
    }

    #[test]
    fn extract_series_file_at_root() {
        let path = Path::new("/libraries/manga/standalone.cbz");
        let root = Path::new("/libraries/manga");
        // File directly in root → no series directory
        assert_eq!(extract_series(path, root), None);
    }

    #[test]
    fn extract_series_unrelated_path() {
        let path = Path::new("/other/path/file.cbz");
        let root = Path::new("/libraries/manga");
        // Path doesn't start with root
        assert_eq!(extract_series(path, root), None);
    }

    // ─── parse_metadata_fast ─────────────────────────────────────────────

    #[test]
    fn parse_metadata_fast_extracts_all() {
        let path = Path::new("/libraries/manga/One Piece/One Piece T05.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.title, "One Piece T05");
        assert_eq!(meta.series, Some("One Piece".to_string()));
        assert_eq!(meta.volume, Some(5));
        assert_eq!(meta.page_count, None);
    }

    #[test]
    fn parse_metadata_fast_no_volume() {
        let path = Path::new("/libraries/bd/Asterix/Asterix le Gaulois.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.title, "Asterix le Gaulois");
        assert_eq!(meta.series, Some("Asterix".to_string()));
        assert_eq!(meta.volume, None);
    }

    #[test]
    fn parse_metadata_fast_no_extension() {
        let path = Path::new("/libraries/manga/Series/Untitled");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.title, "Untitled");
        assert_eq!(meta.series, Some("Series".to_string()));
    }

    // ─── detect_format ───────────────────────────────────────────────────

    #[test]
    fn detect_format_case_insensitive() {
        assert_eq!(detect_format(Path::new("test.PDF")), Some(BookFormat::Pdf));
        assert_eq!(
            detect_format(Path::new("test.Epub")),
            Some(BookFormat::Epub)
        );
        assert_eq!(detect_format(Path::new("test.CbR")), Some(BookFormat::Cbr));
    }

    // ─── BookFormat::as_str ──────────────────────────────────────────────

    #[test]
    fn book_format_as_str() {
        assert_eq!(BookFormat::Cbz.as_str(), "cbz");
        assert_eq!(BookFormat::Cbr.as_str(), "cbr");
        assert_eq!(BookFormat::Pdf.as_str(), "pdf");
        assert_eq!(BookFormat::Epub.as_str(), "epub");
    }

    // ─── clean_title ─────────────────────────────────────────────────────

    #[test]
    fn clean_title_removes_volume_patterns() {
        assert_eq!(clean_title("One Piece T05"), "One Piece");
        assert_eq!(clean_title("Naruto Vol.12"), "Naruto");
        assert_eq!(clean_title("Series Volume 3"), "Series");
        assert_eq!(clean_title("Issue #42"), "Issue");
        assert_eq!(clean_title("Series - 05"), "Series");
    }

    #[test]
    fn clean_title_no_volume() {
        assert_eq!(clean_title("Just a title"), "Just a title");
    }

    // ─── extract_hs_info ────────────────────────────────────────────────

    #[test]
    fn hs_basic() {
        let (num, _) = extract_hs_info("Asterix HS1").unwrap();
        assert_eq!(num, Some(1));
    }

    #[test]
    fn hs_dot_separator() {
        let (num, _) = extract_hs_info("Asterix HS.02").unwrap();
        assert_eq!(num, Some(2));
    }

    #[test]
    fn hs_space_separator() {
        let (num, _) = extract_hs_info("Asterix HS 03").unwrap();
        assert_eq!(num, Some(3));
    }

    #[test]
    fn hs_no_number() {
        let (num, cleaned) = extract_hs_info("Asterix HS").unwrap();
        assert_eq!(num, None);
        assert_eq!(cleaned, "Asterix");
    }

    #[test]
    fn hs_at_end_with_prefix() {
        let result = extract_hs_info("Boruto - Two Blue Vortex HS");
        assert!(
            result.is_some(),
            "should detect HS at end of 'Boruto - Two Blue Vortex HS'"
        );
        let (num, cleaned) = result.unwrap();
        assert_eq!(num, None);
        assert_eq!(cleaned, "Boruto - Two Blue Vortex");
    }

    #[test]
    fn hors_serie_with_accent() {
        let (num, _) = extract_hs_info("Naruto Hors-Série 3").unwrap();
        assert_eq!(num, Some(3));
    }

    #[test]
    fn hors_serie_without_accent() {
        let (num, _) = extract_hs_info("Naruto Hors Serie").unwrap();
        assert_eq!(num, None);
    }

    #[test]
    fn special_with_accent() {
        let (num, _) = extract_hs_info("One Piece Spécial 2").unwrap();
        assert_eq!(num, Some(2));
    }

    #[test]
    fn special_without_accent() {
        let (num, _) = extract_hs_info("One Piece Special").unwrap();
        assert_eq!(num, None);
    }

    #[test]
    fn bonus_pattern() {
        let (num, cleaned) = extract_hs_info("Donjon Bonus - Clefs en Mains").unwrap();
        assert_eq!(num, None);
        assert!(!cleaned.is_empty());
    }

    #[test]
    fn hs_not_in_word() {
        // "hsk" should not match "hs"
        assert!(extract_hs_info("The HSK Guide").is_none());
    }

    #[test]
    fn regular_volume_not_hs() {
        assert!(extract_hs_info("Asterix T05").is_none());
        assert!(extract_hs_info("Naruto Tome 12").is_none());
    }

    #[test]
    fn hs_volume_type_in_parse() {
        let path = Path::new("/libraries/test/Asterix/Asterix HS2.cbz");
        let root = Path::new("/libraries/test");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.volume_type, VolumeType::Hs);
        assert_eq!(meta.volume, Some(2));
    }

    #[test]
    fn regular_volume_type_in_parse() {
        let path = Path::new("/libraries/test/Asterix/Asterix T05.cbz");
        let root = Path::new("/libraries/test");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.volume_type, VolumeType::Regular);
        assert_eq!(meta.volume, Some(5));
    }

    #[test]
    fn hs_subfolder_keeps_parent_series() {
        let path = Path::new("/libraries/manga/Boruto/Hors-Série/Boruto HS.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Boruto".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Hs);
    }

    #[test]
    fn specials_subfolder_keeps_parent_series() {
        let path = Path::new("/libraries/bd/Asterix/Specials/Asterix Special 2.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Asterix".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Hs);
        assert_eq!(meta.volume, Some(2));
    }

    #[test]
    fn nested_series_uses_immediate_parent() {
        let path = Path::new("/libraries/manga/Shonen/Dragon Ball/Dragon Ball T01.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Dragon Ball".to_string()));
    }

    // ─── oneshot folder ───────────────────────────────────────────────────────

    #[test]
    fn oneshot_folder_uses_filename_as_series() {
        let path = Path::new("/libraries/bd/Oneshots/Blacksad.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Blacksad".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Oneshot);
        assert_eq!(meta.volume, None);
    }

    #[test]
    fn oneshot_folder_case_insensitive() {
        let path = Path::new("/libraries/bd/ONESHOTS/My One Shot.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("My One Shot".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Oneshot);
    }

    #[test]
    fn oneshot_folder_hyphenated() {
        let path = Path::new("/libraries/bd/One-Shots/Persepolis.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Persepolis".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Oneshot);
    }

    #[test]
    fn oneshot_folder_underscore_prefix() {
        let path = Path::new("/libraries/bd/_Oneshots/Maus.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Maus".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Oneshot);
    }

    #[test]
    fn non_oneshot_folder_not_affected() {
        // "Oneshots" nested 2 levels deep — NOT treated as oneshot folder
        let path = Path::new("/libraries/bd/MySeries/Oneshots/Extra.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        // Not oneshot type — the 2-level check correctly prevents matching
        assert_ne!(meta.volume_type, VolumeType::Oneshot);
        // Series is the immediate parent dir (normal behaviour)
        assert_eq!(meta.series, Some("Oneshots".to_string()));
    }

    #[test]
    fn nested_series_with_hs_subfolder() {
        let path = Path::new("/libraries/manga/Shonen/Dragon Ball/Hors-Série/Dragon Ball HS1.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Dragon Ball".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Hs);
    }

    #[test]
    fn flat_series_still_works() {
        let path = Path::new("/libraries/manga/One Piece/One Piece T05.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("One Piece".to_string()));
    }

    #[test]
    fn file_at_library_root_is_unclassified() {
        let path = Path::new("/libraries/manga/standalone.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, None);
    }

    #[test]
    fn bonus_subfolder_keeps_parent_series() {
        let path = Path::new("/libraries/bd/Tintin/Bonus/Tintin Bonus.cbz");
        let root = Path::new("/libraries/bd");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Tintin".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Hs);
    }

    #[test]
    fn extras_subfolder_keeps_parent_series() {
        let path = Path::new("/libraries/manga/Naruto/Extras/Naruto Special.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Naruto".to_string()));
    }

    // ─── extract_int_info ─────────────────────────────────────────────

    #[test]
    fn int_basic() {
        let (num, cleaned) = extract_int_info("Dragon Ball INT1").unwrap();
        assert_eq!(num, Some(1));
        assert_eq!(cleaned, "Dragon Ball");
    }

    #[test]
    fn int_no_number() {
        let (num, cleaned) = extract_int_info("Dragon Ball INT").unwrap();
        assert_eq!(num, None);
        assert_eq!(cleaned, "Dragon Ball");
    }

    #[test]
    fn int_dot_separator() {
        let (num, _) = extract_int_info("Asterix INT.02").unwrap();
        assert_eq!(num, Some(2));
    }

    #[test]
    fn int_space_separator() {
        let (num, _) = extract_int_info("Asterix INT 3").unwrap();
        assert_eq!(num, Some(3));
    }

    #[test]
    fn inths_pattern() {
        let (num, cleaned) = extract_int_info("Dragon Ball INTHS").unwrap();
        assert_eq!(num, None);
        assert_eq!(cleaned, "Dragon Ball");
    }

    #[test]
    fn inths_with_number() {
        let (num, cleaned) = extract_int_info("Dragon Ball INTHS1").unwrap();
        assert_eq!(num, Some(1));
        assert_eq!(cleaned, "Dragon Ball");
    }

    #[test]
    fn integrale_full_word() {
        let (num, cleaned) = extract_int_info("Naruto Intégrale 5").unwrap();
        assert_eq!(num, Some(5));
        assert_eq!(cleaned, "Naruto");
    }

    #[test]
    fn integrale_without_accent() {
        let (num, _) = extract_int_info("Naruto Integrale 3").unwrap();
        assert_eq!(num, Some(3));
    }

    #[test]
    fn int_not_in_word() {
        // "international" should NOT match
        assert!(extract_int_info("International Guide").is_none());
    }

    #[test]
    fn int_not_interest() {
        assert!(extract_int_info("Interesting Story").is_none());
    }

    #[test]
    fn int_volume_type_in_parse() {
        let path = Path::new("/libraries/test/Asterix/Asterix INT2.cbz");
        let root = Path::new("/libraries/test");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.volume_type, VolumeType::Integral);
        assert_eq!(meta.volume, Some(2));
    }

    #[test]
    fn inths_volume_type_in_parse() {
        let path = Path::new("/libraries/test/Asterix/Asterix INTHS1.cbz");
        let root = Path::new("/libraries/test");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.volume_type, VolumeType::Integral);
        assert_eq!(meta.volume, Some(1));
    }

    #[test]
    fn integrales_subfolder_keeps_parent_series() {
        let path = Path::new("/libraries/manga/Naruto/Intégrales/Naruto INT3.cbz");
        let root = Path::new("/libraries/manga");
        let meta = parse_metadata_fast(path, BookFormat::Cbz, root);
        assert_eq!(meta.series, Some("Naruto".to_string()));
        assert_eq!(meta.volume_type, VolumeType::Integral);
        assert_eq!(meta.volume, Some(3));
    }

    #[test]
    fn is_hs_subfolder_patterns() {
        assert!(is_hs_subfolder("Hors-Série"));
        assert!(is_hs_subfolder("hors-serie"));
        assert!(is_hs_subfolder("HS"));
        assert!(is_hs_subfolder("Specials"));
        assert!(is_hs_subfolder("Bonus"));
        assert!(is_hs_subfolder("Extras"));
        assert!(is_hs_subfolder("extra"));
        assert!(!is_hs_subfolder("Dragon Ball"));
        assert!(!is_hs_subfolder("Tome 1"));
        assert!(!is_hs_subfolder("Shonen"));
    }

    // ─── CBZ_INDEX_CACHE invalidation on mtime change ─────────────────────
    //
    // Regression test for a bug where the global cache was keyed only on the
    // file path. After replacing a CBZ in place (different content, same path),
    // the parser kept returning the OLD image list — leading to "page out of
    // range" errors and stale page renders.

    fn write_single_image_cbz(path: &Path, image_name: &str) -> std::io::Result<()> {
        use std::io::Write;
        let file = std::fs::File::create(path)?;
        let mut zip = ::zip::ZipWriter::new(file);
        let opts: ::zip::write::SimpleFileOptions = ::zip::write::SimpleFileOptions::default()
            .compression_method(::zip::CompressionMethod::Stored);
        zip.start_file(image_name, opts)
            .map_err(std::io::Error::other)?;
        // Minimal valid JPEG: just the SOI/EOI markers — enough to satisfy is_image_name.
        zip.write_all(&[0xFF, 0xD8, 0xFF, 0xD9])?;
        zip.finish().map_err(std::io::Error::other)?;
        Ok(())
    }

    #[test]
    fn cbz_index_cache_invalidates_on_mtime_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbz");

        // First version: a single image "001.jpg". Read once to populate cache.
        write_single_image_cbz(&path, "001.jpg").unwrap();
        let mtime_before = std::fs::metadata(&path).unwrap().modified().unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut archive = ::zip::ZipArchive::new(file).unwrap();
        let names = get_cbz_image_index(&path, &mut archive);
        assert_eq!(names, vec!["001.jpg".to_string()]);

        // Replace the file in place with a new image. Force a future mtime so
        // the test is robust regardless of filesystem mtime resolution.
        write_single_image_cbz(&path, "002.jpg").unwrap();
        let new_mtime = mtime_before + std::time::Duration::from_secs(10);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(new_mtime)
            .unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let mut archive = ::zip::ZipArchive::new(file).unwrap();
        let names = get_cbz_image_index(&path, &mut archive);
        assert_eq!(
            names,
            vec!["002.jpg".to_string()],
            "cache must invalidate when file mtime changes"
        );
    }

    #[test]
    fn cbz_index_cache_returns_cached_on_unchanged_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbz");
        write_single_image_cbz(&path, "001.jpg").unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let mut archive = ::zip::ZipArchive::new(file).unwrap();
        let first = get_cbz_image_index(&path, &mut archive);

        // Second call — mtime unchanged, should return cached value.
        let file = std::fs::File::open(&path).unwrap();
        let mut archive = ::zip::ZipArchive::new(file).unwrap();
        let second = get_cbz_image_index(&path, &mut archive);

        assert_eq!(first, second);
    }
}
