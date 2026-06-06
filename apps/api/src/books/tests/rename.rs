use super::*;

fn make_book(title: &str, volume: Option<i32>, authors: Vec<&str>, abs_path: &str) -> BookFileData {
    make_book_with_type(title, volume, authors, abs_path, "regular")
}

fn make_book_with_type(
    title: &str,
    volume: Option<i32>,
    authors: Vec<&str>,
    abs_path: &str,
    volume_type: &str,
) -> BookFileData {
    BookFileData {
        book_id: Uuid::new_v4(),
        title: title.to_string(),
        authors: authors.into_iter().map(String::from).collect(),
        volume,
        volume_type: volume_type.to_string(),
        publish_date: None,
        isbn: None,
        abs_path: abs_path.to_string(),
        file_id: Uuid::new_v4(),
    }
}

// -- apply_template --

#[test]
fn basic_template() {
    let book = make_book(
        "Son Goku et ses amis",
        Some(1),
        vec!["Akira Toriyama"],
        "/libraries/BD/old.cbz",
    );
    let result = apply_template(
        "{series_name} - T{volume_padded} - {title}",
        "Dragon Ball",
        &book,
        42,
    );
    assert_eq!(result, "Dragon Ball - T01 - Son Goku et ses amis");
}

#[test]
fn volume_padding_two_digits() {
    let book = make_book("Title", Some(3), vec![], "/libraries/BD/old.cbz");
    let result = apply_template("{volume_padded}", "S", &book, 25);
    assert_eq!(result, "03");
}

#[test]
fn volume_padding_three_digits() {
    let book = make_book("Title", Some(5), vec![], "/libraries/BD/old.cbz");
    let result = apply_template("{volume_padded}", "S", &book, 150);
    assert_eq!(result, "005");
}

#[test]
fn volume_padding_single_digit_series() {
    let book = make_book("Title", Some(3), vec![], "/libraries/BD/old.cbz");
    let result = apply_template("{volume_padded}", "S", &book, 5);
    assert_eq!(result, "3");
}

#[test]
fn volume_padding_four_digits() {
    let book = make_book("Title", Some(5), vec![], "/libraries/BD/old.cbz");
    let result = apply_template("{volume_padded}", "S", &book, 1200);
    assert_eq!(result, "0005");
}

#[test]
fn null_volume_removes_entire_segment() {
    let book = make_book("Title", None, vec!["Author"], "/libraries/BD/old.cbz");
    let result = apply_template(
        "{series_name} - T{volume_padded} - {title}",
        "Dragon Ball",
        &book,
        10,
    );
    assert_eq!(result, "Dragon Ball - Title");
}

#[test]
fn null_volume_removes_segment_without_prefix() {
    let book = make_book("Title", None, vec!["Author"], "/libraries/BD/old.cbz");
    let result = apply_template(
        "{series_name} - {volume_padded} - {title}",
        "Dragon Ball",
        &book,
        10,
    );
    assert_eq!(result, "Dragon Ball - Title");
}

#[test]
fn empty_authors_fallback_to_unknown() {
    let book = make_book("Title", Some(1), vec![], "/libraries/BD/old.cbz");
    let result = apply_template("{authors}", "S", &book, 1);
    assert_eq!(result, "Unknown");
}

#[test]
fn multiple_authors_joined() {
    let book = make_book(
        "Title",
        Some(1),
        vec!["Author A", "Author B"],
        "/libraries/BD/old.cbz",
    );
    let result = apply_template("{authors}", "S", &book, 1);
    assert_eq!(result, "Author A, Author B");
}

#[test]
fn unknown_variable_removed() {
    let book = make_book("Title", Some(1), vec![], "/libraries/BD/old.cbz");
    let result = apply_template("{series_name} - {unknown_var} - {title}", "S", &book, 1);
    assert_eq!(result, "S - Title");
}

#[test]
fn publish_date_and_isbn() {
    let mut book = make_book("Title", Some(1), vec![], "/libraries/BD/old.cbz");
    book.publish_date = Some("2023-01-15".to_string());
    book.isbn = Some("978-123".to_string());
    let result = apply_template("{title} ({publish_date}) [{isbn}]", "S", &book, 1);
    assert_eq!(result, "Title (2023-01-15) [978-123]");
}

