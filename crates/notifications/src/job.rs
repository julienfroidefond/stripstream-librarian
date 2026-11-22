//! The notification policy of a finished indexer job.
//!
//! The indexer knows *when* a job ends; it must not know *what* that means for
//! the chat. [`notify_job`] is the only entry point: it loads the job, its
//! library, its book and the books a scan discovered, then lets [`events_for`]
//! turn those facts into events. Both halves have one reason to change — a new
//! job category, a new message — and neither needs the worker's loop.

use std::collections::HashMap;

use sqlx::{PgPool, Row};
use tracing::warn;
use uuid::Uuid;

use stripstream_jobs::{JobCategory, ScanStats};

use crate::{deliver, job_type_category, load_telegram_config, NotificationEvent};

/// What the worker knows when a job reaches a terminal state.
#[derive(Clone, Debug)]
pub struct JobFinished {
    pub job_id: Uuid,
    pub library_id: Option<Uuid>,
    pub outcome: JobOutcome,
    /// Wall-clock time of the run, measured by the worker around `process_job`.
    pub duration_seconds: u64,
}

/// How a job ended.
#[derive(Clone, Debug)]
pub enum JobOutcome {
    Completed,
    Failed { error: String },
    Cancelled,
}

/// Notify the chat about one finished job, in the background.
///
/// The Telegram configuration is loaded once for the whole batch: the events of
/// a scan are sent sequentially, in the order the job produced them.
pub fn notify_job(pool: PgPool, finished: JobFinished) {
    tokio::spawn(async move {
        let Some(config) = load_telegram_config(&pool).await else {
            return; // disabled or not configured
        };
        for event in job_events(&pool, &finished).await {
            deliver(&config, event).await;
        }
    });
}

/// The events a finished job deserves, read from the database.
///
/// Public so the mapping from `index_jobs` rows to events is testable without a
/// Telegram call: [`deliver`] only adds the config gate, formatting and the send.
pub async fn job_events(pool: &PgPool, finished: &JobFinished) -> Vec<NotificationEvent> {
    let facts = load_facts(pool, finished).await;
    events_for(finished, facts)
}

/// Facts about a terminal job, as the messages need them.
struct JobFacts {
    job_type: String,
    library_name: Option<String>,
    book_title: Option<String>,
    thumbnail_path: Option<String>,
    stats: ScanStats,
    series: Vec<SeriesDiscovered>,
}

/// The books a scan discovered in one series, grouped for a single message.
struct SeriesDiscovered {
    series_name: String,
    thumbnail_path: Option<String>,
    book_titles: Vec<String>,
}

