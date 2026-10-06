use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::handlers::{BookSyncReport, FieldChange, SeriesSyncReport, SyncReport};
use super::shared_sync::{self, is_field_locked, LinkMetadata};
use crate::{error::ApiError, metadata_providers};

// ---------------------------------------------------------------------------
// Report builders (shared by the multi-link sync path)
// ---------------------------------------------------------------------------

/// Build a series sync report from the pre-update row and the merged fields.
pub(crate) fn build_series_report(
    existing: Option<&sqlx::postgres::PgRow>,
    fields: &shared_sync::SeriesFields,
) -> SeriesSyncReport {
    let mut report = SeriesSyncReport::default();
    let locked = existing
        .map(|r| r.get::<serde_json::Value, _>("locked_fields"))
        .unwrap_or(serde_json::json!({}));

    let checks: Vec<(&str, Option<serde_json::Value>, Option<serde_json::Value>)> = vec![
        (
            "description",
            existing
                .and_then(|r| r.get::<Option<String>, _>("description"))
                .map(serde_json::Value::String),
            fields
                .description
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
        ),
        (
            "authors",
            existing.map(|r| serde_json::json!(r.get::<Vec<String>, _>("authors"))),
            if fields.authors.is_empty() {
                None
            } else {
                Some(serde_json::json!(fields.authors))
            },
        ),
        (
            "publishers",
            existing.map(|r| serde_json::json!(r.get::<Vec<String>, _>("publishers"))),
            if fields.publishers.is_empty() {
                None
            } else {
                Some(serde_json::json!(fields.publishers))
            },
        ),
        (
            "start_year",
            existing
                .and_then(|r| r.get::<Option<i32>, _>("start_year"))
                .map(|y| serde_json::json!(y)),
            fields.start_year.map(|y| serde_json::json!(y)),
        ),
        (
            "total_volumes",
            existing
                .and_then(|r| r.get::<Option<i32>, _>("total_volumes"))
                .map(|y| serde_json::json!(y)),
            fields.total_volumes.map(|y| serde_json::json!(y)),
        ),
        (
            "status",
            existing
                .and_then(|r| r.get::<Option<String>, _>("status"))
                .map(serde_json::Value::String),
            fields
                .status
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
        ),
    ];

    for (name, old, new) in checks {
        if new.is_none() {
            continue;
        }
        let change = FieldChange {
            field: name.to_string(),
            old_value: old.clone(),
            new_value: new.clone(),
        };
        if is_field_locked(&locked, name) {
            report.fields_skipped.push(change);
        } else if old != new {
            report.fields_updated.push(change);
        }
    }

    report
}

/// Build a per-book sync report from the pre-update row and the merged candidate.
pub(crate) fn build_book_report(
    book_id: Uuid,
    current: &sqlx::postgres::PgRow,
    ext_book: &metadata_providers::BookCandidate,
) -> Option<BookSyncReport> {
    let locked = current.get::<serde_json::Value, _>("locked_fields");
    let book_title: String = current.get("title");
    let mut fields_updated = Vec::new();
    let mut fields_skipped = Vec::new();

    let field_checks: Vec<(&str, Option<serde_json::Value>, Option<serde_json::Value>)> = vec![
        (
            "summary",
            current
                .get::<Option<String>, _>("summary")
                .map(|s| serde_json::json!(s)),
            ext_book.summary.as_ref().map(|s| serde_json::json!(s)),
        ),
        (
            "isbn",
            current
                .get::<Option<String>, _>("isbn")
                .map(|s| serde_json::json!(s)),
            ext_book.isbn.as_ref().map(|s| serde_json::json!(s)),
        ),
        (
            "publish_date",
            current
                .get::<Option<String>, _>("publish_date")
                .map(|s| serde_json::json!(s)),
            ext_book.publish_date.as_ref().map(|s| serde_json::json!(s)),
        ),
        (
            "language",
            current
                .get::<Option<String>, _>("language")
                .map(|s| serde_json::json!(s)),
            ext_book.language.as_ref().map(|s| serde_json::json!(s)),
        ),
        (
            "authors",
            Some(serde_json::json!(current.get::<Vec<String>, _>("authors"))),
            if ext_book.authors.is_empty() {
                None
            } else {
                Some(serde_json::json!(&ext_book.authors))
            },
        ),
    ];

    for (name, old, new) in field_checks {
        if new.is_none() {
            continue;
        }
        let change = FieldChange {
            field: name.to_string(),
            old_value: old.clone(),
            new_value: new.clone(),
        };
        if is_field_locked(&locked, name) {
            fields_skipped.push(change);
        } else if old != new {
            fields_updated.push(change);
        }
    }

    if fields_updated.is_empty() && fields_skipped.is_empty() {
        None
    } else {
        Some(BookSyncReport {
            book_id,
            title: book_title,
            volume: ext_book.volume_number,
            fields_updated,
            fields_skipped,
        })
    }
}