#[test]
fn null_publish_date_cleaned_up() {
    let book = make_book("Title", Some(1), vec![], "/libraries/BD/old.cbz");
    let result = apply_template("{title} - {publish_date}", "S", &book, 1);
    assert_eq!(result, "Title");
}

// -- sanitize_filename --

#[test]
fn sanitize_replaces_forbidden_chars() {
    assert_eq!(
        sanitize_filename("a/b\\c:d*e?f\"g<h>i|j"),
        "a_b_c_d_e_f_g_h_i_j"
    );
}

#[test]
fn sanitize_trims_whitespace_and_dots() {
    assert_eq!(sanitize_filename("  ..hello world..  "), "hello world");
}

#[test]
fn sanitize_truncates_long_names() {
    let long_name = "a".repeat(250);
    let result = sanitize_filename(&long_name);
    assert_eq!(result.len(), 200);
}

#[test]
fn sanitize_normal_name_unchanged() {
    assert_eq!(
        sanitize_filename("Dragon Ball - T01 - Son Goku"),
        "Dragon Ball - T01 - Son Goku"
    );
}

// -- deduplicate_filenames --

fn make_entry(old: &str, new: &str, new_path: &str) -> RenameEntry {
    RenameEntry {
        book_id: Uuid::new_v4(),
        old_filename: old.to_string(),
        new_filename: new.to_string(),
        old_path: format!("/libraries/BD/{}", old),
        new_path: new_path.to_string(),
        changed: old != new,
    }
}

#[test]
fn no_duplicates_unchanged() {
    let mut entries = vec![
        make_entry("old1.cbz", "new1.cbz", "/libraries/BD/new1.cbz"),
        make_entry("old2.cbz", "new2.cbz", "/libraries/BD/new2.cbz"),
    ];
    deduplicate_filenames(&mut entries);
    assert_eq!(entries[0].new_filename, "new1.cbz");
    assert_eq!(entries[1].new_filename, "new2.cbz");
}

#[test]
fn duplicates_get_suffix() {
    let mut entries = vec![
        make_entry("old1.cbz", "same.cbz", "/libraries/BD/same.cbz"),
        make_entry("old2.cbz", "same.cbz", "/libraries/BD/same.cbz"),
        make_entry("old3.cbz", "same.cbz", "/libraries/BD/same.cbz"),
    ];
    deduplicate_filenames(&mut entries);
    assert_eq!(entries[0].new_filename, "same.cbz");
    assert_eq!(entries[1].new_filename, "same (2).cbz");
    assert_eq!(entries[2].new_filename, "same (3).cbz");
}

#[test]
fn duplicate_without_extension() {
    let mut entries = vec![
        make_entry("old1", "same", "/libraries/BD/same"),
        make_entry("old2", "same", "/libraries/BD/same"),
    ];
    deduplicate_filenames(&mut entries);
    assert_eq!(entries[0].new_filename, "same");
    assert_eq!(entries[1].new_filename, "same (2)");
}

#[test]
fn dedup_updates_new_path() {
    let mut entries = vec![
        make_entry("old1.cbz", "same.cbz", "/libraries/BD/same.cbz"),
        make_entry("old2.cbz", "same.cbz", "/libraries/BD/same.cbz"),
    ];
    deduplicate_filenames(&mut entries);
    assert!(entries[1].new_path.ends_with("same (2).cbz"));
}

#[test]
fn mixed_duplicates_and_unique() {
    let mut entries = vec![
        make_entry("a.cbz", "dup.cbz", "/libraries/BD/dup.cbz"),
        make_entry("b.cbz", "unique.cbz", "/libraries/BD/unique.cbz"),
        make_entry("c.cbz", "dup.cbz", "/libraries/BD/dup.cbz"),
    ];
    deduplicate_filenames(&mut entries);
    assert_eq!(entries[0].new_filename, "dup.cbz");
    assert_eq!(entries[1].new_filename, "unique.cbz");
    assert_eq!(entries[2].new_filename, "dup (2).cbz");
}