async fn load_facts(pool: &PgPool, finished: &JobFinished) -> JobFacts {
    let (job_type, book_id) = load_job_row(pool, finished.job_id).await;
    let category = job_type_category(&job_type);

    let library_name = match finished.library_id {
        Some(library_id) => sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
            .bind(library_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten(),
        None => None,
    };

    let (book_title, thumbnail_path) = match book_id {
        Some(book_id) => {
            let row = sqlx::query("SELECT title, thumbnail_path FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
            match row {
                Some(row) => (row.get("title"), row.get("thumbnail_path")),
                None => (None, None),
            }
        }
        None => (None, None),
    };

    // Only a scan discovers books; asking for the series of a thumbnail job
    // would be an empty query on every single-unit job.
    let series = if category == JobCategory::Scan {
        load_discovered_series(pool, finished.job_id).await
    } else {
        Vec::new()
    };

    JobFacts {
        job_type,
        library_name,
        book_title,
        thumbnail_path,
        stats: load_stats(pool, finished.job_id).await,
        series,
    }
}

/// The job's type and book, or `("unknown", None)` when the row cannot be read.
/// Notifications are best-effort: a missing row must not lose the message.
async fn load_job_row(pool: &PgPool, job_id: Uuid) -> (String, Option<Uuid>) {
    let row = sqlx::query("SELECT type, book_id FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_optional(pool)
        .await;

    match row {
        Ok(Some(row)) => (row.get("type"), row.get("book_id")),
        Ok(None) => ("unknown".to_string(), None),
        Err(err) => {
            warn!("[NOTIFY] Failed to read job {job_id}: {err}");
            ("unknown".to_string(), None)
        }
    }
}

/// `stats_json` decoded through the one shared schema; a missing or malformed
/// value is an empty run, never an error.
async fn load_stats(pool: &PgPool, job_id: Uuid) -> ScanStats {
    let row = sqlx::query("SELECT stats_json FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_optional(pool)
        .await;

    let value = match row {
        Ok(Some(row)) => row
            .try_get::<Option<serde_json::Value>, _>("stats_json")
            .ok()
            .flatten(),
        Ok(None) => None,
        Err(err) => {
            warn!("[NOTIFY] Failed to read stats of job {job_id}: {err}");
            None
        }
    };

    value
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

/// The books added by a scan, grouped by series (see `index_job_events`).
async fn load_discovered_series(pool: &PgPool, job_id: Uuid) -> Vec<SeriesDiscovered> {
    let rows = sqlx::query(
        "SELECT \
            COALESCE(s.name, 'Non classé') AS series_name, \
            b.title, \
            b.thumbnail_path \
         FROM index_job_events e \
         JOIN books b ON b.id = e.entity_id \
         LEFT JOIN series s ON s.id = b.series_id \
         WHERE e.job_id = $1 AND e.event_type = 'book_added' \
         ORDER BY COALESCE(s.name, 'Non classé'), b.volume NULLS LAST, b.title",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut groups: HashMap<String, SeriesDiscovered> = HashMap::new();
    for row in rows {
        let series_name: String = row.get("series_name");
        let title: String = row.get("title");
        let thumbnail_path: Option<String> = row.get("thumbnail_path");

        let group = groups
            .entry(series_name.clone())
            .or_insert_with(|| SeriesDiscovered {
                series_name,
                thumbnail_path: None,
                book_titles: Vec::new(),
            });

        if group.thumbnail_path.is_none() && thumbnail_path.is_some() {
            group.thumbnail_path = thumbnail_path;
        }
        group.book_titles.push(title);
    }

    let mut grouped: Vec<_> = groups.into_values().collect();
    grouped.sort_by(|a, b| a.series_name.cmp(&b.series_name));
    grouped
}

/// The events of a finished job — the whole policy, with no I/O.
fn events_for(finished: &JobFinished, mut facts: JobFacts) -> Vec<NotificationEvent> {
    let category = job_type_category(&facts.job_type);

    match &finished.outcome {
        JobOutcome::Cancelled => vec![NotificationEvent::ScanCancelled {
            job_type: facts.job_type,
            library_name: facts.library_name,
        }],
        JobOutcome::Failed { error } => vec![failed_event(category, facts, error.clone())],
        JobOutcome::Completed => {
            if category != JobCategory::Scan {
                return vec![completed_event(category, facts, finished.duration_seconds)];
            }

            let discovered = std::mem::take(&mut facts.series);

            // A scan that only discovered books speaks through the per-series
            // messages; the summary is for a silent run (nothing discovered) or a
            // noisy one (removals, errors). `is_noteworthy` still drops the
            // summary of a run where nothing at all happened.
            let wants_summary =
                discovered.is_empty() || facts.stats.removed_files > 0 || facts.stats.errors > 0;

            let mut events: Vec<_> = discovered
                .into_iter()
                .map(|series| NotificationEvent::ScanSeriesDiscovered {
                    library_name: facts.library_name.clone(),
                    series_name: series.series_name,
                    thumbnail_path: series.thumbnail_path,
                    book_titles: series.book_titles,
                })
                .collect();

            if wants_summary {
                events.push(completed_event(category, facts, finished.duration_seconds));
            }

            events
        }
    }
}

fn completed_event(
    category: JobCategory,
    facts: JobFacts,
    duration_seconds: u64,
) -> NotificationEvent {
    match category {
        JobCategory::Thumbnail => NotificationEvent::ThumbnailCompleted {
            job_type: facts.job_type,
            library_name: facts.library_name,
            duration_seconds,
        },
        JobCategory::Conversion => NotificationEvent::ConversionCompleted {
            library_name: facts.library_name,
            book_title: facts.book_title,
            thumbnail_path: facts.thumbnail_path,
        },
        _ => NotificationEvent::ScanCompleted {
            job_type: facts.job_type,
            library_name: facts.library_name,
            stats: facts.stats,
            duration_seconds,
        },
    }
}

fn failed_event(category: JobCategory, facts: JobFacts, error: String) -> NotificationEvent {
    match category {
        JobCategory::Thumbnail => NotificationEvent::ThumbnailFailed {
            job_type: facts.job_type,
            library_name: facts.library_name,
            error,
        },
        JobCategory::Conversion => NotificationEvent::ConversionFailed {
            library_name: facts.library_name,
            book_title: facts.book_title,
            thumbnail_path: facts.thumbnail_path,
            error,
        },
        _ => NotificationEvent::ScanFailed {
            job_type: facts.job_type,
            library_name: facts.library_name,
            error,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finished(outcome: JobOutcome) -> JobFinished {
        JobFinished {
            job_id: Uuid::nil(),
            library_id: None,
            outcome,
            duration_seconds: 12,
        }
    }

    fn facts(job_type: &str) -> JobFacts {
        JobFacts {
            job_type: job_type.to_string(),
            library_name: Some("Manga".to_string()),
            book_title: None,
            thumbnail_path: None,
            stats: ScanStats::default(),
            series: Vec::new(),
        }
    }

    fn series(name: &str, titles: &[&str]) -> SeriesDiscovered {
        SeriesDiscovered {
            series_name: name.to_string(),
            thumbnail_path: None,
            book_titles: titles.iter().map(|title| title.to_string()).collect(),
        }
    }

    #[test]
    fn completed_scan_speaks_through_its_series_messages() {
        let mut facts = facts("scan");
        facts.stats = ScanStats {
            indexed_files: 2,
            new_series: 1,
            ..ScanStats::default()
        };
        facts.series = vec![series("Dragon Ball", &["T01", "T02"])];

        let events = events_for(&finished(JobOutcome::Completed), facts);

        assert_eq!(
            events.len(),
            1,
            "no summary when every book has a series message"
        );
        assert!(matches!(
            &events[0],
            NotificationEvent::ScanSeriesDiscovered { series_name, book_titles, .. }
                if series_name == "Dragon Ball" && book_titles.len() == 2
        ));
    }

    #[test]
    fn completed_scan_with_nothing_discovered_sends_the_summary() {
        let events = events_for(&finished(JobOutcome::Completed), facts("scan"));

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            NotificationEvent::ScanCompleted { duration_seconds, .. } if *duration_seconds == 12
        ));
    }

    #[test]
    fn completed_scan_with_removals_sends_the_summary_too() {
        let mut facts = facts("scan");
        facts.stats.removed_files = 3;
        facts.series = vec![series("Dragon Ball", &["T01"])];

        let events = events_for(&finished(JobOutcome::Completed), facts);

        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            NotificationEvent::ScanSeriesDiscovered { .. }
        ));
        assert!(matches!(
            &events[1],
            NotificationEvent::ScanCompleted { .. }
        ));
    }

    #[test]
    fn completed_thumbnail_job_sends_one_message() {
        let events = events_for(&finished(JobOutcome::Completed), facts("thumbnail_rebuild"));

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            NotificationEvent::ThumbnailCompleted { job_type, .. } if job_type == "thumbnail_rebuild"
        ));
    }

    #[test]
    fn completed_conversion_sends_the_book() {
        let mut facts = facts("cbr_to_cbz");
        facts.book_title = Some("Tome 1".to_string());

        let events = events_for(&finished(JobOutcome::Completed), facts);

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            NotificationEvent::ConversionCompleted { book_title, .. }
                if book_title.as_deref() == Some("Tome 1")
        ));
    }

    #[test]
    fn failed_scan_sends_only_the_failure() {
        let mut facts = facts("scan");
        facts.series = vec![series("Dragon Ball", &["T01"])];

        let events = events_for(
            &finished(JobOutcome::Failed {
                error: "boom".to_string(),
            }),
            facts,
        );

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            NotificationEvent::ScanFailed { error, .. } if error == "boom"
        ));
    }

    #[test]
    fn failed_thumbnail_job_names_the_thumbnail_event() {
        let events = events_for(
            &finished(JobOutcome::Failed {
                error: "boom".to_string(),
            }),
            facts("thumbnail_regenerate"),
        );

        assert!(matches!(
            &events[0],
            NotificationEvent::ThumbnailFailed { .. }
        ));
    }

    #[test]
    fn cancelled_scan_sends_only_the_cancellation() {
        let mut facts = facts("scan");
        facts.series = vec![series("Dragon Ball", &["T01"])];

        let events = events_for(&finished(JobOutcome::Cancelled), facts);

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            NotificationEvent::ScanCancelled { job_type, .. } if job_type == "scan"
        ));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn job_events_reads_a_scan_from_the_database(pool: PgPool) {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind("Manga")
            .bind("/libraries/manga")
            .execute(&pool)
            .await
            .unwrap();

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind("Dragon Ball")
            .execute(&pool)
            .await
            .unwrap();

        let job_id = Uuid::new_v4();
        let stats = serde_json::json!({
            "scanned_files": 5,
            "indexed_files": 2,
            "removed_files": 0,
            "errors": 0,
            "warnings": 2,
            "new_series": 1,
            "new_series_names": ["Dragon Ball"],
            "new_book_titles": ["T01", "T02"],
        });
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, stats_json) \
             VALUES ($1, $2, 'scan', 'success', $3)",
        )
        .bind(job_id)
        .bind(library_id)
        .bind(&stats)
        .execute(&pool)
        .await
        .unwrap();

        for (volume, title) in [(1i32, "T01"), (2, "T02")] {
            let book_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO books (id, library_id, kind, title, series_id, volume) \
                 VALUES ($1, $2, 'comic', $3, $4, $5)",
            )
            .bind(book_id)
            .bind(library_id)
            .bind(title)
            .bind(series_id)
            .bind(volume)
            .execute(&pool)
            .await
            .unwrap();

            sqlx::query(
                "INSERT INTO index_job_events (job_id, event_type, entity_id) \
                 VALUES ($1, 'book_added', $2)",
            )
            .bind(job_id)
            .bind(book_id)
            .execute(&pool)
            .await
            .unwrap();
        }

        let events = job_events(
            &pool,
            &JobFinished {
                job_id,
                library_id: Some(library_id),
                outcome: JobOutcome::Completed,
                duration_seconds: 42,
            },
        )
        .await;

        assert_eq!(events.len(), 1, "the two books share one series message");
        match &events[0] {
            NotificationEvent::ScanSeriesDiscovered {
                library_name,
                series_name,
                book_titles,
                ..
            } => {
                assert_eq!(library_name.as_deref(), Some("Manga"));
                assert_eq!(series_name, "Dragon Ball");
                assert_eq!(book_titles, &["T01".to_string(), "T02".to_string()]);
            }
            _ => panic!("expected the series message"),
        }
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn job_events_keeps_the_warnings_of_the_summary(pool: PgPool) {
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, type, status, stats_json) \
             VALUES ($1, 'scan', 'success', $2)",
        )
        .bind(job_id)
        .bind(serde_json::json!({ "warnings": 4, "scanned_files": 10 }))
        .execute(&pool)
        .await
        .unwrap();

        let events = job_events(
            &pool,
            &JobFinished {
                job_id,
                library_id: None,
                outcome: JobOutcome::Completed,
                duration_seconds: 1,
            },
        )
        .await;

        assert_eq!(events.len(), 1);
        match &events[0] {
            NotificationEvent::ScanCompleted { stats, .. } => {
                assert_eq!(stats.warnings, 4);
                assert_eq!(stats.scanned_files, 10);
            }
            _ => panic!("expected the scan summary"),
        }
    }
}
