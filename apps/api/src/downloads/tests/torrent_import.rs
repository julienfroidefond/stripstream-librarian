use super::*;
use std::collections::HashSet;

// ─── deduplicate_by_format ──────────────────────────────────────────────

#[test]
fn dedup_filters_by_expected_set() {
    let files = vec![
        "/dl/Series - T01.cbz".to_string(),
        "/dl/Series - T02.cbz".to_string(),
        "/dl/Series - T03.cbz".to_string(),
    ];
    let expected: HashSet<i32> = [2].into_iter().collect();
    let result = deduplicate_by_format(&files, &expected);
    assert_eq!(result.len(), 1);
    assert!(result[0].contains("T02"));
}

#[test]
fn dedup_empty_expected_keeps_all() {
    let files = vec![
        "/dl/Series - T01.cbz".to_string(),
        "/dl/Series - T02.cbz".to_string(),
        "/dl/Series - T03.cbz".to_string(),
    ];
    let expected: HashSet<i32> = HashSet::new();
    let result = deduplicate_by_format(&files, &expected);
    assert_eq!(result.len(), 3, "empty expected_set should keep all files");
}

#[test]
fn dedup_empty_set_for_replace_mode_keeps_all_volumes() {
    // Simulates replace mode: expected_set is empty, all 8 volumes should be kept
    let files: Vec<String> = (1..=8)
        .map(|i| format!("/dl/La Quete - T{:02}.cbz", i))
        .collect();
    let expected: HashSet<i32> = HashSet::new(); // replace mode passes empty set
    let result = deduplicate_by_format(&files, &expected);
    assert_eq!(
        result.len(),
        8,
        "replace mode should keep all 8 volumes, got {}",
        result.len()
    );
}

#[test]
fn dedup_prefers_cbz_over_cbr() {
    let files = vec![
        "/dl/Series - T01.cbr".to_string(),
        "/dl/Series - T01.cbz".to_string(),
    ];
    let expected: HashSet<i32> = HashSet::new();
    let result = deduplicate_by_format(&files, &expected);
    assert_eq!(result.len(), 1);
    assert!(result[0].contains(".cbz"), "should prefer cbz over cbr");
}

#[test]
fn dedup_partial_expected_only_keeps_matching() {
    let files: Vec<String> = (1..=8)
        .map(|i| format!("/dl/Series - T{:02}.cbz", i))
        .collect();
    let expected: HashSet<i32> = [3, 5, 8].into_iter().collect();
    let result = deduplicate_by_format(&files, &expected);
    assert_eq!(result.len(), 3, "should keep only volumes 3, 5, 8");
}

// ─── build_target_filename ──────────────────────────────────────────────

#[test]
fn simple_t_prefix() {
    // "One Piece - T104.cbz" → replace 104 → 105
    let result =
        build_target_filename("/libraries/One Piece/One Piece - T104.cbz", 104, 105, "cbz");
    assert_eq!(result, Some("One Piece - T105.cbz".to_string()));
}

#[test]
fn preserves_leading_zeros() {
    let result = build_target_filename("/libraries/Asterix/Asterix - T01.cbz", 1, 2, "cbz");
    assert_eq!(result, Some("Asterix - T02.cbz".to_string()));
}

#[test]
fn three_digit_zero_padded() {
    let result = build_target_filename("/libraries/Naruto/Naruto T001.cbz", 1, 72, "cbz");
    assert_eq!(result, Some("Naruto T072.cbz".to_string()));
}

#[test]
fn different_source_ext() {
    // Source file is cbr, reference is cbz
    let result = build_target_filename("/libraries/DBZ/Dragon Ball - T01.cbz", 1, 5, "cbr");
    assert_eq!(result, Some("Dragon Ball - T05.cbr".to_string()));
}

#[test]
fn accented_series_name() {
    let result = build_target_filename("/libraries/bd/Astérix - T01.cbz", 1, 3, "cbz");
    assert_eq!(result, Some("Astérix - T03.cbz".to_string()));
}

#[test]
fn build_target_from_tome_reference() {
    // Reference uses "Tome" pattern
    let result = build_target_filename(
        "/libraries/bd/Kaiju no8/Kaiju no8 - Tome 05.cbz",
        5,
        9,
        "cbz",
    );
    assert_eq!(result, Some("Kaiju no8 - Tome 09.cbz".to_string()));
}