// -- full flow simulation --

#[test]
fn full_flow_42_books_with_mixed_volumes() {
    let template = "{series_name} - T{volume_padded} - {title}";
    let series_name = "dragon ball";
    let max_volume = 42i64;

    // Simulate books with various edge cases
    let test_cases: Vec<(Option<i32>, &str)> = vec![
        (Some(1), "Le secret du pouvoir surhumain"),
        (Some(2), "Kamehameha"),
        (Some(10), "Le miracle"),
        (Some(42), "La victoire"),
        (None, "Hors s\u{00e9}rie sp\u{00e9}cial"),
        (Some(3), "L'\u{00e9}preuve"),
    ];

    for (vol, title) in &test_cases {
        let book = make_book(title, *vol, vec!["Akira Toriyama"], "/libraries/BD/old.cbz");
        let result = apply_template(template, series_name, &book, max_volume);
        let result = sanitize_filename(&result);
        // Should never be empty
        assert!(
            !result.is_empty(),
            "empty result for vol={:?} title={}",
            vol,
            title
        );
        // Should not contain null bytes
        assert!(!result.contains('\x00'), "null byte in result: {}", result);
    }

    // Check specific outputs
    let book1 = make_book("Le secret", Some(1), vec![], "/libraries/BD/old.cbz");
    assert_eq!(
        apply_template(template, series_name, &book1, max_volume),
        "dragon ball - T01 - Le secret"
    );

    let book42 = make_book("La victoire", Some(42), vec![], "/libraries/BD/old.cbz");
    assert_eq!(
        apply_template(template, series_name, &book42, max_volume),
        "dragon ball - T42 - La victoire"
    );

    let book_hs = make_book("Hors s\u{00e9}rie", None, vec![], "/libraries/BD/old.cbz");
    assert_eq!(
        apply_template(template, series_name, &book_hs, max_volume),
        "dragon ball - Hors s\u{00e9}rie"
    );
}

// -- volume_type with separate templates --

#[test]
fn hs_book_uses_hs_template() {
    let book = make_book_with_type(
        "Spécial été",
        Some(2),
        vec![],
        "/libraries/BD/old.cbz",
        "hs",
    );
    let hs_template = "{series_name} - HS {volume_padded}";
    let result = apply_template(hs_template, "Dragon Ball", &book, 10);
    assert_eq!(result, "Dragon Ball - HS 02");
}

#[test]
fn hs_book_without_volume() {
    let book = make_book_with_type("Bonus", None, vec![], "/libraries/BD/old.cbz", "hs");
    let hs_template = "{series_name} - HS {volume_padded}";
    let result = apply_template(hs_template, "Naruto", &book, 10);
    assert_eq!(result, "Naruto - HS");
}

#[test]
fn regular_book_not_affected_by_hs_template() {
    let book = make_book_with_type(
        "Chapter 1",
        Some(1),
        vec![],
        "/libraries/BD/old.cbz",
        "regular",
    );
    let regular_template = "{series_name} - T{volume_padded}";
    let result = apply_template(regular_template, "One Piece", &book, 100);
    assert_eq!(result, "One Piece - T001");
}

#[test]
fn oneshot_book_uses_regular_template() {
    let book = make_book_with_type(
        "Le Monde sans fin",
        None,
        vec![],
        "/libraries/BD/old.cbz",
        "oneshot",
    );
    let template = "{series_name} - T{volume_padded} - {title}";
    let result = apply_template(template, "Le Monde sans fin", &book, 1);
    assert_eq!(result, "Le Monde sans fin - Le Monde sans fin");
}

