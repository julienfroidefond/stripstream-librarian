//! Shared publication of collected releases.
//!
//! The three download sources — Prowlarr detection, the Prowlarr RSS poll and
//! the DDL catalogue — find releases differently: a search per series, one
//! global fetch matched in memory, a scan of provider catalogues. Their loops
//! therefore stay in their own modules. What they must not re-implement is what
//! happens once a series' releases are collected:
//!
//! * drop blacklisted titles;
//! * stamp `detected_at` exactly once, keeping the value `merge_releases`
//!   already stored for a known release;
//! * emit the job's "found" event with the source's vocabulary;
//! * merge into `available_downloads` under an explicit error policy;
//! * count the releases newly discovered since the job started, and the
//!   per-series cleanup that follows a run.
//!
//! Those differences are **data** (`StorePolicy`, `Cleanup`), not three copies
//! that can silently drift. `merge_releases` remains the only owner of the merge
//! itself (`stripstream_core::availability`, ADR 0006).

use std::collections::HashSet;

use sqlx::{PgPool, Row};
use tracing::warn;
use uuid::Uuid;

use stripstream_core::availability;

use super::release::AvailableReleaseDto;
use crate::jobs::helpers::{insert_entity_event, insert_event};

/// One series a source is about to publish releases for.
pub(super) struct ReleaseTarget<'a> {
    pub library_id: Uuid,
    pub series_id: Uuid,
    pub series_name: &'a str,
}

/// Where the "found" event attaches: a bare entity name (detection, RSS) or a
/// typed entity (`series` for the DDL search).
#[derive(Clone, Copy)]
pub(super) enum EntityKind {
    Name,
    Typed(&'static str),
}

/// How much of the release set the "found" event carries.
#[derive(Clone, Copy)]
pub(super) enum ReleaseDetail {
    /// `{release_count, missing_count, available_releases}` — detection, RSS.
    Releases,
    /// `{release_count, missing_count}` — DDL keeps offers out of job events.
    CountOnly,
}

/// What a source does when `merge_releases` fails.
#[derive(Clone, Copy)]
pub(super) enum MergeFailure {
    /// Log and keep going: a failed merge is not a failed series.
    Ignore,
    /// Surface the SQL error to the job, which turns it into a job error.
    FailJob,
}

/// The publication policy of one source.
pub(super) struct StorePolicy {
    pub event: &'static str,
    pub entity: EntityKind,
    pub detail: ReleaseDetail,
    pub on_merge_error: MergeFailure,
}

impl StorePolicy {
    /// Prowlarr detection: `downloads_found`, full release set in the event.
    pub(super) fn prowlarr_detection() -> Self {
        Self {
            event: "downloads_found",
            entity: EntityKind::Name,
            detail: ReleaseDetail::Releases,
            on_merge_error: MergeFailure::Ignore,
        }
    }

    /// Prowlarr RSS poll: same event shape as detection.
    pub(super) fn prowlarr_rss() -> Self {
        Self {
            event: "downloads_found",
            entity: EntityKind::Name,
            detail: ReleaseDetail::Releases,
            on_merge_error: MergeFailure::Ignore,
        }
    }

