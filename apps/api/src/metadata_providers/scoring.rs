//! The single match-scoring policy for metadata candidates.
//!
//! Every provider scores a candidate against the searched name with
//! [`title_similarity`], wraps it in a [`Score`] that carries its own ceiling,
//! and the batch matcher asks [`Score::is_auto_match`] for the verdict. The
//! local book-count bonus is added to the similarity *before* the ceiling, so a
//! capped score can never be promoted into an automatic match.

use serde::{Deserialize, Serialize};

use super::SeriesCandidate;

/// Score a candidate must reach to be approved without manual review.
pub(crate) const AUTO_MATCH_THRESHOLD: f32 = 1.0;

/// Ceiling of a score that may reach the auto-match threshold.
pub(crate) const UNCAPPED: f32 = 1.0;

/// Tolerance on the threshold: `0.7 + 0.3` must count as `1.0` in `f32`.
const THRESHOLD_TOLERANCE: f32 = 1e-4;

/// Similarity when one normalized name starts with the other.
const PREFIX_SIMILARITY: f32 = 0.8;
/// Similarity when one normalized name contains the other.
const CONTAINS_SIMILARITY: f32 = 0.7;
/// Bounds of the word-overlap similarity of unrelated names. The upper bound
/// keeps them below the auto-match threshold even with the full book-count bonus.
const MIN_OVERLAP_SIMILARITY: f32 = 0.1;
const MAX_OVERLAP_SIMILARITY: f32 = 0.6;

/// Bonus when the local book count equals the provider's volume total.
const EXACT_BOOK_COUNT_BONUS: f32 = 0.3;
/// Bonus when the local book count is within [`CLOSE_BOOK_COUNT_DELTA`].
const CLOSE_BOOK_COUNT_BONUS: f32 = 0.15;
const CLOSE_BOOK_COUNT_DELTA: i64 = 2;

/// Similarity between a candidate `title` and the searched `query`, in `0.0..=1.0`.
///
/// Both sides are normalized (case, accents, punctuation). Equal names score
/// `1.0`, a prefix in either direction `0.8`, a substring in either direction
/// `0.7`, anything else the word overlap clamped to `0.1..=0.6`.
pub(crate) fn title_similarity(title: &str, query: &str) -> f32 {
    let title = parsers::normalize_title(title);
    let query = parsers::normalize_title(query);
    if title.is_empty() || query.is_empty() {
        return 0.0;
    }
    if title == query {
        return 1.0;
    }
    if title.starts_with(&query) || query.starts_with(&title) {
        return PREFIX_SIMILARITY;
    }
    if title.contains(&query) || query.contains(&title) {
        return CONTAINS_SIMILARITY;
    }
    let title_words: std::collections::HashSet<&str> = title.split_whitespace().collect();
    let query_words: std::collections::HashSet<&str> = query.split_whitespace().collect();
    let shared = title_words.intersection(&query_words).count() as f32;
    let union = title_words.union(&query_words).count().max(1) as f32;
    (shared / union).clamp(MIN_OVERLAP_SIMILARITY, MAX_OVERLAP_SIMILARITY)
}

/// Bonus for a candidate whose volume total matches the local book count.
pub(crate) fn book_count_bonus(local_count: i64, provider_total: Option<i32>) -> f32 {
    let Some(total) = provider_total.filter(|total| *total > 0) else {
        return 0.0;
    };
    if local_count <= 0 {
        return 0.0;
    }
    let delta = (local_count - i64::from(total)).abs();
    if delta == 0 {
        EXACT_BOOK_COUNT_BONUS
    } else if delta <= CLOSE_BOOK_COUNT_DELTA {
        CLOSE_BOOK_COUNT_BONUS
    } else {
        0.0
    }
}

/// A candidate's match score: a similarity, the bonuses earned on top of it and
/// the ceiling the total may never exceed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Score {
    similarity: f32,
    bonus: f32,
    cap: f32,
}

impl Score {
    /// An uncapped score from a raw similarity.
    pub(crate) fn new(similarity: f32) -> Self {
        Self::capped(similarity, UNCAPPED)
    }

    /// A score that may never exceed `cap`, whatever bonus it later earns.
    pub(crate) fn capped(similarity: f32, cap: f32) -> Self {
        Self {
            similarity,
            bonus: 0.0,
            cap,
        }
    }