#[test]
fn no_double_extension_when_series_name_contains_ext() {
    let book = make_book(
        "La S\u{00e9}paration",
        Some(1),
        vec![],
        "/libraries/BD/Avengers.cbr/old.cbr",
    );
    let template = "{series_name} - T{volume_padded} - {title}";
    let series_name = "Avengers - La S\u{00e9}paration.cbr";
    let new_stem = apply_template(template, series_name, &book, 1);
    let new_stem = sanitize_filename(&new_stem);
    let extension = ".cbr";
    let new_filename = if new_stem.to_lowercase().ends_with(&extension.to_lowercase()) {
        new_stem.clone()
    } else {
        format!("{}{}", new_stem, extension)
    };
    assert!(
        !new_filename.ends_with(".cbr.cbr"),
        "double extension detected: {}",
        new_filename
    );
    assert!(new_filename.ends_with(".cbr"));
}

#[test]
fn template_with_special_chars_in_title() {
    let book = make_book(
        "L'\u{00e9}preuve: le retour! (2\u{00e8}me \u{00e9}dition)",
        Some(5),
        vec!["Auteur"],
        "/libraries/BD/old.cbz",
    );
    let result = apply_template(
        "{series_name} - T{volume_padded} - {title}",
        "S\u{00e9}rie \u{00e0} accents",
        &book,
        10,
    );
    let sanitized = sanitize_filename(&result);
    assert_eq!(sanitized, "S\u{00e9}rie \u{00e0} accents - T05 - L'\u{00e9}preuve_ le retour! (2\u{00e8}me \u{00e9}dition)");
}

// --- Volume extraction fallback from filename ---

#[test]
fn volume_extracted_from_filename_when_db_null_tome() {
    let book = make_book("Tome 05", None, vec![], "/libraries/BD/Frieren/Tome 05.cbz");
    let result = apply_template("{series_name} - T{volume_padded}", "Frieren", &book, 14);
    assert_eq!(result, "Frieren - T05");
}

#[test]
fn volume_extracted_from_filename_when_db_null_t_prefix() {
    let book = make_book(
        "Frieren \u{2013} T10",
        None,
        vec![],
        "/libraries/BD/Frieren/Frieren \u{2013} T10.cbz",
    );
    let result = apply_template("{series_name} - T{volume_padded}", "Frieren", &book, 14);
    assert_eq!(result, "Frieren - T10");
}

#[test]
fn volume_db_takes_precedence_over_filename() {
    let book = make_book(
        "Tome 05",
        Some(3),
        vec![],
        "/libraries/BD/Frieren/Tome 05.cbz",
    );
    let result = apply_template("{series_name} - T{volume_padded}", "Frieren", &book, 14);
    assert_eq!(result, "Frieren - T03");
}

#[test]
fn volume_none_and_no_volume_in_filename() {
    let book = make_book(
        "Special Edition",
        None,
        vec![],
        "/libraries/BD/Frieren/Special Edition.cbz",
    );
    let result = apply_template("{series_name} - T{volume_padded}", "Frieren", &book, 10);
    assert_eq!(result, "Frieren");
}

// --- Post-rename DB update: title + volume extraction ---

#[test]
fn post_rename_extracts_title_and_volume() {
    let new_path = std::path::Path::new("/libraries/BD/Frieren/Frieren - T05.cbz");
    let new_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap();
    let new_volume = parsers::extract_volume(new_stem);
    assert_eq!(new_stem, "Frieren - T05");
    assert_eq!(new_volume, Some(5));
}

#[test]
fn post_rename_extracts_volume_from_tome_pattern() {
    let new_path = std::path::Path::new("/libraries/BD/Series/Series - Tome 12.cbz");
    let new_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap();
    let new_volume = parsers::extract_volume(new_stem);
    assert_eq!(new_volume, Some(12));
}

#[test]
fn post_rename_no_volume_in_special_edition() {
    let new_path = std::path::Path::new("/libraries/BD/Series/Series - Special.cbz");
    let new_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap();
    let new_volume = parsers::extract_volume(new_stem);
    assert_eq!(new_volume, None);
}

#[test]
fn post_rename_padded_volume() {
    let new_path = std::path::Path::new("/libraries/BD/One Piece/One Piece - T001.cbz");
    let new_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap();
    let new_volume = parsers::extract_volume(new_stem);
    assert_eq!(new_volume, Some(1));
}
