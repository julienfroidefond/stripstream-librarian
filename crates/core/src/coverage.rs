//! Canonical volume coverage for a series.
//!
//! Numbered `regular` and `integral` books cover their volume. An unnumbered
//! `regular` or `integral` book covers one unspecified volume; an unnumbered
//! `integral` covers the whole series. `hs` and `oneshot` never participate, and
//! volume `0` is a provider placeholder for specials, not a tome.
//!
//! Every consumer that needs to know whether a volume is owned, how many
//! volumes are missing, or whether a series is complete must go through this
//! module. Keeping the rule in one place avoids the series list, the metadata
//! gaps, the download availability and the indexer cleanup drifting apart.

use std::collections::BTreeSet;

/// Volume types that participate in tome numbering.
pub const COUNTED_VOLUME_TYPES: [&str; 2] = ["regular", "integral"];

/// First volume number that represents a real tome. Volume `0` (and negative
/// values, which should not exist) are provider placeholders, not tomes.
pub const FIRST_TOME: i32 = 1;

/// Whether a `books.volume_type` value participates in tome numbering.
pub fn is_counted_volume_type(volume_type: &str) -> bool {
    COUNTED_VOLUME_TYPES.contains(&volume_type)
}

/// Whether a volume number represents a real tome.
pub fn is_numbered_volume(volume: i32) -> bool {
    volume >= FIRST_TOME
}

/// The coverage facts of one series.
///
/// Build it with [`SeriesCoverage::new`] / [`SeriesCoverage::add`] when reading
/// books one by one, or with [`SeriesCoverage::from_parts`] when the database
/// already aggregated them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeriesCoverage {
    total_volumes: Option<i32>,
    owned_volumes: BTreeSet<i32>,
    unnumbered_books: i32,
    has_unnumbered_integral: bool,
}

impl SeriesCoverage {
    /// Start with the series' declared total (`series.total_volumes`).
    pub fn new(total_volumes: Option<i32>) -> Self {
        Self {
            total_volumes,
            ..Self::default()
        }
    }

    /// Build from facts already aggregated by a database query.
    ///
    /// Numbered volumes are deduplicated; non-tome numbers are ignored.
    /// `unnumbered_books` must include every counted book with a NULL volume,
    /// including the integral that sets `has_unnumbered_integral`.
    pub fn from_parts(
        total_volumes: Option<i32>,
        owned_volumes: impl IntoIterator<Item = i32>,
        unnumbered_books: i32,
        has_unnumbered_integral: bool,
    ) -> Self {
        Self {
            total_volumes,
            owned_volumes: owned_volumes
                .into_iter()
                .filter(|volume| is_numbered_volume(*volume))
                .collect(),
            unnumbered_books: unnumbered_books.max(0),
            has_unnumbered_integral,
        }
    }

    /// Record one local book's contribution.
    pub fn add(&mut self, volume_type: &str, volume: Option<i32>) {
        if !is_counted_volume_type(volume_type) {
            return;
        }
        match volume {
            Some(volume) if is_numbered_volume(volume) => {
                self.owned_volumes.insert(volume);
            }
            Some(_) => {}
            None => {
                self.unnumbered_books += 1;
                if volume_type == "integral" {
                    self.has_unnumbered_integral = true;
                }
            }
        }
    }

    /// The declared total, if the series has one.
    pub fn total_volumes(&self) -> Option<i32> {
        self.total_volumes
    }

    /// Distinct numbered volumes covered by `regular` or `integral` books.
    pub fn owned_volumes(&self) -> Vec<i32> {
        self.owned_volumes.iter().copied().collect()
    }

    /// Whether a specific numbered volume is covered.
    ///
    /// An unnumbered integral covers every volume. An unnumbered regular book
    /// covers one unspecified volume, so it cannot be attributed to a specific
    /// number and is only reflected in [`Self::missing_count`].
    pub fn covers_volume(&self, volume: i32) -> bool {
        self.has_unnumbered_integral || self.owned_volumes.contains(&volume)
    }

    /// Number of counted books: distinct numbered volumes plus unnumbered ones.
    pub fn covered_count(&self) -> i32 {
        self.owned_volumes.len() as i32 + self.unnumbered_books
    }

