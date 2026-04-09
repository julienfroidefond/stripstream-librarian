use axum::{
    extract::{Path as AxumPath, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{error::ApiError, metadata_providers, state::AppState};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize, ToSchema)]
pub struct MetadataSearchRequest {
    pub library_id: String,
    pub series_name: String,
    /// Optional provider override (defaults to library/global setting)
    pub provider: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct SeriesCandidateDto {
    pub provider: String,
    pub external_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub publishers: Vec<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    pub cover_url: Option<String>,
    pub external_url: Option<String>,
    pub confidence: f32,
    pub metadata_json: serde_json::Value,
}

#[derive(Deserialize, ToSchema)]
pub struct MetadataMatchRequest {
    pub library_id: String,
    pub series_name: String,
    pub provider: String,
    pub external_id: String,
    pub external_url: Option<String>,
    pub confidence: Option<f32>,
    #[allow(dead_code)]
    pub title: String,
    pub metadata_json: serde_json::Value,
    pub total_volumes: Option<i32>,
}

#[derive(Serialize, ToSchema)]
pub struct ExternalMetadataLinkDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub series_name: String,
    pub provider: String,
    pub external_id: String,
    pub external_url: Option<String>,
    pub status: String,
    pub confidence: Option<f32>,
    pub metadata_json: serde_json::Value,
    pub total_volumes_external: Option<i32>,
    pub matched_at: String,
    pub approved_at: Option<String>,
    pub synced_at: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ApproveRequest {
    #[serde(default)]
    pub sync_series: bool,
    #[serde(default)]
    pub sync_books: bool,
}

#[derive(Serialize, ToSchema)]
pub struct FieldChange {
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<serde_json::Value>,
}

#[derive(Serialize, ToSchema, Default)]
pub struct SeriesSyncReport {
    pub fields_updated: Vec<FieldChange>,
    pub fields_skipped: Vec<FieldChange>,
}

#[derive(Serialize, ToSchema)]
pub struct BookSyncReport {
    #[schema(value_type = String)]
    pub book_id: Uuid,
    pub title: String,
    pub volume: Option<i32>,
    pub fields_updated: Vec<FieldChange>,
    pub fields_skipped: Vec<FieldChange>,
}

#[derive(Serialize, ToSchema, Default)]
pub struct SyncReport {
    pub series: Option<SeriesSyncReport>,
    pub books: Vec<BookSyncReport>,
    pub books_matched: i64,
    pub books_unmatched: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub books_message: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ApproveResponse {
    pub status: String,
    pub report: SyncReport,
}

#[derive(Serialize, ToSchema)]
pub struct MissingBooksDto {
    pub total_external: i64,
    pub total_local: i64,
    pub missing_count: i64,
    pub missing_books: Vec<MissingBookItem>,
}

#[derive(Serialize, ToSchema)]
pub struct MissingBookItem {
    pub title: Option<String>,
    pub volume_number: Option<i32>,
    pub external_book_id: Option<String>,
}

#[derive(Deserialize)]
pub struct MetadataLinkQuery {
    pub library_id: Option<String>,
    pub series_id: Option<String>,
}

// ---------------------------------------------------------------------------
// POST /metadata/search
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/metadata/search",
    tag = "metadata",
    request_body = MetadataSearchRequest,
    responses(
        (status = 200, body = Vec<SeriesCandidateDto>),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Provider error"),
    ),
    security(("Bearer" = []))
)]
pub async fn search_metadata(
    State(state): State<AppState>,
    Json(body): Json<MetadataSearchRequest>,
) -> Result<Json<Vec<SeriesCandidateDto>>, ApiError> {
    let library_id: Uuid = body
        .library_id
        .parse()
        .map_err(|_| ApiError::bad_request("invalid library_id"))?;

    if body.series_name.trim().is_empty() {
        return Err(ApiError::bad_request("series_name is required"));
    }

    // Determine provider: explicit override -> library-level -> global setting -> default
    let provider_name = if let Some(ref p) = body.provider {
        if !p.is_empty() { p.clone() } else { get_provider_for_library(&state, library_id).await? }
    } else {
        get_provider_for_library(&state, library_id).await?
    };

    // Fall back to google_books if the configured provider isn't implemented yet
    let provider = metadata_providers::get_provider(&provider_name)
        .or_else(|| metadata_providers::get_provider("google_books"))
        .ok_or_else(|| ApiError::bad_request(format!("unknown provider: {provider_name}")))?;

    let mut provider_config = super::config::load_provider_config(&state.pool, &provider_name).await;
    provider_config.detailed = true; // Manual search: show per-edition results

    let mut candidates = provider
        .search_series(&body.series_name, &provider_config)
        .await
        .map_err(|e| ApiError::internal(format!("provider error: {e}")))?;

    // Boost confidence based on local book count vs candidate total_volumes
    let local_count: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books b \
         JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 AND LOWER(unaccent(s.name)) = LOWER(unaccent($2)) \
         AND b.volume_type = 'regular'",
    )
    .bind(library_id)
    .bind(&body.series_name)
    .fetch_one(&state.pool)
    .await
    .ok();

    if let Some(count) = local_count {
        super::config::boost_confidence_by_book_count(&mut candidates, count);
    }

    let actual_provider = provider.name().to_string();
    let dtos: Vec<SeriesCandidateDto> = candidates
        .into_iter()
        .map(|c| SeriesCandidateDto {
            provider: actual_provider.clone(),
            external_id: c.external_id,
            title: c.title,
            authors: c.authors,
            description: c.description,
            publishers: c.publishers,
            start_year: c.start_year,
            total_volumes: c.total_volumes,
            cover_url: c.cover_url,
            external_url: c.external_url,
            confidence: c.confidence,
            metadata_json: c.metadata_json,
        })
        .collect();

    Ok(Json(dtos))
}