#[test]
fn build_target_from_tome_with_subtitle_reference() {
    // Reference with "Tome XX - Subtitle"
    let result = build_target_filename(
        "/libraries/bd/Series/Tome 19 - Pas de Nol.pdf",
        19,
        20,
        "pdf",
    );
    // Should produce "Tome 20" (truncates subtitle after volume)
    assert_eq!(result, Some("Tome 20.pdf".to_string()));
}

#[test]
fn no_match_returns_none() {
    // Volume 5 not present in "Series - T01.cbz" whose reference_volume is 99
    let result = build_target_filename("/libraries/Series/Series - T01.cbz", 99, 100, "cbz");
    assert_eq!(result, None);
}

#[test]
fn uses_last_occurrence() {
    // "Code 451 - T04.cbz" with reference_volume=4 should replace the "04" not the "4" in 451
    let result = build_target_filename("/libraries/Code 451/Code 451 - T04.cbz", 4, 5, "cbz");
    assert_eq!(result, Some("Code 451 - T05.cbz".to_string()));
}

#[test]
fn truncates_suffix_after_volume() {
    let result = build_target_filename(
        "/libraries/manga/Goblin slayer/Goblin.Slayer.Tome.007.FR-NoFace696.cbr",
        7,
        8,
        "cbz",
    );
    assert_eq!(result, Some("Goblin.Slayer.Tome.008.cbz".to_string()));
}

#[test]
fn template_filename_without_reference_uses_regular_rename_format() {
    let templates = crate::books::rename::RenameTemplates {
        regular: "{series_name} - T{volume_padded} - {title}".to_string(),
        hs: "{series_name} - HS {volume_padded}".to_string(),
        integral: "{series_name} - INT {volume_padded}".to_string(),
    };

    let result = build_target_filename_from_template(
        &templates,
        "Frieren",
        "/downloads/sl-1/Frieren Tome 3 - Le voyage.cbz",
        3,
        "cbz",
        12,
    );

    assert_eq!(
        result,
        Some("Frieren - T03 - Frieren Tome 3 - Le voyage.cbz".to_string())
    );
}

#[test]
fn template_filename_without_reference_does_not_add_title_when_template_omits_it() {
    let templates = crate::books::rename::RenameTemplates {
        regular: "{series_name} - T{volume_padded}".to_string(),
        hs: "{series_name} - HS {volume_padded}".to_string(),
        integral: "{series_name} - INT {volume_padded}".to_string(),
    };

    let result = build_target_filename_from_template(
        &templates,
        "Frieren",
        "/downloads/sl-1/Frieren Tome 3 - Le voyage.cbz",
        3,
        "cbz",
        12,
    );

    assert_eq!(result, Some("Frieren - T03.cbz".to_string()));
}

#[test]
fn template_filename_without_reference_uses_hs_rename_format() {
    let templates = crate::books::rename::RenameTemplates {
        regular: "{series_name} - T{volume_padded} - {title}".to_string(),
        hs: "{series_name} - HS {volume_padded}".to_string(),
        integral: "{series_name} - INT {volume_padded}".to_string(),
    };

    let result = build_target_filename_from_template(
        &templates,
        "Frieren",
        "/downloads/sl-1/Frieren HS 2.cbz",
        2,
        "cbz",
        12,
    );

    assert_eq!(result, Some("Frieren - HS 02.cbz".to_string()));
}

// default_filename tests removed — function replaced by keeping original filename

// ─── format_priority ─────────────────────────────────────────────────

#[test]
fn format_priority_cbz_is_best() {
    assert_eq!(format_priority("cbz"), 0);
}

#[test]
fn format_priority_ordering() {
    assert!(format_priority("cbz") < format_priority("cbr"));
    assert!(format_priority("cbr") < format_priority("pdf"));
    assert!(format_priority("pdf") < format_priority("epub"));
    assert!(format_priority("epub") < format_priority("unknown"));
}