    /// DDL catalogue: its own event type and entity, counts only, and a merge
    /// failure fails the run.
    pub(super) fn ddl_catalogue() -> Self {
        Self {
            event: "ddl_downloads_found",
            entity: EntityKind::Typed("series"),
            detail: ReleaseDetail::CountOnly,
            on_merge_error: MergeFailure::FailJob,
        }
    }
}

/// What happened to one series' collected releases.
pub(super) enum Stored {
    /// Every collected release was blacklisted (or none was collected): nothing
    /// was written and no event was emitted.
    Nothing,
    /// Number of releases written.
    Found(usize),
}

/// Publish one series' releases: blacklist, stamp, event, merge.
pub(super) async fn store_releases(
    pool: &PgPool,
    job_id: Uuid,
    target: &ReleaseTarget<'_>,
    missing_count: i32,
    releases: Vec<AvailableReleaseDto>,
    blacklist: &HashSet<String>,
    policy: &StorePolicy,
) -> Result<Stored, sqlx::Error> {
    let mut releases: Vec<AvailableReleaseDto> = releases
        .into_iter()
        .filter(|release| !blacklist.contains(&release.title))
        .collect();

    if releases.is_empty() {
        return Ok(Stored::Nothing);
    }

    let detected_at = chrono::Utc::now().to_rfc3339();
    for release in &mut releases {
        release.detected_at = Some(detected_at.clone());
    }

    let detail = match policy.detail {
        ReleaseDetail::Releases => serde_json::json!({
            "release_count": releases.len(),
            "missing_count": missing_count,
            "available_releases": serde_json::to_value(&releases).ok(),
        }),
        ReleaseDetail::CountOnly => serde_json::json!({
            "release_count": releases.len(),
            "missing_count": missing_count,
        }),
    };
    match policy.entity {
        EntityKind::Name => {
            insert_entity_event(
                pool,
                job_id,
                policy.event,
                "info",
                Some(target.series_name),
                None,
                Some(detail),
            )
            .await;
        }
        EntityKind::Typed(entity_type) => {
            insert_event(
                pool,
                job_id,
                policy.event,
                "info",
                Some(entity_type),
                Some(target.series_name),
                None,
                Some(detail),
            )
            .await;
        }
    }

    match availability::merge_releases(
        pool,
        target.library_id,
        target.series_id,
        missing_count,
        &releases,
    )
    .await
    {
        Ok(()) => Ok(Stored::Found(releases.len())),
        Err(error) => match policy.on_merge_error {
            MergeFailure::Ignore => {
                warn!(
                    series = target.series_name,
                    %error,
                    "release merge failed, keeping the stored offers"
                );
                Ok(Stored::Found(releases.len()))
            }
            MergeFailure::FailJob => Err(error),
        },
    }
}

/// What to do with `available_downloads` once a run has published its releases.
#[derive(Clone, Copy)]
pub(super) enum Cleanup {
    /// Nothing (RSS poll): the next detection cleans up.
    Keep,
    /// Drop the rows whose release list became empty (DDL search, global).
    DeleteEmpty(Option<Uuid>),
    /// Drop legacy non-tome volumes, then empty rows (detection, per library).
    NormalizeThenDeleteEmpty(Uuid),
}

pub(super) async fn cleanup(pool: &PgPool, cleanup: Cleanup) {
    match cleanup {
        Cleanup::Keep => {}
        Cleanup::DeleteEmpty(library_id) => {
            let _ = availability::delete_empty(pool, library_id).await;
        }
        Cleanup::NormalizeThenDeleteEmpty(library_id) => {
            let _ = availability::normalize_library(pool, library_id).await;
            let _ = availability::delete_empty(pool, Some(library_id)).await;
        }
    }
}

/// Releases first seen at or after the job started, for the run's stats.
///
/// `merge_releases` preserves the `detected_at` of a release that already
/// existed, so a timestamp at or after the job start means genuinely new.
pub(super) async fn new_releases_since(
    pool: &PgPool,
    job_started_at: chrono::DateTime<chrono::Utc>,
    library_id: Option<Uuid>,
) -> i64 {
    let count = match library_id {
        Some(library_id) => {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM available_downloads ad,
                     jsonb_array_elements(ad.available_releases) AS rel
                WHERE ad.library_id = $1
                  AND rel->>'detected_at' IS NOT NULL
                  AND (rel->>'detected_at')::timestamptz >= $2
                "#,
            )
            .bind(library_id)
            .bind(job_started_at)
            .fetch_one(pool)
            .await
        }
        None => {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM available_downloads ad,
                     jsonb_array_elements(ad.available_releases) AS rel
                WHERE rel->>'detected_at' IS NOT NULL
                  AND (rel->>'detected_at')::timestamptz >= $1
                "#,
            )
            .bind(job_started_at)
            .fetch_one(pool)
            .await
        }
    };
    count.unwrap_or(0)
}

/// Releases first seen since the job started, grouped by series and capped at
/// 10, for the completion notification.
pub(super) async fn new_items_since(
    pool: &PgPool,
    job_started_at: chrono::DateTime<chrono::Utc>,
    library_id: Option<Uuid>,
) -> Vec<(String, String)> {
    let query = match library_id {
        Some(_) => {
            "SELECT DISTINCT s.name AS series_name, rel->>'title' AS release_title \
             FROM available_downloads ad \
             CROSS JOIN jsonb_array_elements(ad.available_releases) AS rel \
             JOIN series s ON s.id = ad.series_id \
             WHERE ad.library_id = $1 \
               AND (rel->>'detected_at')::timestamptz >= $2 \
             ORDER BY s.name LIMIT 10"
        }
        None => {
            "SELECT DISTINCT s.name AS series_name, rel->>'title' AS release_title \
             FROM available_downloads ad \
             CROSS JOIN jsonb_array_elements(ad.available_releases) AS rel \
             JOIN series s ON s.id = ad.series_id \
             WHERE (rel->>'detected_at')::timestamptz >= $1 \
             ORDER BY s.name LIMIT 10"
        }
    };
    let mut query = sqlx::query(query);
    if let Some(library_id) = library_id {
        query = query.bind(library_id);
    }
    query
        .bind(job_started_at)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("series_name"),
                row.get::<String, _>("release_title"),
            )
        })
        .collect()
}