// ---------------------------------------------------------------------------
// POST /metadata/match
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/metadata/match",
    tag = "metadata",
    request_body = MetadataMatchRequest,
    responses(
        (status = 200, body = ExternalMetadataLinkDto),
        (status = 400, description = "Bad request"),
    ),
    security(("Bearer" = []))
)]
pub async fn create_metadata_match(
    State(state): State<AppState>,
    Json(body): Json<MetadataMatchRequest>,
) -> Result<Json<ExternalMetadataLinkDto>, ApiError> {
    let library_id: Uuid = body
        .library_id
        .parse()
        .map_err(|_| ApiError::bad_request("invalid library_id"))?;

    let series_id = crate::series::get_or_create_series(&state.pool, library_id, &body.series_name).await?;

    let row = sqlx::query(
        r#"
        INSERT INTO external_metadata_links
            (library_id, series_id, provider, external_id, external_url, status, confidence, metadata_json, total_volumes_external)
        VALUES ($1, $2, $3, $4, $5, 'pending', $6, $7, $8)
        ON CONFLICT (series_id, provider)
        DO UPDATE SET
            external_id = EXCLUDED.external_id,
            external_url = EXCLUDED.external_url,
            status = 'pending',
            confidence = EXCLUDED.confidence,
            metadata_json = EXCLUDED.metadata_json,
            total_volumes_external = EXCLUDED.total_volumes_external,
            matched_at = NOW(),
            updated_at = NOW(),
            approved_at = NULL,
            synced_at = NULL
        RETURNING id
        "#,
    )
    .bind(library_id)
    .bind(series_id)
    .bind(&body.provider)
    .bind(&body.external_id)
    .bind(&body.external_url)
    .bind(body.confidence)
    .bind(&body.metadata_json)
    .bind(body.total_volumes)
    .fetch_one(&state.pool)
    .await?;

    let link_id: Uuid = row.get("id");
    // Re-fetch with JOIN to get series_name for the DTO
    let full_row = sqlx::query(
        r#"
        SELECT eml.id, eml.library_id, s.name AS series_name, eml.series_id, eml.provider, eml.external_id, eml.external_url, eml.status, eml.confidence,
               eml.metadata_json, eml.total_volumes_external, eml.matched_at, eml.approved_at, eml.synced_at
        FROM external_metadata_links eml
        JOIN series s ON s.id = eml.series_id
        WHERE eml.id = $1
        "#,
    )
    .bind(link_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(row_to_link_dto(&full_row)))
}