    /// A candidate fetched by its own id: the identity is certain.
    pub(crate) fn exact() -> Self {
        Self::new(1.0)
    }

    /// Score of `title` against the searched `query`, capped at `cap`.
    pub(crate) fn of_titles(title: &str, query: &str, cap: f32) -> Self {
        Self::capped(title_similarity(title, query), cap)
    }

    /// Relevance of the `index`-th item of a ranked listing that has no query
    /// (trending, top lists): `1.0` for the first, `step` less for each next
    /// one, never below `0.1`.
    pub(crate) fn ranked(index: usize, step: f32) -> Self {
        Self::new(1.0 - (index as f32 * step).clamp(0.0, 0.9))
    }

    /// Add a bonus. It is summed with the similarity before the cap applies.
    pub(crate) fn with_bonus(mut self, bonus: f32) -> Self {
        self.bonus += bonus;
        self
    }

    /// Add the bonus earned by matching the local book count.
    pub(crate) fn with_book_count(self, local_count: i64, provider_total: Option<i32>) -> Self {
        self.with_bonus(book_count_bonus(local_count, provider_total))
    }

    /// The score exposed to users and stored on links, in `0.0..=cap`.
    pub(crate) fn value(&self) -> f32 {
        (self.similarity + self.bonus).clamp(0.0, 1.0).min(self.cap)
    }

    /// The prudence rule: only a score at the threshold is auto-approved.
    pub(crate) fn is_auto_match(&self) -> bool {
        self.value() >= AUTO_MATCH_THRESHOLD - THRESHOLD_TOLERANCE
    }
}

/// Sort candidates best score first; equal scores keep their order.
pub(crate) fn sort_best_first(candidates: &mut [SeriesCandidate]) {
    candidates.sort_by(|a, b| b.score.value().total_cmp(&a.score.value()));
}

/// Add the book-count bonus to every candidate, then re-sort them.
pub(crate) fn apply_book_count(candidates: &mut [SeriesCandidate], local_count: i64) {
    for candidate in candidates.iter_mut() {
        candidate.score = candidate
            .score
            .with_book_count(local_count, candidate.total_volumes);
    }
    sort_best_first(candidates);
}

#[cfg(test)]
mod tests {
    use super::*;

    const DERIVED_CAP: f32 = 0.7;

    #[test]
    fn equal_names_are_a_full_match_whatever_case_accents_or_punctuation() {
        assert_eq!(title_similarity("Naruto", "naruto"), 1.0);
        assert_eq!(title_similarity("Astérix", "Asterix"), 1.0);
        assert_eq!(title_similarity("Spider-Man", "spider man"), 1.0);
    }

    #[test]
    fn prefix_and_substring_rank_below_a_full_match() {
        assert_eq!(
            title_similarity("Naruto Shippuden", "naruto"),
            PREFIX_SIMILARITY
        );
        assert_eq!(
            title_similarity("Naruto", "Naruto Shippuden"),
            PREFIX_SIMILARITY
        );
        assert_eq!(
            title_similarity("The Art of Naruto", "naruto"),
            CONTAINS_SIMILARITY
        );
    }

    #[test]
    fn unrelated_names_stay_in_the_overlap_band() {
        assert_eq!(title_similarity("Bilbo", "Spawn"), MIN_OVERLAP_SIMILARITY);
        let partial = title_similarity("Les Cités obscures", "Cités perdues");
        assert!((MIN_OVERLAP_SIMILARITY..=MAX_OVERLAP_SIMILARITY).contains(&partial));
        assert_eq!(title_similarity("", "Spawn"), 0.0);
    }

    #[test]
    fn book_count_bonus_rewards_exact_then_close_totals() {
        assert_eq!(book_count_bonus(72, Some(72)), EXACT_BOOK_COUNT_BONUS);
        assert_eq!(book_count_bonus(10, Some(12)), CLOSE_BOOK_COUNT_BONUS);
        assert_eq!(book_count_bonus(10, Some(13)), 0.0);
        assert_eq!(book_count_bonus(0, Some(10)), 0.0);
        assert_eq!(book_count_bonus(10, None), 0.0);
        assert_eq!(book_count_bonus(10, Some(0)), 0.0);
    }