#[test]
fn format_priority_case_insensitive() {
    assert_eq!(format_priority("CBZ"), 0);
    assert_eq!(format_priority("Cbr"), 1);
    assert_eq!(format_priority("PDF"), 2);
    assert_eq!(format_priority("EPUB"), 3);
}

#[test]
fn format_priority_unknown_extension() {
    assert_eq!(format_priority("txt"), 4);
    assert_eq!(format_priority(""), 4);
    assert_eq!(format_priority("doc"), 4);
}

// ─── strip_accents ───────────────────────────────────────────────────

#[test]
fn strip_accents_french() {
    assert_eq!(strip_accents("les géants"), "les geants");
    assert_eq!(strip_accents("astérix"), "asterix");
    assert_eq!(strip_accents("à la maison"), "a la maison");
}

#[test]
fn strip_accents_special() {
    assert_eq!(strip_accents("naïve"), "naive");
    assert_eq!(strip_accents("über"), "uber");
    assert_eq!(strip_accents("señor"), "senor");
    assert_eq!(strip_accents("cœur"), "coeur");
    assert_eq!(strip_accents("æther"), "aether");
}

#[test]
fn strip_accents_no_accents() {
    assert_eq!(strip_accents("hello world"), "hello world");
    assert_eq!(strip_accents(""), "");
}

// ─── collect_book_files (uses temp dirs) ─────────────────────────────

#[test]
fn collect_book_files_finds_supported_formats() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("vol1.cbz"), b"fake").unwrap();
    std::fs::write(dir.path().join("vol2.cbr"), b"fake").unwrap();
    std::fs::write(dir.path().join("vol3.pdf"), b"fake").unwrap();
    std::fs::write(dir.path().join("vol4.epub"), b"fake").unwrap();
    std::fs::write(dir.path().join("readme.txt"), b"not a book").unwrap();

    let files = collect_book_files(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(files.len(), 4, "should find 4 book files, got {:?}", files);
}

#[test]
fn collect_book_files_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("subdir");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(dir.path().join("vol1.cbz"), b"fake").unwrap();
    std::fs::write(sub.join("vol2.cbz"), b"fake").unwrap();

    let files = collect_book_files(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn collect_book_files_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let files = collect_book_files(dir.path().to_str().unwrap()).unwrap();
    assert!(files.is_empty());
}

#[test]
fn collect_book_files_single_file() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("single.cbz");
    std::fs::write(&file_path, b"fake").unwrap();

    let files = collect_book_files(file_path.to_str().unwrap()).unwrap();
    assert_eq!(files.len(), 1);
}

// ─── find_existing_series_dir (uses temp dirs) ───────────────────────

#[test]
fn find_existing_series_dir_exact_match() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("One Piece")).unwrap();

    let result = find_existing_series_dir(dir.path().to_str().unwrap(), "One Piece");
    assert!(result.is_some());
    assert!(result.unwrap().contains("One Piece"));
}

#[test]
fn find_existing_series_dir_case_insensitive() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("one piece")).unwrap();

    let result = find_existing_series_dir(dir.path().to_str().unwrap(), "One Piece");
    assert!(result.is_some());
}

#[test]
fn find_existing_series_dir_accent_insensitive() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("les geants")).unwrap();

    let result = find_existing_series_dir(dir.path().to_str().unwrap(), "les géants");
    assert!(result.is_some());
}

#[test]
fn find_existing_series_dir_no_match() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Naruto")).unwrap();

    let result = find_existing_series_dir(dir.path().to_str().unwrap(), "One Piece");
    assert!(result.is_none());
}

#[test]
fn find_existing_series_dir_prefers_exact_case() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Les Geants")).unwrap();
    std::fs::create_dir(dir.path().join("les géants")).unwrap();

    // When searching for "les géants", should prefer the exact case match
    let result = find_existing_series_dir(dir.path().to_str().unwrap(), "les géants");
    assert!(result.is_some());
    assert!(result.unwrap().contains("les géants"));
}

// ─── find_reference_from_disk (uses temp dirs) ───────────────────────