// ---------------------------------------------------------------------------
// Multi-link sync (primary + secondary fallback)
// ---------------------------------------------------------------------------

/// Sync a series from all its approved links: merge series fields (primary
/// first, secondaries as fallback) and/or merge book metadata across links.
///
/// This is the single sync entry point for approve / change-primary: it reads
/// every approved link, so the result is independent of which link triggered it.
pub(crate) async fn sync_series_from_links(
    pool: &PgPool,
    series_id: Uuid,
    sync_series: bool,
    sync_books: bool,
) -> Result<SyncReport, ApiError> {
    let series_row = sqlx::query("SELECT library_id, name FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("series not found"))?;
    let library_id: Uuid = series_row.get("library_id");
    let series_name: String = series_row.get("name");

    let links = shared_sync::fetch_approved_links(pool, series_id).await?;

    let mut report = SyncReport::default();

    if sync_series {
        let mut fields = shared_sync::merge_series_fields(&links);
        if let Some(raw) = fields.status.clone() {
            fields.status = Some(super::handlers::normalize_series_status(pool, &raw).await);
        }
        let existing =
            shared_sync::upsert_series_metadata(pool, library_id, &series_name, &fields).await?;
        report.series = Some(build_series_report(existing.as_ref(), &fields));
    }

    if sync_books {
        let (matched, book_reports, unmatched) =
            sync_books_from_links(pool, series_id, library_id, &series_name, &links).await?;
        report.books_matched = matched;
        report.books = book_reports;
        report.books_unmatched = unmatched;

        if matched == 0 && unmatched == 0 {
            report.books_message = Some(
                "This provider does not have volume-level data for this series. \
                 Series metadata was synced, but book matching is not available."
                    .to_string(),
            );
        }

        for link in &links {
            shared_sync::update_link_synced_at(pool, link.id).await?;
        }
    }

    Ok(report)
}

/// Fetch provider books for every approved link, match them, persist the
/// external rows, then push the merged (primary-first) metadata to local books.
async fn sync_books_from_links(
    pool: &PgPool,
    series_id: Uuid,
    library_id: Uuid,
    series_name: &str,
    links: &[LinkMetadata],
) -> Result<(i64, Vec<BookSyncReport>, i64), ApiError> {
    let mut matched_total: i64 = 0;
    let mut unmatched_total: i64 = 0;

    let local_books = shared_sync::fetch_local_books(pool, library_id, series_name).await?;

    for link in links {
        let provider = metadata_providers::get_provider(&link.provider)
            .or_else(|| metadata_providers::get_provider("google_books"))
            .ok_or_else(|| ApiError::internal(format!("unknown provider: {}", link.provider)))?;
        let provider_config = super::config::load_provider_config(pool, &link.provider).await;

        let books = provider
            .get_series_books(&link.external_id, &provider_config)
            .await
            .map_err(|e| ApiError::internal(format!("provider error: {e}")))?;

        shared_sync::delete_link_book_metadata(pool, link.id).await?;

        let matched = if link.provider == "bdtheque" || link.provider == "bdphile" {
            shared_sync::match_books_for_new_provider(&books, &local_books)
        } else {
            shared_sync::match_books(&books, &local_books)
        };

        let mut matched_count: i64 = 0;
        for m in &matched {
            shared_sync::insert_external_book_metadata(pool, link.id, m.local_book_id, m.ext_book)
                .await?;
            if m.local_book_id.is_some() {
                matched_count += 1;
            }
        }

        matched_total += matched_count;
        unmatched_total += books.len() as i64 - matched_count;
    }

    // Push merged metadata across all approved links (primary first).
    let rows = shared_sync::fetch_approved_external_books(pool, series_id).await?;
    let merged = shared_sync::merge_book_rows(&rows);

    let mut book_reports = Vec::new();
    for (book_id, candidate) in &merged {
        let current = shared_sync::push_book_metadata(pool, *book_id, candidate).await?;
        if let Some(report) = build_book_report(*book_id, &current, candidate) {
            book_reports.push(report);
        }
    }

    Ok((matched_total, book_reports, unmatched_total))
}

#[cfg(test)]
#[path = "tests/sync.rs"]
mod sync_tests;
