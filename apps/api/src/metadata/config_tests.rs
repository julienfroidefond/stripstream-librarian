use super::*;

fn make_candidate(title: &str, total_volumes: Option<i32>, confidence: f32) -> metadata_providers::SeriesCandidate {
    metadata_providers::SeriesCandidate {
        external_id: format!("test:{title}"),
        title: title.to_string(),
        authors: vec![],
        description: None,
        publishers: vec![],
        start_year: None,
        total_volumes,
        cover_url: None,
        external_url: None,
        confidence,
        metadata_json: serde_json::json!({}),
    }
}

#[test]
fn boost_exact_match_increases_confidence() {
    let mut candidates = vec![
        make_candidate("Naruto", Some(72), 0.7),
        make_candidate("Naruto (Édition Hokage)", Some(35), 0.5),
    ];
    boost_confidence_by_book_count(&mut candidates, 72);
    assert!((candidates[0].confidence - 1.0).abs() < f32::EPSILON);
    assert!((candidates[1].confidence - 0.5).abs() < f32::EPSILON);
}

#[test]
fn boost_close_match_moderate_increase() {
    let mut candidates = vec![make_candidate("Series", Some(10), 0.6)];
    boost_confidence_by_book_count(&mut candidates, 11);
    assert!((candidates[0].confidence - 0.75).abs() < f32::EPSILON);
}

#[test]
fn boost_no_match_unchanged() {
    let mut candidates = vec![make_candidate("Series", Some(10), 0.6)];
    boost_confidence_by_book_count(&mut candidates, 50);
    assert!((candidates[0].confidence - 0.6).abs() < f32::EPSILON);
}

#[test]
fn boost_none_total_volumes_unchanged() {
    let mut candidates = vec![make_candidate("Series", None, 0.8)];
    boost_confidence_by_book_count(&mut candidates, 10);
    assert!((candidates[0].confidence - 0.8).abs() < f32::EPSILON);
}

#[test]
fn boost_reorders_by_confidence() {
    let mut candidates = vec![
        make_candidate("Edition A", Some(35), 0.6),
        make_candidate("Edition B", Some(72), 0.5),
    ];
    boost_confidence_by_book_count(&mut candidates, 72);
    assert_eq!(candidates[0].title, "Edition B");
}

#[test]
fn boost_capped_at_one() {
    let mut candidates = vec![make_candidate("Series", Some(10), 0.9)];
    boost_confidence_by_book_count(&mut candidates, 10);
    assert!((candidates[0].confidence - 1.0).abs() < f32::EPSILON);
}

#[test]
fn boost_zero_local_count_unchanged() {
    let mut candidates = vec![make_candidate("Series", Some(10), 0.7)];
    boost_confidence_by_book_count(&mut candidates, 0);
    assert!((candidates[0].confidence - 0.7).abs() < f32::EPSILON);
}