    /// True when an unnumbered integral makes the whole series complete.
    pub fn has_unnumbered_integral(&self) -> bool {
        self.has_unnumbered_integral
    }

    /// Missing tome count against the series' declared total.
    pub fn missing_count(&self) -> i32 {
        self.missing_count_for_total(self.total_volumes)
    }

    /// Missing tome count against an explicit total.
    ///
    /// Metadata gaps use the provider's volume count when the user has not set
    /// `series.total_volumes`, so they need to override the declared total.
    pub fn missing_count_for_total(&self, total_volumes: Option<i32>) -> i32 {
        if self.has_unnumbered_integral {
            return 0;
        }
        let total = total_volumes.unwrap_or(0).max(0);
        (total - self.covered_count()).max(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coverage(books: &[(&str, Option<i32>)], total: Option<i32>) -> SeriesCoverage {
        let mut coverage = SeriesCoverage::new(total);
        for (volume_type, volume) in books {
            coverage.add(volume_type, *volume);
        }
        coverage
    }

    #[test]
    fn unknown_total_is_zero_missing() {
        assert_eq!(coverage(&[], None).missing_count(), 0);
    }

    #[test]
    fn regular_books_cover_distinct_numbered_volumes() {
        let coverage = coverage(
            &[
                ("regular", Some(1)),
                ("regular", Some(2)),
                ("regular", Some(2)),
            ],
            Some(5),
        );
        assert_eq!(coverage.owned_volumes(), vec![1, 2]);
        assert_eq!(coverage.covered_count(), 2, "duplicates count once");
        assert_eq!(coverage.missing_count(), 3);
    }

    #[test]
    fn hs_and_oneshot_do_not_count() {
        let coverage = coverage(
            &[
                ("regular", Some(1)),
                ("hs", Some(1)),
                ("oneshot", Some(2)),
                ("oneshot", None),
            ],
            Some(3),
        );
        assert_eq!(coverage.owned_volumes(), vec![1]);
        assert_eq!(coverage.covered_count(), 1);
        assert_eq!(coverage.missing_count(), 2);
    }

    #[test]
    fn numbered_integral_covers_only_its_volume() {
        let coverage = coverage(
            &[
                ("regular", Some(1)),
                ("regular", Some(2)),
                ("integral", Some(2)),
            ],
            Some(10),
        );
        assert_eq!(coverage.owned_volumes(), vec![1, 2]);
        assert_eq!(coverage.missing_count(), 8);
    }

    #[test]
    fn unnumbered_integral_completes_the_series() {
        let coverage = coverage(&[("integral", None)], Some(10));
        assert!(coverage.has_unnumbered_integral());
        assert_eq!(coverage.missing_count(), 0);
        assert!(coverage.covers_volume(7));
    }

    #[test]
    fn unnumbered_regular_covers_one_unspecified_volume() {
        let coverage = coverage(&[("regular", None)], Some(5));
        assert_eq!(coverage.covered_count(), 1);
        assert_eq!(coverage.missing_count(), 4);
        assert!(!coverage.covers_volume(1));
    }

    #[test]
    fn volume_zero_is_not_a_tome() {
        let coverage = coverage(&[("regular", Some(0)), ("integral", Some(0))], Some(3));
        assert!(coverage.owned_volumes().is_empty());
        assert_eq!(coverage.covered_count(), 0);
        assert_eq!(coverage.missing_count(), 3);
    }

    #[test]
    fn covered_count_is_clamped() {
        let coverage = coverage(
            &[
                ("regular", Some(1)),
                ("regular", Some(2)),
                ("regular", Some(3)),
            ],
            Some(2),
        );
        assert_eq!(coverage.missing_count(), 0);
    }

    #[test]
    fn missing_count_can_override_the_declared_total() {
        let coverage = coverage(&[("regular", Some(1))], Some(2));
        assert_eq!(coverage.missing_count(), 1);
        assert_eq!(coverage.missing_count_for_total(Some(5)), 4);
    }

    #[test]
    fn from_parts_matches_added_books() {
        let mut added = SeriesCoverage::new(Some(4));
        added.add("regular", Some(1));
        added.add("integral", Some(1));
        added.add("regular", None);
        added.add("hs", Some(2));

        let parts = SeriesCoverage::from_parts(Some(4), [1, 1], 1, false);
        assert_eq!(parts, added);
    }
}