/// Per-event counters read back from `index_job_events`, the shape the
/// download jobs report in their stats.
#[derive(Default)]
pub(super) struct EventCounts {
    pub found: i64,
    pub not_found: i64,
    pub no_missing: i64,
    pub no_metadata: i64,
    pub errors: i64,
}

pub(super) async fn event_counts(pool: &PgPool, job_id: Uuid) -> Result<EventCounts, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT event_type, COUNT(*) AS cnt FROM index_job_events \
         WHERE job_id = $1 GROUP BY event_type",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await?;

    let mut counts = EventCounts::default();
    for row in &rows {
        let count: i64 = row.get("cnt");
        match row.get::<String, _>("event_type").as_str() {
            "downloads_found" => counts.found = count,
            "downloads_not_found" => counts.not_found = count,
            "no_missing_volumes" => counts.no_missing = count,
            "no_metadata_link" => counts.no_metadata = count,
            "error" => counts.errors = count,
            _ => {}
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed_library(pool: &PgPool, name: &str) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO libraries (id, name, root_path) \
             VALUES (gen_random_uuid(), $1, $2) RETURNING id",
        )
        .bind(name)
        .bind(format!("/libraries/{name}"))
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn seed_series(pool: &PgPool, library_id: Uuid, name: &str) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO series (id, library_id, name) \
             VALUES (gen_random_uuid(), $1, $2) RETURNING id",
        )
        .bind(library_id)
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn seed_job(pool: &PgPool) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO index_jobs (id, type, status, started_at) \
             VALUES (gen_random_uuid(), 'prowlarr_rss', 'running', NOW()) RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn release(title: &str) -> AvailableReleaseDto {
        AvailableReleaseDto {
            title: title.to_string(),
            size: 100,
            download_url: None,
            indexer: None,
            seeders: None,
            matched_missing_volumes: vec![1],
            all_volumes: vec![1],
            has_failed: false,
            detected_at: None,
            match_confidence: "high".to_string(),
            match_reasons: Vec::new(),
            provider: None,
            external_id: None,
        }
    }

    fn blacklist(titles: &[&str]) -> HashSet<String> {
        titles.iter().map(|title| title.to_string()).collect()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn store_releases_filters_blacklisted_titles_and_stamps_detected_at(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        let job_id = seed_job(&pool).await;
        let target = ReleaseTarget {
            library_id,
            series_id,
            series_name: "Astérix",
        };

        let stored = store_releases(
            &pool,
            job_id,
            &target,
            2,
            vec![release("Kept"), release("Hidden")],
            &blacklist(&["Hidden"]),
            &StorePolicy::prowlarr_detection(),
        )
        .await
        .unwrap();

        assert!(matches!(stored, Stored::Found(1)));

        let releases: serde_json::Value = sqlx::query_scalar(
            "SELECT available_releases FROM available_downloads WHERE series_id = $1",
        )
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let releases = releases.as_array().unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0]["title"], "Kept");
        assert!(releases[0]["detected_at"].as_str().is_some());

        let detail: serde_json::Value = sqlx::query_scalar(
            "SELECT detail FROM index_job_events \
             WHERE job_id = $1 AND event_type = 'downloads_found'",
        )
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(detail["release_count"], 1);
        assert_eq!(detail["missing_count"], 2);
        assert_eq!(detail["available_releases"][0]["title"], "Kept");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn store_releases_writes_nothing_when_every_title_is_blacklisted(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        let job_id = seed_job(&pool).await;
        let target = ReleaseTarget {
            library_id,
            series_id,
            series_name: "Astérix",
        };

        let stored = store_releases(
            &pool,
            job_id,
            &target,
            1,
            vec![release("Hidden")],
            &blacklist(&["Hidden"]),
            &StorePolicy::prowlarr_rss(),
        )
        .await
        .unwrap();

        assert!(matches!(stored, Stored::Nothing));
        let rows: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM available_downloads WHERE series_id = $1")
                .bind(series_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let events: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM index_job_events WHERE job_id = $1")
                .bind(job_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(rows, 0);
        assert_eq!(events, 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_failed_merge_is_either_logged_or_surfaced_by_the_policy(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let job_id = seed_job(&pool).await;
        // A series id that does not exist makes the merge violate its FK.
        let target = ReleaseTarget {
            library_id,
            series_id: Uuid::new_v4(),
            series_name: "Ghost",
        };

        let ignored = store_releases(
            &pool,
            job_id,
            &target,
            1,
            vec![release("Release")],
            &HashSet::new(),
            &StorePolicy::prowlarr_detection(),
        )
        .await;
        assert!(matches!(ignored, Ok(Stored::Found(1))));

        let surfaced = store_releases(
            &pool,
            job_id,
            &target,
            1,
            vec![release("Release")],
            &HashSet::new(),
            &StorePolicy::ddl_catalogue(),
        )
        .await;
        assert!(surfaced.is_err());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn the_ddl_policy_emits_a_typed_count_only_event(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        let job_id = seed_job(&pool).await;
        let target = ReleaseTarget {
            library_id,
            series_id,
            series_name: "Astérix",
        };

        store_releases(
            &pool,
            job_id,
            &target,
            3,
            vec![release("Release")],
            &HashSet::new(),
            &StorePolicy::ddl_catalogue(),
        )
        .await
        .unwrap();

        let (entity_type, detail): (Option<String>, serde_json::Value) = sqlx::query_as(
            "SELECT entity_type, detail FROM index_job_events \
             WHERE job_id = $1 AND event_type = 'ddl_downloads_found'",
        )
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(entity_type.as_deref(), Some("series"));
        assert_eq!(detail["release_count"], 1);
        assert_eq!(detail["missing_count"], 3);
        assert!(detail.get("available_releases").is_none());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn cleanup_drops_empty_rows_only_when_the_policy_asks(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        sqlx::query(
            "INSERT INTO available_downloads \
                 (library_id, series_id, missing_count, available_releases) \
             VALUES ($1, $2, 1, '[]'::jsonb)",
        )
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        cleanup(&pool, Cleanup::Keep).await;
        let kept: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM available_downloads WHERE series_id = $1")
                .bind(series_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(kept, 1);

        cleanup(&pool, Cleanup::DeleteEmpty(Some(library_id))).await;
        let dropped: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM available_downloads WHERE series_id = $1")
                .bind(series_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(dropped, 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn new_releases_and_items_scope_to_the_library(pool: PgPool) {
        let first = seed_library(&pool, "first").await;
        let second = seed_library(&pool, "second").await;
        let first_series = seed_series(&pool, first, "Astérix").await;
        let second_series = seed_series(&pool, second, "Tintin").await;
        let since = chrono::Utc::now() - chrono::Duration::minutes(1);
        let stamp = chrono::Utc::now().to_rfc3339();

        for (library_id, series_id, title) in [
            (first, first_series, "Astérix T1"),
            (second, second_series, "Tintin T1"),
        ] {
            sqlx::query(
                "INSERT INTO available_downloads \
                     (library_id, series_id, missing_count, available_releases) \
                 VALUES ($1, $2, 1, $3)",
            )
            .bind(library_id)
            .bind(series_id)
            .bind(serde_json::json!([{ "title": title, "detected_at": stamp }]))
            .execute(&pool)
            .await
            .unwrap();
        }

        assert_eq!(new_releases_since(&pool, since, Some(first)).await, 1);
        assert_eq!(new_releases_since(&pool, since, None).await, 2);
        assert_eq!(
            new_items_since(&pool, since, Some(first)).await,
            vec![("Astérix".to_string(), "Astérix T1".to_string())]
        );
        assert_eq!(new_items_since(&pool, since, None).await.len(), 2);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn event_counts_reads_every_known_type(pool: PgPool) {
        let job_id = seed_job(&pool).await;
        for event in [
            "downloads_found",
            "downloads_found",
            "downloads_not_found",
            "no_missing_volumes",
            "no_metadata_link",
            "error",
        ] {
            insert_entity_event(&pool, job_id, event, "info", None, None, None).await;
        }

        let counts = event_counts(&pool, job_id).await.unwrap();
        assert_eq!(counts.found, 2);
        assert_eq!(counts.not_found, 1);
        assert_eq!(counts.no_missing, 1);
        assert_eq!(counts.no_metadata, 1);
        assert_eq!(counts.errors, 1);
    }
}
