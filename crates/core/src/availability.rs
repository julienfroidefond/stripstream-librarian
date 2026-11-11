//! Download-release availability for a series.
//!
//! `available_downloads` stores one row per series. Its `available_releases`
//! JSON array carries, for each release, the missing volumes it can fill in
//! `matched_missing_volumes`. Detection, RSS polling and the DDL search *merge*
//! releases in; scan cleanup, torrent import and detection normalization *prune*
//! volumes out. This module owns both directions and the counter derivation so
//! the API and the indexer cannot drift.
//!
//! Pruning derives `missing_count` from the volumes that actually disappeared:
//! a volume offered by three releases is still one missing tome, and a volume
//! that was never represented in a release cannot be inferred from the JSON.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::coverage::{is_numbered_volume, SeriesCoverage};

/// Release field holding the volumes the release can fill in.
pub const MATCHED_MISSING_VOLUMES: &str = "matched_missing_volumes";

/// Volumes known to be present locally (or just imported), plus whether an
/// unnumbered integral completes the whole series.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PresentVolumes {
    volumes: BTreeSet<i32>,
    complete: bool,
}

impl PresentVolumes {
    /// No present volume.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from an iterator of volume numbers. Non-tome numbers are ignored.
    pub fn from_volumes(volumes: impl IntoIterator<Item = i32>) -> Self {
        let mut present = Self::new();
        for volume in volumes {
            present.insert(volume);
        }
        present
    }

    /// An unnumbered integral covers every volume of the series.
    pub fn complete() -> Self {
        Self {
            volumes: BTreeSet::new(),
            complete: true,
        }
    }

    /// Record a present volume. Volume `0`, `hs` and one-shots never reach the
    /// coverage builder, but the guard keeps direct callers honest.
    pub fn insert(&mut self, volume: i32) {
        if is_numbered_volume(volume) {
            self.volumes.insert(volume);
        }
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn contains(&self, volume: i32) -> bool {
        self.complete || self.volumes.contains(&volume)
    }
}

impl From<&SeriesCoverage> for PresentVolumes {
    fn from(coverage: &SeriesCoverage) -> Self {
        Self {
            volumes: coverage.owned_volumes().into_iter().collect(),
            complete: coverage.has_unnumbered_integral(),
        }
    }
}

/// Result of removing volumes from a release list.
#[derive(Debug, Clone, PartialEq)]
pub struct PrunedReleases {
    pub releases: Vec<Value>,
    /// Distinct numbered volumes removed from at least one release.
    pub removed_volumes: BTreeSet<i32>,
    /// Whether the stored JSON must be rewritten (or the row deleted).
    pub changed: bool,
}

impl PrunedReleases {
    pub fn is_empty(&self) -> bool {
        self.releases.is_empty()
    }