#[test]
fn find_reference_from_disk_picks_highest_volume() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Series T01.cbz"), b"fake").unwrap();
    std::fs::write(dir.path().join("Series T05.cbz"), b"fake").unwrap();
    std::fs::write(dir.path().join("Series T03.cbz"), b"fake").unwrap();

    let exclude: HashSet<i32> = HashSet::new();
    let result = find_reference_from_disk(dir.path().to_str().unwrap(), &exclude);
    assert!(result.is_some());
    let (_, vol) = result.unwrap();
    assert_eq!(vol, 5);
}

#[test]
fn find_reference_from_disk_excludes_volumes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Series T05.cbz"), b"fake").unwrap();
    std::fs::write(dir.path().join("Series T03.cbz"), b"fake").unwrap();

    let exclude: HashSet<i32> = [5].into_iter().collect();
    let result = find_reference_from_disk(dir.path().to_str().unwrap(), &exclude);
    assert!(result.is_some());
    let (_, vol) = result.unwrap();
    assert_eq!(vol, 3);
}

#[test]
fn find_reference_from_disk_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let exclude: HashSet<i32> = HashSet::new();
    let result = find_reference_from_disk(dir.path().to_str().unwrap(), &exclude);
    assert!(result.is_none());
}

#[test]
fn find_reference_from_disk_ignores_non_book_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("readme.txt"), b"not a book").unwrap();
    std::fs::write(dir.path().join("cover.jpg"), b"not a book").unwrap();

    let exclude: HashSet<i32> = HashSet::new();
    let result = find_reference_from_disk(dir.path().to_str().unwrap(), &exclude);
    assert!(result.is_none());
}

