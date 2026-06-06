use sqlx::Row;
use uuid::Uuid;

use super::handlers::{BookSyncReport, FieldChange, SeriesSyncReport};
use super::shared_sync::{self, is_field_locked};
use crate::{error::ApiError, metadata_providers, state::AppState};

pub(crate) async fn sync_series_metadata(
    state: &AppState,
    library_id: Uuid,
    series_name: &str,
    metadata_json: &serde_json::Value,
    total_volumes: Option<i32>,
) -> Result<SeriesSyncReport, ApiError> {
    let fields =
        shared_sync::extract_series_fields(&state.pool, metadata_json, None, total_volumes).await;

    let existing =
        shared_sync::upsert_series_metadata(&state.pool, library_id, series_name, &fields).await?;

    // Build report from pre-update state
    let mut report = SeriesSyncReport::default();
    let locked = existing
        .as_ref()
        .map(|r| r.get::<serde_json::Value, _>("locked_fields"))
        .unwrap_or(serde_json::json!({}));

    let checks: Vec<(&str, Option<serde_json::Value>, Option<serde_json::Value>)> = vec![
        (
            "description",
            existing
                .as_ref()
                .and_then(|r| r.get::<Option<String>, _>("description"))
                .map(serde_json::Value::String),
            fields
                .description
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
        ),
        (
            "authors",
            existing
                .as_ref()
                .map(|r| serde_json::json!(r.get::<Vec<String>, _>("authors"))),
            if fields.authors.is_empty() {
                None
            } else {
                Some(serde_json::json!(fields.authors))
            },
        ),
        (
            "publishers",
            existing
                .as_ref()
                .map(|r| serde_json::json!(r.get::<Vec<String>, _>("publishers"))),
            if fields.publishers.is_empty() {
                None
            } else {
                Some(serde_json::json!(fields.publishers))
            },
        ),
        (
            "start_year",
            existing
                .as_ref()
                .and_then(|r| r.get::<Option<i32>, _>("start_year"))
                .map(|y| serde_json::json!(y)),
            fields.start_year.map(|y| serde_json::json!(y)),
        ),
        (
            "total_volumes",
            existing
                .as_ref()
                .and_then(|r| r.get::<Option<i32>, _>("total_volumes"))
                .map(|y| serde_json::json!(y)),
            fields.total_volumes.map(|y| serde_json::json!(y)),
        ),
        (
            "status",
            existing
                .as_ref()
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

    Ok(report)
}

pub(crate) async fn sync_books_metadata(
    state: &AppState,
    link_id: Uuid,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    external_id: &str,
) -> Result<(i64, Vec<BookSyncReport>, i64), ApiError> {
    let provider = metadata_providers::get_provider(provider_name)
        .or_else(|| metadata_providers::get_provider("google_books"))
        .ok_or_else(|| ApiError::internal(format!("unknown provider: {provider_name}")))?;

    let provider_config = super::config::load_provider_config(&state.pool, provider_name).await;

    let books = provider
        .get_series_books(external_id, &provider_config)
        .await
        .map_err(|e| ApiError::internal(format!("provider error: {e}")))?;

    shared_sync::delete_link_book_metadata(&state.pool, link_id).await?;

    let local_books = shared_sync::fetch_local_books(&state.pool, library_id, series_name).await?;
    let matched = shared_sync::match_books(&books, &local_books);

    let mut matched_count: i64 = 0;
    let mut book_reports: Vec<BookSyncReport> = Vec::new();

    for m in &matched {
        shared_sync::insert_external_book_metadata(
            &state.pool,
            link_id,
            m.local_book_id,
            m.ext_book,
        )
        .await?;

        if let Some(book_id) = m.local_book_id {
            let current = shared_sync::push_book_metadata(&state.pool, book_id, m.ext_book).await?;

            // Build per-book report
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
                    m.ext_book.summary.as_ref().map(|s| serde_json::json!(s)),
                ),
                (
                    "isbn",
                    current
                        .get::<Option<String>, _>("isbn")
                        .map(|s| serde_json::json!(s)),
                    m.ext_book.isbn.as_ref().map(|s| serde_json::json!(s)),
                ),
                (
                    "publish_date",
                    current
                        .get::<Option<String>, _>("publish_date")
                        .map(|s| serde_json::json!(s)),
                    m.ext_book
                        .publish_date
                        .as_ref()
                        .map(|s| serde_json::json!(s)),
                ),
                (
                    "language",
                    current
                        .get::<Option<String>, _>("language")
                        .map(|s| serde_json::json!(s)),
                    m.ext_book.language.as_ref().map(|s| serde_json::json!(s)),
                ),
                (
                    "authors",
                    Some(serde_json::json!(current.get::<Vec<String>, _>("authors"))),
                    if m.ext_book.authors.is_empty() {
                        None
                    } else {
                        Some(serde_json::json!(&m.ext_book.authors))
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

            if !fields_updated.is_empty() || !fields_skipped.is_empty() {
                book_reports.push(BookSyncReport {
                    book_id,
                    title: book_title,
                    volume: m.ext_book.volume_number,
                    fields_updated,
                    fields_skipped,
                });
            }

            matched_count += 1;
        }
    }

    let unmatched = books.len() as i64 - matched_count;
    Ok((matched_count, book_reports, unmatched))
}

#[cfg(test)]
#[path = "tests/sync.rs"]
mod sync_tests;