// ---------------------------------------------------------------------------
// POST /metadata/approve/:id
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/metadata/approve/{id}",
    tag = "metadata",
    params(("id" = String, Path, description = "Link UUID")),
    request_body = ApproveRequest,
    responses(
        (status = 200, body = ApproveResponse),
        (status = 404, description = "Link not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn approve_metadata(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
    Json(body): Json<ApproveRequest>,
) -> Result<Json<ApproveResponse>, ApiError> {
    // Update status to approved
    let result = sqlx::query(
        r#"
        UPDATE external_metadata_links
        SET status = 'approved', approved_at = NOW(), updated_at = NOW()
        WHERE id = $1
        RETURNING library_id, series_id, provider, external_id, metadata_json, total_volumes_external
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let row = result.ok_or_else(|| ApiError::not_found("link not found"))?;

    let library_id: Uuid = row.get("library_id");
    let series_id: Uuid = row.get("series_id");
    let series_name: String = sqlx::query_scalar("SELECT name FROM series WHERE id = $1")
        .bind(series_id).fetch_one(&state.pool).await?;

    // Reject any other approved links for the same series (only one active link per series)
    // Also clean up their external_book_metadata
    let old_link_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        UPDATE external_metadata_links
        SET status = 'rejected', updated_at = NOW()
        WHERE series_id = $1 AND id != $2 AND status = 'approved'
        RETURNING id
        "#,
    )
    .bind(series_id)
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    if !old_link_ids.is_empty() {
        sqlx::query("DELETE FROM external_book_metadata WHERE link_id = ANY($1)")
            .bind(&old_link_ids)
            .execute(&state.pool)
            .await?;
    }

    let provider_name: String = row.get("provider");
    let external_id: String = row.get("external_id");
    let metadata_json: serde_json::Value = row.get("metadata_json");
    let total_volumes_external: Option<i32> = row.get("total_volumes_external");

    let mut report = SyncReport::default();

    // Sync series metadata if requested
    if body.sync_series {
        report.series = Some(
            sync_series_metadata(&state, library_id, &series_name, &metadata_json, total_volumes_external).await?
        );
    }

    // Sync books if requested
    if body.sync_books {
        let (matched, book_reports, unmatched) =
            sync_books_metadata(&state, id, library_id, &series_name, &provider_name, &external_id)
                .await?;
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

        // Update synced_at
        sqlx::query("UPDATE external_metadata_links SET synced_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await?;
    }

    // Notify via Telegram (with first book thumbnail if available)
    let provider_for_notif: String = row.get("provider");
    let thumbnail_path: Option<String> = sqlx::query_scalar(
        "SELECT b.thumbnail_path FROM books b JOIN series s ON s.id = b.series_id WHERE b.library_id = $1 AND s.name = $2 AND b.thumbnail_path IS NOT NULL ORDER BY b.volume NULLS LAST, b.title LIMIT 1",
    )
    .bind(library_id)
    .bind(&series_name)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    // Collect notification data from the report
    let notif_fields: Vec<String> = report
        .series
        .as_ref()
        .map(|s| s.fields_updated.iter().map(|f| f.field.clone()).collect())
        .unwrap_or_default();
    let notif_books_matched = report.books_matched as usize;
    let notif_books_updated = report
        .books
        .iter()
        .filter(|b| !b.fields_updated.is_empty())
        .count();

    notifications::notify(
        state.pool.clone(),
        notifications::NotificationEvent::MetadataApproved {
            series_name: series_name.clone(),
            provider: provider_for_notif,
            thumbnail_path,
            fields_updated: notif_fields,
            books_matched: notif_books_matched,
            books_updated: notif_books_updated,
        },
    );

    Ok(Json(ApproveResponse {
        status: "approved".to_string(),
        report,
    }))
}

// ---------------------------------------------------------------------------
// POST /metadata/reject/:id
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/metadata/reject/{id}",
    tag = "metadata",
    params(("id" = String, Path, description = "Link UUID")),
    responses(
        (status = 200, description = "Rejected"),
        (status = 404, description = "Link not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn reject_metadata(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<crate::responses::StatusResponse>, ApiError> {
    let result = sqlx::query(
        "UPDATE external_metadata_links SET status = 'rejected', updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("link not found"));
    }

    Ok(Json(crate::responses::StatusResponse::new("rejected")))
}

// ---------------------------------------------------------------------------
// GET /metadata/links
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/metadata/links",
    tag = "metadata",
    params(
        ("library_id" = Option<String>, Query, description = "Library UUID"),
        ("series_id" = Option<String>, Query, description = "Series UUID"),
    ),
    responses(
        (status = 200, body = Vec<ExternalMetadataLinkDto>),
    ),
    security(("Bearer" = []))
)]
pub async fn get_metadata_links(
    State(state): State<AppState>,
    Query(query): Query<MetadataLinkQuery>,
) -> Result<Json<Vec<ExternalMetadataLinkDto>>, ApiError> {
    let library_id: Option<Uuid> = query
        .library_id
        .as_deref()
        .and_then(|s| s.parse().ok());

    let series_id: Option<Uuid> = query.series_id.as_deref().and_then(|s| s.parse().ok());

    let rows = sqlx::query(
        r#"
        SELECT eml.id, eml.library_id, s.name AS series_name, eml.series_id, eml.provider, eml.external_id, eml.external_url, eml.status, eml.confidence,
               eml.metadata_json, eml.total_volumes_external, eml.matched_at, eml.approved_at, eml.synced_at
        FROM external_metadata_links eml
        JOIN series s ON s.id = eml.series_id
        WHERE ($1::uuid IS NULL OR eml.library_id = $1)
          AND ($2::uuid IS NULL OR eml.series_id = $2)
        ORDER BY eml.updated_at DESC
        "#,
    )
    .bind(library_id)
    .bind(series_id)
    .fetch_all(&state.pool)
    .await?;

    let links: Vec<ExternalMetadataLinkDto> = rows.iter().map(row_to_link_dto).collect();

    Ok(Json(links))
}

// ---------------------------------------------------------------------------
// GET /metadata/missing/:id
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/metadata/missing/{id}",
    tag = "metadata",
    params(("id" = String, Path, description = "Link UUID")),
    responses(
        (status = 200, body = MissingBooksDto),
        (status = 404, description = "Link not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_missing_books(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<MissingBooksDto>, ApiError> {
    // Verify link exists
    let link = sqlx::query(
        "SELECT eml.series_id FROM external_metadata_links eml WHERE eml.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("link not found"))?;

    let series_id: Uuid = link.get("series_id");

    // Count external books from provider
    let provider_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_book_metadata WHERE link_id = $1")
            .bind(id)
            .fetch_one(&state.pool)
            .await?;

    // Use series.total_volumes if set (user override), otherwise fall back to provider count
    let series_total: Option<i32> = sqlx::query_scalar(
        "SELECT total_volumes FROM series WHERE id = $1",
    )
    .bind(series_id)
    .fetch_one(&state.pool)
    .await?;

    let total_external = series_total
        .filter(|&v| v > 0)
        .map(|v| v as i64)
        .unwrap_or(provider_count);

    // Count local books (only regular volumes — HS/oneshot are not part of the numbering)
    let total_local: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books WHERE series_id = $1 AND volume_type = 'regular'",
    )
    .bind(series_id)
    .fetch_one(&state.pool)
    .await?;

    // Get unmatched external books (no book_id link)
    let missing_rows = sqlx::query(
        r#"
        SELECT title, volume_number, external_book_id
        FROM external_book_metadata
        WHERE link_id = $1 AND book_id IS NULL
        ORDER BY volume_number NULLS LAST
        "#,
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let missing_books: Vec<MissingBookItem> = missing_rows
        .iter()
        .map(|row| MissingBookItem {
            title: row.get("title"),
            volume_number: row.get("volume_number"),
            external_book_id: row.get("external_book_id"),
        })
        .collect();

    // missing_count: use total_external - local count (respects user override),
    // but cap at 0 (no negatives)
    let missing_count = (total_external - total_local).max(0);

    Ok(Json(MissingBooksDto {
        total_external,
        total_local,
        missing_count,
        missing_books,
    }))
}

// ---------------------------------------------------------------------------
// DELETE /metadata/links/:id
// ---------------------------------------------------------------------------

#[utoipa::path(
    delete,
    path = "/metadata/links/{id}",
    tag = "metadata",
    params(("id" = String, Path, description = "Link UUID")),
    responses(
        (status = 200, description = "Deleted"),
        (status = 404, description = "Link not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_metadata_link(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<crate::responses::DeletedResponse>, ApiError> {
    let result = sqlx::query("DELETE FROM external_metadata_links WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("link not found"));
    }

    Ok(Json(crate::responses::DeletedResponse::new(id)))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn row_to_link_dto(row: &sqlx::postgres::PgRow) -> ExternalMetadataLinkDto {
    let matched_at: chrono::DateTime<chrono::Utc> = row.get("matched_at");
    let approved_at: Option<chrono::DateTime<chrono::Utc>> = row.get("approved_at");
    let synced_at: Option<chrono::DateTime<chrono::Utc>> = row.get("synced_at");

    ExternalMetadataLinkDto {
        id: row.get("id"),
        library_id: row.get("library_id"),
        series_name: row.get("series_name"),
        provider: row.get("provider"),
        external_id: row.get("external_id"),
        external_url: row.get("external_url"),
        status: row.get("status"),
        confidence: row.get("confidence"),
        metadata_json: row.get("metadata_json"),
        total_volumes_external: row.get("total_volumes_external"),
        matched_at: matched_at.to_rfc3339(),
        approved_at: approved_at.map(|d| d.to_rfc3339()),
        synced_at: synced_at.map(|d| d.to_rfc3339()),
    }
}

pub(crate) async fn get_provider_for_library(state: &AppState, library_id: Uuid) -> Result<String, ApiError> {
    // Check library-level provider first
    let row = sqlx::query("SELECT metadata_provider FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("library not found"))?;

    let lib_provider: Option<String> = row.get("metadata_provider");
    if let Some(p) = lib_provider {
        if !p.is_empty() {
            return Ok(p);
        }
    }

    // Fall back to global setting
    let global = sqlx::query("SELECT value FROM app_settings WHERE key = 'metadata_providers'")
        .fetch_optional(&state.pool)
        .await?;

    if let Some(row) = global {
        let value: serde_json::Value = row.get("value");
        if let Some(default) = value.get("default_provider").and_then(|v| v.as_str()) {
            if !default.is_empty() {
                return Ok(default.to_string());
            }
        }
    }

    // Default to google_books
    Ok("google_books".to_string())
}

// Provider config loading is in super::config::load_provider_config
// sync_series_metadata and sync_books_metadata are in super::sync

pub(crate) use super::sync::{sync_series_metadata, sync_books_metadata};

/// Normalize provider-specific status strings using the status_mappings table.
/// Returns None if no mapping is found -- unknown statuses are not stored.
pub(crate) async fn normalize_series_status(pool: &sqlx::PgPool, raw: &str) -> String {
    let lower = raw.to_lowercase();

    // Try exact match first (only mapped entries)
    if let Ok(Some(row)) = sqlx::query_scalar::<_, String>(
        "SELECT mapped_status FROM status_mappings WHERE provider_status = $1 AND mapped_status IS NOT NULL",
    )
    .bind(&lower)
    .fetch_optional(pool)
    .await
    {
        return row;
    }

    // Try substring match (for Bedetheque-style statuses like "Serie finie")
    if let Ok(Some(row)) = sqlx::query_scalar::<_, String>(
        "SELECT mapped_status FROM status_mappings WHERE $1 LIKE '%' || provider_status || '%' AND mapped_status IS NOT NULL LIMIT 1",
    )
    .bind(&lower)
    .fetch_optional(pool)
    .await
    {
        return row;
    }

    // No mapping found -- return the provider status as-is (lowercased)
    lower
}


/// Check if a field is locked based on the locked_fields JSON object.
/// Extracted for testability.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn is_field_locked(locked_fields: &serde_json::Value, field: &str) -> bool {
    locked_fields
        .get(field)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Build a FieldChange and classify it as updated or skipped based on lock status.
/// Returns (is_skipped, change). Returns None if new_value is None (nothing to sync).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn classify_field_change(
    field: &str,
    old_value: Option<serde_json::Value>,
    new_value: Option<serde_json::Value>,
    locked_fields: &serde_json::Value,
) -> Option<(bool, FieldChange)> {
    let new_value = new_value?;
    let change = FieldChange {
        field: field.to_string(),
        old_value: old_value.clone(),
        new_value: Some(new_value.clone()),
    };
    if is_field_locked(locked_fields, field) {
        Some((true, change)) // skipped
    } else if old_value.as_ref() != Some(&new_value) {
        Some((false, change)) // updated
    } else {
        None // no change
    }
}

/// Extract authors from metadata JSON as Vec<String>.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn extract_string_array(metadata: &serde_json::Value, key: &str) -> Vec<String> {
    metadata
        .get(key)
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/handlers.rs"]
mod tests;