// ─── DB integration tests (sqlx::test) ──────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_matching_unaccent_query(pool: sqlx::PgPool) {
    // Setup: create a library and a series with accented name "Astérix"
    let library_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Test Lib', '/libraries/test')",
    )
    .bind(library_id)
    .execute(&pool)
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Astérix')")
        .bind(series_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert a book linked to this series with a volume
    let book_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, kind, title, volume, series_id) \
         VALUES ($1, $2, 'bd', 'Astérix le Gaulois', 1, $3)",
    )
    .bind(book_id)
    .bind(library_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    // Insert a book_file for this book
    let bf_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint) \
         VALUES ($1, $2, 'cbz', '/libraries/test/Astérix/Astérix - T01.cbz', 1024, NOW(), 'fp1')",
    )
    .bind(bf_id)
    .bind(book_id)
    .execute(&pool)
    .await
    .unwrap();

    // Execute: run the same unaccent query used in do_import with "Asterix" (no accent)
    let row = sqlx::query(
        "SELECT bf.abs_path, b.volume \
         FROM book_files bf \
         JOIN books b ON b.id = bf.book_id \
         LEFT JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 \
           AND LOWER(unaccent(s.name)) = LOWER(unaccent($2)) \
           AND b.volume IS NOT NULL \
         ORDER BY b.volume DESC LIMIT 1",
    )
    .bind(library_id)
    .bind("Asterix") // no accent — should match "Astérix" via unaccent
    .fetch_optional(&pool)
    .await
    .unwrap();

    // Assert: it finds the series
    assert!(
        row.is_some(),
        "unaccent query should match 'Astérix' when searching 'Asterix'"
    );
    let row = row.unwrap();
    let abs_path: String = row.get("abs_path");
    let volume: i32 = row.get("volume");
    assert_eq!(abs_path, "/libraries/test/Astérix/Astérix - T01.cbz");
    assert_eq!(volume, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn db_reference_query_returns_correct_data(pool: sqlx::PgPool) {
    // Setup: library + series + book + book_file
    let library_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'BD', '/libraries/bd')")
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'One Piece')")
        .bind(series_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert two books with different volumes
    let book1_id = Uuid::new_v4();
    let book2_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, kind, title, volume, series_id) VALUES \
         ($1, $2, 'bd', 'One Piece T01', 1, $3), \
         ($4, $2, 'bd', 'One Piece T104', 104, $3)",
    )
    .bind(book1_id)
    .bind(library_id)
    .bind(series_id)
    .bind(book2_id)
    .execute(&pool)
    .await
    .unwrap();

    // Insert book_files
    sqlx::query(
        "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint) VALUES \
         ($1, $2, 'cbz', '/libraries/bd/One Piece/One Piece - T01.cbz', 5000, NOW(), 'fp_a'), \
         ($3, $4, 'cbz', '/libraries/bd/One Piece/One Piece - T104.cbz', 8000, NOW(), 'fp_b')",
    )
    .bind(Uuid::new_v4())
    .bind(book1_id)
    .bind(Uuid::new_v4())
    .bind(book2_id)
    .execute(&pool)
    .await
    .unwrap();

    // Execute: same query as do_import — should return highest volume (104)
    let row = sqlx::query(
        "SELECT bf.abs_path, b.volume \
         FROM book_files bf \
         JOIN books b ON b.id = bf.book_id \
         LEFT JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 \
           AND LOWER(unaccent(s.name)) = LOWER(unaccent($2)) \
           AND b.volume IS NOT NULL \
         ORDER BY b.volume DESC LIMIT 1",
    )
    .bind(library_id)
    .bind("One Piece")
    .fetch_optional(&pool)
    .await
    .unwrap();

    // Assert
    assert!(row.is_some());
    let row = row.unwrap();
    let abs_path: String = row.get("abs_path");
    let volume: i32 = row.get("volume");
    assert_eq!(volume, 104, "should return highest volume");
    assert_eq!(abs_path, "/libraries/bd/One Piece/One Piece - T104.cbz");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn expand_expected_volumes_with_missing(pool: sqlx::PgPool) {
    // Setup: library + series with volumes 1, 2, 5 (missing 3, 4, 6, 7, 8)
    let lib_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'test', '/libraries/test')",
    )
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Tom-Tom et Nana')")
        .bind(series_id)
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert existing books: volumes 1, 2, 5
    for vol in [1, 2, 5] {
        let book_id = Uuid::new_v4();
        sqlx::query("INSERT INTO books (id, library_id, series_id, title, kind, volume) VALUES ($1, $2, $3, $4, 'comic', $5)")
            .bind(book_id).bind(lib_id).bind(series_id)
            .bind(format!("Tom-Tom et Nana - T{:02}", vol)).bind(vol)
            .execute(&pool).await.unwrap();
    }

    // Simulate: torrent has volumes 1-8, expected_volumes from detection was only {7}
    let mut expected_set: std::collections::HashSet<i32> = [7].into_iter().collect();
    let torrent_volumes: std::collections::HashSet<i32> = (1..=8).collect();

    // Query existing volumes (same logic as in do_import)
    let existing_volumes: Vec<i32> = sqlx::query_scalar(
        "SELECT DISTINCT b.volume FROM books b \
         LEFT JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 AND LOWER(unaccent(s.name)) = LOWER(unaccent($2)) AND b.volume IS NOT NULL"
    )
    .bind(lib_id)
    .bind("Tom-Tom et Nana")
    .fetch_all(&pool)
    .await
    .unwrap();

    let existing_set: std::collections::HashSet<i32> = existing_volumes.into_iter().collect();
    assert_eq!(
        existing_set,
        [1, 2, 5]
            .into_iter()
            .collect::<std::collections::HashSet<i32>>()
    );

    // Expand expected_set with missing volumes from library
    let missing_in_library: Vec<i32> = torrent_volumes
        .iter()
        .filter(|v| !existing_set.contains(v))
        .copied()
        .collect();
    expected_set.extend(missing_in_library);

    // Should now contain 3, 4, 6, 7, 8 (all missing from library that torrent has)
    assert!(
        expected_set.contains(&3),
        "volume 3 missing from library, should be in expected_set"
    );
    assert!(
        expected_set.contains(&4),
        "volume 4 missing from library, should be in expected_set"
    );
    assert!(
        expected_set.contains(&6),
        "volume 6 missing from library, should be in expected_set"
    );
    assert!(
        expected_set.contains(&7),
        "volume 7 was originally expected"
    );
    assert!(
        expected_set.contains(&8),
        "volume 8 missing from library, should be in expected_set"
    );
    // Should NOT contain existing volumes
    assert!(
        !expected_set.contains(&1),
        "volume 1 exists in library, should NOT be imported"
    );
    assert!(
        !expected_set.contains(&2),
        "volume 2 exists in library, should NOT be imported"
    );
    assert!(
        !expected_set.contains(&5),
        "volume 5 exists in library, should NOT be imported"
    );
}