    /// The series-level counter is stored separately from the release list.
    /// Subtract the distinct missing volumes the prune actually removed.
    pub fn missing_count_after(&self, previous: i32) -> i32 {
        let removed = self.removed_volumes.len().min(i32::MAX as usize) as i32;
        previous.saturating_sub(removed).max(0)
    }
}

/// Remove volumes from every release's `matched_missing_volumes`.
///
/// Rules:
/// - a numbered volume is removed when it is present (or the series is complete);
/// - non-tome numbers (`0`, negative) are always removed;
/// - a release left without matched volumes is dropped;
/// - a release without a `matched_missing_volumes` array is kept untouched.
pub fn prune_releases(releases: &[Value], present: &PresentVolumes) -> PrunedReleases {
    let mut kept_releases = Vec::with_capacity(releases.len());
    let mut removed_volumes = BTreeSet::new();
    let mut changed = false;

    for release in releases {
        let Some(matched) = release.get(MATCHED_MISSING_VOLUMES) else {
            kept_releases.push(release.clone());
            continue;
        };
        let Some(matched) = matched.as_array() else {
            kept_releases.push(release.clone());
            continue;
        };

        if matched.is_empty() {
            changed = true;
            continue;
        }

        let mut kept_volumes = Vec::with_capacity(matched.len());
        let mut release_changed = false;
        for raw in matched {
            let volume = raw.as_i64().and_then(|value| i32::try_from(value).ok());
            let drop = match volume {
                Some(volume) if !is_numbered_volume(volume) => true,
                Some(volume) => present.contains(volume),
                None => false,
            };

            if drop {
                release_changed = true;
                if let Some(volume) = volume.filter(|volume| is_numbered_volume(*volume)) {
                    removed_volumes.insert(volume);
                }
            } else {
                kept_volumes.push(raw.clone());
            }
        }

        if !release_changed {
            kept_releases.push(release.clone());
            continue;
        }

        changed = true;
        if kept_volumes.is_empty() {
            continue;
        }

        let mut release = release.clone();
        release[MATCHED_MISSING_VOLUMES] = Value::Array(kept_volumes);
        kept_releases.push(release);
    }

    PrunedReleases {
        releases: kept_releases,
        removed_volumes,
        changed,
    }
}

/// Drop legacy non-tome volumes without changing the counter.
pub fn normalize_releases(releases: &[Value]) -> PrunedReleases {
    prune_releases(releases, &PresentVolumes::new())
}

/// Outcome of pruning one series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PruneOutcome {
    /// No `available_downloads` row exists for the series.
    Missing,
    /// Nothing needed to change.
    Unchanged,
    /// Releases and/or the counter were rewritten.
    Updated {
        removed_volumes: BTreeSet<i32>,
        missing_count: i32,
    },
    /// The whole row was deleted.
    Deleted,
}

/// One changed series in a library-wide prune.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesPrune {
    pub series_id: Uuid,
    pub outcome: PruneOutcome,
}

/// Upserts a series' releases, merging them with the ones already stored.
pub async fn merge_releases<T>(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Uuid,
    missing_count: i32,
    releases: &T,
) -> sqlx::Result<()>
where
    T: Serialize + ?Sized,
{
    let releases_json = serde_json::to_value(releases).unwrap_or_else(|_| Value::Array(Vec::new()));

    sqlx::query(
        "INSERT INTO available_downloads (library_id, series_id, missing_count, available_releases, updated_at) \
         VALUES ($1, $2, $3, $4, NOW()) \
         ON CONFLICT (series_id) DO UPDATE SET \
           missing_count = EXCLUDED.missing_count, \
           available_releases = merge_releases(available_downloads.available_releases, EXCLUDED.available_releases), \
           updated_at = NOW()",
    )
    .bind(library_id)
    .bind(series_id)
    .bind(missing_count)
    .bind(releases_json)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Updates only the stored `missing_count` for a series, leaving its releases
/// untouched. Used when a run found nothing, so previous results survive.
pub async fn set_missing_count(
    pool: &PgPool,
    series_id: Uuid,
    missing_count: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE available_downloads SET missing_count = $2, updated_at = NOW() WHERE series_id = $1",
    )
    .bind(series_id)
    .bind(missing_count)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Deletes the availability row of one series (series complete or dismissed).
pub async fn delete_for_series(pool: &PgPool, series_id: Uuid) -> sqlx::Result<u64> {
    sqlx::query("DELETE FROM available_downloads WHERE series_id = $1")
        .bind(series_id)
        .execute(pool)
        .await
        .map(|result| result.rows_affected())
}

/// Deletes entries that no longer hold any release, optionally scoped to a
/// single library.
pub async fn delete_empty(pool: &PgPool, library_id: Option<Uuid>) -> sqlx::Result<u64> {
    let query = match library_id {
        Some(_) => {
            "DELETE FROM available_downloads \
             WHERE library_id = $1 AND (available_releases = '[]'::jsonb OR available_releases IS NULL)"
        }
        None => {
            "DELETE FROM available_downloads \
             WHERE available_releases = '[]'::jsonb OR available_releases IS NULL"
        }
    };

    let mut query = sqlx::query(query);
    if let Some(id) = library_id {
        query = query.bind(id);
    }
    query
        .execute(pool)
        .await
        .map(|result| result.rows_affected())
}

/// Prune the releases of one series.
pub async fn prune_series(
    pool: &PgPool,
    series_id: Uuid,
    present: &PresentVolumes,
) -> sqlx::Result<PruneOutcome> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT id, missing_count, available_releases \
         FROM available_downloads WHERE series_id = $1 FOR UPDATE",
    )
    .bind(series_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(PruneOutcome::Missing);
    };

    let outcome = apply_prune(
        &mut tx,
        row.get("id"),
        row.get("missing_count"),
        row.get("available_releases"),
        present,
    )
    .await?;
    tx.commit().await?;
    Ok(outcome)
}