    #[test]
    fn only_a_full_score_auto_matches() {
        assert!(Score::exact().is_auto_match());
        assert!(Score::of_titles("Blacksad", "blacksad", UNCAPPED).is_auto_match());
        assert!(!Score::new(0.95).is_auto_match());
    }

    #[test]
    fn book_count_bonus_can_promote_an_uncapped_partial_match() {
        let score = Score::new(CONTAINS_SIMILARITY).with_book_count(10, Some(10));
        assert!(score.is_auto_match());
        assert_eq!(score.value(), 1.0);
    }

    #[test]
    fn capped_variant_with_the_right_book_count_never_auto_matches() {
        // A hit found through a derived query (subtitle dropped) is capped:
        // even an exact title and an exact volume total must stay for review.
        let score = Score::of_titles("Spawn", "Spawn", DERIVED_CAP).with_book_count(12, Some(12));
        assert_eq!(score.value(), DERIVED_CAP);
        assert!(!score.is_auto_match());

        let score = Score::capped(CONTAINS_SIMILARITY, DERIVED_CAP)
            .with_bonus(EXACT_BOOK_COUNT_BONUS)
            .with_bonus(CLOSE_BOOK_COUNT_BONUS);
        assert!(!score.is_auto_match());
    }

    #[test]
    fn unrelated_names_never_auto_match_even_with_the_bonus() {
        let score = Score::new(MAX_OVERLAP_SIMILARITY).with_book_count(5, Some(5));
        assert!(!score.is_auto_match());
    }

    #[test]
    fn value_is_bounded_by_one_and_by_the_cap() {
        assert_eq!(Score::new(0.9).with_bonus(0.3).value(), 1.0);
        assert_eq!(Score::capped(0.5, DERIVED_CAP).value(), 0.5);
        assert_eq!(Score::capped(1.0, DERIVED_CAP).value(), DERIVED_CAP);
    }

    fn candidate(title: &str, total_volumes: Option<i32>, score: Score) -> SeriesCandidate {
        SeriesCandidate {
            external_id: format!("test:{title}"),
            title: title.to_string(),
            authors: vec![],
            description: None,
            publishers: vec![],
            start_year: None,
            total_volumes,
            cover_url: None,
            external_url: None,
            score,
            metadata_json: serde_json::json!({}),
        }
    }

    #[test]
    fn apply_book_count_boosts_matching_totals_and_reorders() {
        let mut candidates = vec![
            candidate("Edition A", Some(35), Score::new(0.6)),
            candidate("Edition B", Some(72), Score::new(0.5)),
            candidate("Edition C", Some(70), Score::new(0.6)),
            candidate("Edition D", None, Score::new(0.55)),
        ];
        apply_book_count(&mut candidates, 72);
        let expected = [
            ("Edition B", 0.8),
            ("Edition C", 0.75),
            ("Edition A", 0.6),
            ("Edition D", 0.55),
        ];
        assert_eq!(candidates.len(), expected.len());
        for (candidate, (title, value)) in candidates.iter().zip(expected) {
            assert_eq!(candidate.title, title);
            assert!((candidate.score.value() - value).abs() < 1e-6);
        }
    }

    #[test]
    fn apply_book_count_without_local_books_changes_nothing() {
        let mut candidates = vec![candidate("Series", Some(10), Score::new(0.7))];
        apply_book_count(&mut candidates, 0);
        assert_eq!(candidates[0].score.value(), 0.7);
    }

    #[test]
    fn apply_book_count_keeps_a_capped_candidate_below_the_threshold() {
        let mut candidates = vec![candidate(
            "Spawn",
            Some(12),
            Score::of_titles("Spawn", "Spawn", DERIVED_CAP),
        )];
        apply_book_count(&mut candidates, 12);
        assert_eq!(candidates[0].score.value(), DERIVED_CAP);
        assert!(!candidates[0].score.is_auto_match());
    }

    #[test]
    fn ranked_listing_decreases_with_the_position() {
        assert_eq!(Score::ranked(0, 0.05).value(), 1.0);
        assert!((Score::ranked(2, 0.05).value() - 0.9).abs() < 1e-6);
        assert!((Score::ranked(50, 0.05).value() - 0.1).abs() < 1e-6);
    }
}