/// Prune every availability row of a library in one transaction.
///
/// `present_by_series` must contain the coverage of every series that still has
/// an availability row. Series missing from the map are normalized only.
pub async fn prune_library(
    pool: &PgPool,
    library_id: Uuid,
    present_by_series: &HashMap<Uuid, PresentVolumes>,
) -> sqlx::Result<Vec<SeriesPrune>> {
    let mut tx = pool.begin().await?;
    let rows = sqlx::query(
        "SELECT id, series_id, missing_count, available_releases \
         FROM available_downloads WHERE library_id = $1 FOR UPDATE",
    )
    .bind(library_id)
    .fetch_all(&mut *tx)
    .await?;

    let empty = PresentVolumes::new();
    let mut outcomes = Vec::with_capacity(rows.len());
    for row in rows {
        let series_id: Uuid = row.get("series_id");
        let present = present_by_series.get(&series_id).unwrap_or(&empty);
        let outcome = apply_prune(
            &mut tx,
            row.get("id"),
            row.get("missing_count"),
            row.get("available_releases"),
            present,
        )
        .await?;
        outcomes.push(SeriesPrune { series_id, outcome });
    }

    tx.commit().await?;
    Ok(outcomes)
}

/// Drop legacy non-tome volumes from every release of a library and delete rows
/// left without releases. The counter is left untouched because non-tomes never
/// counted as missing.
pub async fn normalize_library(pool: &PgPool, library_id: Uuid) -> sqlx::Result<usize> {
    let mut tx = pool.begin().await?;
    let rows = sqlx::query(
        "SELECT id, available_releases FROM available_downloads \
         WHERE library_id = $1 FOR UPDATE",
    )
    .bind(library_id)
    .fetch_all(&mut *tx)
    .await?;

    let mut changed = 0;
    for row in rows {
        let id: Uuid = row.get("id");
        let Some(Value::Array(releases)) = row.get::<Option<Value>, _>("available_releases") else {
            continue;
        };

        let pruned = normalize_releases(&releases);
        if !pruned.changed {
            continue;
        }

        changed += 1;
        if pruned.releases.is_empty() {
            sqlx::query("DELETE FROM available_downloads WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query(
                "UPDATE available_downloads SET available_releases = $1, updated_at = NOW() \
                 WHERE id = $2",
            )
            .bind(Value::Array(pruned.releases))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(changed)
}

async fn apply_prune(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    missing_count: i32,
    releases_json: Option<Value>,
    present: &PresentVolumes,
) -> sqlx::Result<PruneOutcome> {
    if present.is_complete() {
        delete_row(tx, id).await?;
        return Ok(PruneOutcome::Deleted);
    }

    let Some(releases_json) = releases_json else {
        delete_row(tx, id).await?;
        return Ok(PruneOutcome::Deleted);
    };

    let Value::Array(releases) = releases_json else {
        return Ok(PruneOutcome::Unchanged);
    };

    if releases.is_empty() {
        delete_row(tx, id).await?;
        return Ok(PruneOutcome::Deleted);
    }

    let pruned = prune_releases(&releases, present);
    if !pruned.changed {
        return Ok(PruneOutcome::Unchanged);
    }

    if pruned.releases.is_empty() {
        delete_row(tx, id).await?;
        return Ok(PruneOutcome::Deleted);
    }

    let missing_count = pruned.missing_count_after(missing_count);
    sqlx::query(
        "UPDATE available_downloads SET available_releases = $1, missing_count = $2, \
         updated_at = NOW() WHERE id = $3",
    )
    .bind(Value::Array(pruned.releases))
    .bind(missing_count)
    .bind(id)
    .execute(&mut **tx)
    .await?;

    Ok(PruneOutcome::Updated {
        removed_volumes: pruned.removed_volumes,
        missing_count,
    })
}

async fn delete_row(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM available_downloads WHERE id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn release(title: &str, volumes: &[i64]) -> Value {
        json!({ "title": title, "matched_missing_volumes": volumes })
    }

    #[test]
    fn prune_removes_present_volumes_and_dedups_the_counter() {
        let releases = vec![release("R1", &[1]), release("R2", &[1, 2])];

        let pruned = prune_releases(&releases, &PresentVolumes::from_volumes([1]));

        assert_eq!(pruned.releases, vec![release("R2", &[2])]);
        assert_eq!(pruned.removed_volumes, BTreeSet::from([1]));
        assert_eq!(
            pruned.missing_count_after(2),
            1,
            "the same volume offered by two releases decrements once"
        );
        assert!(pruned.changed);
    }

    #[test]
    fn prune_drops_releases_with_no_remaining_volume() {
        let releases = vec![release("R1", &[1]), release("R2", &[1, 2])];

        let pruned = prune_releases(&releases, &PresentVolumes::from_volumes([1, 2]));

        assert!(pruned.is_empty());
        assert_eq!(pruned.removed_volumes, BTreeSet::from([1, 2]));
        assert!(pruned.changed);
    }

    #[test]
    fn normalize_drops_non_tomes_without_touching_the_counter() {
        let releases = vec![release("R1", &[0]), release("R2", &[0, 1])];

        let pruned = normalize_releases(&releases);

        assert_eq!(pruned.releases, vec![release("R2", &[1])]);
        assert!(pruned.removed_volumes.is_empty());
        assert_eq!(
            pruned.missing_count_after(2),
            2,
            "volume 0 never counted as a missing tome"
        );
    }

    #[test]
    fn prune_keeps_releases_without_a_match_list() {
        let release = json!({ "title": "legacy" });

        let pruned = prune_releases(std::slice::from_ref(&release), &PresentVolumes::complete());

        assert_eq!(pruned.releases, vec![release]);
        assert!(!pruned.changed);
    }

    #[test]
    fn prune_drops_releases_with_an_empty_match_list() {
        let releases = vec![json!({ "title": "empty", "matched_missing_volumes": [] })];

        let pruned = prune_releases(&releases, &PresentVolumes::new());

        assert!(pruned.is_empty());
        assert!(pruned.changed);
    }

    #[test]
    fn complete_series_removes_every_numbered_volume() {
        let releases = vec![release("R1", &[1, 2])];

        let pruned = prune_releases(&releases, &PresentVolumes::complete());

        assert!(pruned.is_empty());
        assert_eq!(pruned.removed_volumes, BTreeSet::from([1, 2]));
    }

    #[test]
    fn prune_ignores_malformed_volumes() {
        let release = json!({
            "title": "R1",
            "matched_missing_volumes": ["1", 2]
        });

        let pruned = prune_releases(&[release], &PresentVolumes::from_volumes([2]));

        assert_eq!(
            pruned.releases,
            vec![json!({
                "title": "R1",
                "matched_missing_volumes": ["1"]
            })]
        );
        assert_eq!(pruned.removed_volumes, BTreeSet::from([2]));
    }

    async fn seed_library(pool: &PgPool, name: &str) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind(name)
            .bind(format!("/libraries/{name}"))
            .execute(pool)
            .await
            .unwrap();
        library_id
    }

    async fn seed_series(pool: &PgPool, library_id: Uuid, name: &str) -> Uuid {
        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        series_id
    }

    async fn seed_availability(
        pool: &PgPool,
        library_id: Uuid,
        series_id: Uuid,
        missing_count: i32,
        releases: Value,
    ) {
        sqlx::query(
            "INSERT INTO available_downloads \
             (library_id, series_id, missing_count, available_releases, updated_at) \
             VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(library_id)
        .bind(series_id)
        .bind(missing_count)
        .bind(releases)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn stored_releases(pool: &PgPool, series_id: Uuid) -> Option<(i32, Value)> {
        sqlx::query_as(
            "SELECT missing_count, available_releases FROM available_downloads WHERE series_id = $1",
        )
        .bind(series_id)
        .fetch_optional(pool)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn merge_releases_merges_and_sets_the_counter(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;

        merge_releases(&pool, library_id, series_id, 2, &[release("T01", &[1])])
            .await
            .unwrap();
        merge_releases(&pool, library_id, series_id, 1, &[release("T02", &[2])])
            .await
            .unwrap();

        let (missing_count, releases) = stored_releases(&pool, series_id).await.unwrap();
        assert_eq!(missing_count, 1);
        assert_eq!(releases.as_array().unwrap().len(), 2);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn merge_releases_dedups_on_title(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;

        merge_releases(&pool, library_id, series_id, 2, &[release("T01", &[1])])
            .await
            .unwrap();
        merge_releases(&pool, library_id, series_id, 2, &[release("T01", &[1])])
            .await
            .unwrap();

        let (_, releases) = stored_releases(&pool, series_id).await.unwrap();
        assert_eq!(releases.as_array().unwrap().len(), 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn set_missing_count_keeps_stored_releases(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        let releases = vec![release("T01", &[1])];
        merge_releases(&pool, library_id, series_id, 3, &releases)
            .await
            .unwrap();

        set_missing_count(&pool, series_id, 1).await.unwrap();

        assert_eq!(
            stored_releases(&pool, series_id).await.unwrap(),
            (1, Value::Array(releases))
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn prune_series_updates_releases_and_counter(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        seed_availability(
            &pool,
            library_id,
            series_id,
            4,
            json!([
                {"title": "R1", "matched_missing_volumes": [1, 2]},
                {"title": "R2", "matched_missing_volumes": [1, 3]},
                {"title": "R3", "matched_missing_volumes": [4]}
            ]),
        )
        .await;

        let outcome = prune_series(&pool, series_id, &PresentVolumes::from_volumes([1]))
            .await
            .unwrap();

        assert_eq!(
            outcome,
            PruneOutcome::Updated {
                removed_volumes: BTreeSet::from([1]),
                missing_count: 3,
            }
        );
        let (missing_count, releases) = stored_releases(&pool, series_id).await.unwrap();
        assert_eq!(missing_count, 3);
        assert_eq!(
            releases,
            json!([
                {"title": "R1", "matched_missing_volumes": [2]},
                {"title": "R2", "matched_missing_volumes": [3]},
                {"title": "R3", "matched_missing_volumes": [4]}
            ])
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn prune_series_deletes_when_every_release_is_covered(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        seed_availability(
            &pool,
            library_id,
            series_id,
            2,
            json!([{"title": "R1", "matched_missing_volumes": [1, 2]}]),
        )
        .await;

        let outcome = prune_series(&pool, series_id, &PresentVolumes::from_volumes([1, 2]))
            .await
            .unwrap();

        assert_eq!(outcome, PruneOutcome::Deleted);
        assert!(stored_releases(&pool, series_id).await.is_none());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn prune_series_deletes_a_complete_series(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        seed_availability(
            &pool,
            library_id,
            series_id,
            2,
            json!([{"title": "R1", "matched_missing_volumes": [1, 2]}]),
        )
        .await;

        let outcome = prune_series(&pool, series_id, &PresentVolumes::complete())
            .await
            .unwrap();

        assert_eq!(outcome, PruneOutcome::Deleted);
        assert!(stored_releases(&pool, series_id).await.is_none());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn prune_series_is_a_noop_when_no_matched_volume_is_present(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        let releases = json!([{"title": "R1", "matched_missing_volumes": [1, 2]}]);
        seed_availability(&pool, library_id, series_id, 2, releases.clone()).await;

        let outcome = prune_series(&pool, series_id, &PresentVolumes::from_volumes([3]))
            .await
            .unwrap();

        assert_eq!(outcome, PruneOutcome::Unchanged);
        assert_eq!(
            stored_releases(&pool, series_id).await.unwrap(),
            (2, releases)
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn prune_library_handles_every_series_in_one_pass(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let pruned_series = seed_series(&pool, library_id, "Astérix").await;
        let kept_series = seed_series(&pool, library_id, "Tintin").await;
        seed_availability(
            &pool,
            library_id,
            pruned_series,
            2,
            json!([{"title": "R1", "matched_missing_volumes": [1, 2]}]),
        )
        .await;
        seed_availability(
            &pool,
            library_id,
            kept_series,
            1,
            json!([{"title": "R2", "matched_missing_volumes": [3]}]),
        )
        .await;

        let present = HashMap::from([(pruned_series, PresentVolumes::from_volumes([1]))]);
        let outcomes = prune_library(&pool, library_id, &present).await.unwrap();

        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().any(|series| {
            series.series_id == pruned_series
                && matches!(series.outcome, PruneOutcome::Updated { .. })
        }));
        assert!(outcomes.iter().any(|series| {
            series.series_id == kept_series && series.outcome == PruneOutcome::Unchanged
        }));
        let (missing_count, releases) = stored_releases(&pool, pruned_series).await.unwrap();
        assert_eq!(missing_count, 1);
        assert_eq!(
            releases,
            json!([{"title": "R1", "matched_missing_volumes": [2]}])
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn prune_series_deletes_a_row_with_no_release(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        seed_availability(&pool, library_id, series_id, 0, json!([])).await;

        let outcome = prune_series(&pool, series_id, &PresentVolumes::new())
            .await
            .unwrap();

        assert_eq!(outcome, PruneOutcome::Deleted);
        assert!(stored_releases(&pool, series_id).await.is_none());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn normalize_library_removes_legacy_volume_zero(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let kept_series = seed_series(&pool, library_id, "Astérix").await;
        let deleted_series = seed_series(&pool, library_id, "Tintin").await;
        seed_availability(
            &pool,
            library_id,
            kept_series,
            2,
            json!([{"title": "R1", "matched_missing_volumes": [0, 1]}]),
        )
        .await;
        seed_availability(
            &pool,
            library_id,
            deleted_series,
            1,
            json!([{"title": "R2", "matched_missing_volumes": [0]}]),
        )
        .await;

        let changed = normalize_library(&pool, library_id).await.unwrap();

        assert_eq!(changed, 2);
        let (missing_count, releases) = stored_releases(&pool, kept_series).await.unwrap();
        assert_eq!(missing_count, 2, "volume 0 never counted");
        assert_eq!(
            releases,
            json!([{"title": "R1", "matched_missing_volumes": [1]}])
        );
        assert!(stored_releases(&pool, deleted_series).await.is_none());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn delete_empty_can_be_scoped_to_a_library(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        let other_library = seed_library(&pool, "other").await;
        let other_series = seed_series(&pool, other_library, "Tintin").await;

        for (library, series) in [(library_id, series_id), (other_library, other_series)] {
            seed_availability(&pool, library, series, 0, json!([])).await;
        }

        assert_eq!(delete_empty(&pool, Some(library_id)).await.unwrap(), 1);
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM available_downloads")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 1, "only the scoped library should be cleaned");

        assert_eq!(delete_empty(&pool, None).await.unwrap(), 1);
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM available_downloads")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 0);
    }
}
