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
#[allow(dead_code)]
pub struct MetadataMatchRequest {
    pub library_id: String,
    pub series_name: String,
    pub provider: String,
    pub external_id: String,
    pub external_url: Option<String>,
    pub confidence: Option<f32>,
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
    pub series_name: Option<String>,
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

    // Determine provider: explicit override → library-level → global setting → default
    let provider_name = if let Some(ref p) = body.provider {
        if !p.is_empty() { p.clone() } else { get_provider_for_library(&state, library_id).await? }
    } else {
        get_provider_for_library(&state, library_id).await?
    };

    // Fall back to google_books if the configured provider isn't implemented yet
    let provider = metadata_providers::get_provider(&provider_name)
        .or_else(|| metadata_providers::get_provider("google_books"))
        .ok_or_else(|| ApiError::bad_request(format!("unknown provider: {provider_name}")))?;

    let provider_config = load_provider_config(&state, &provider_name).await;

    let candidates = provider
        .search_series(&body.series_name, &provider_config)
        .await
        .map_err(|e| ApiError::internal(format!("provider error: {e}")))?;

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

    let row = sqlx::query(
        r#"
        INSERT INTO external_metadata_links
            (library_id, series_name, provider, external_id, external_url, status, confidence, metadata_json, total_volumes_external)
        VALUES ($1, $2, $3, $4, $5, 'pending', $6, $7, $8)
        ON CONFLICT (library_id, series_name, provider)
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
        RETURNING id, library_id, series_name, provider, external_id, external_url, status, confidence,
                  metadata_json, total_volumes_external, matched_at, approved_at, synced_at
        "#,
    )
    .bind(library_id)
    .bind(&body.series_name)
    .bind(&body.provider)
    .bind(&body.external_id)
    .bind(&body.external_url)
    .bind(body.confidence)
    .bind(&body.metadata_json)
    .bind(body.total_volumes)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(row_to_link_dto(&row)))
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
        RETURNING library_id, series_name, provider, external_id, metadata_json, total_volumes_external
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let row = result.ok_or_else(|| ApiError::not_found("link not found"))?;

    let library_id: Uuid = row.get("library_id");
    let series_name: String = row.get("series_name");

    // Reject any other approved links for the same series (only one active link per series)
    // Also clean up their external_book_metadata
    let old_link_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        UPDATE external_metadata_links
        SET status = 'rejected', updated_at = NOW()
        WHERE library_id = $1 AND series_name = $2 AND id != $3 AND status = 'approved'
        RETURNING id
        "#,
    )
    .bind(library_id)
    .bind(&series_name)
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
        "SELECT thumbnail_path FROM books WHERE library_id = $1 AND series_name = $2 AND thumbnail_path IS NOT NULL ORDER BY sort_order LIMIT 1",
    )
    .bind(library_id)
    .bind(&series_name)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    notifications::notify(
        state.pool.clone(),
        notifications::NotificationEvent::MetadataApproved {
            series_name: series_name.clone(),
            provider: provider_for_notif,
            thumbnail_path,
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
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query(
        "UPDATE external_metadata_links SET status = 'rejected', updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("link not found"));
    }

    Ok(Json(serde_json::json!({"status": "rejected"})))
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
        ("series_name" = Option<String>, Query, description = "Series name"),
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

    let rows = sqlx::query(
        r#"
        SELECT id, library_id, series_name, provider, external_id, external_url, status, confidence,
               metadata_json, total_volumes_external, matched_at, approved_at, synced_at
        FROM external_metadata_links
        WHERE ($1::uuid IS NULL OR library_id = $1)
          AND ($2::text IS NULL OR series_name = $2)
        ORDER BY updated_at DESC
        "#,
    )
    .bind(library_id)
    .bind(query.series_name.as_deref())
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
        "SELECT library_id, series_name FROM external_metadata_links WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("link not found"))?;

    let library_id: Uuid = link.get("library_id");
    let series_name: String = link.get("series_name");

    // Count external books
    let total_external: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_book_metadata WHERE link_id = $1")
            .bind(id)
            .fetch_one(&state.pool)
            .await?;

    // Count local books
    let total_local: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books WHERE library_id = $1 AND COALESCE(NULLIF(series, ''), 'unclassified') = $2",
    )
    .bind(library_id)
    .bind(&series_name)
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

    let missing_count = missing_books.len() as i64;

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
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query("DELETE FROM external_metadata_links WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("link not found"));
    }

    Ok(Json(serde_json::json!({"deleted": true, "id": id.to_string()})))
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

pub(crate) async fn load_provider_config(
    state: &AppState,
    provider_name: &str,
) -> metadata_providers::ProviderConfig {
    let mut config = metadata_providers::ProviderConfig {
        language: "en".to_string(),
        ..Default::default()
    };

    if let Ok(Some(row)) =
        sqlx::query("SELECT value FROM app_settings WHERE key = 'metadata_providers'")
            .fetch_optional(&state.pool)
            .await
    {
        let value: serde_json::Value = row.get("value");
        if let Some(api_key) = value
            .get(provider_name)
            .and_then(|p| p.get("api_key"))
            .and_then(|k| k.as_str())
        {
            if !api_key.is_empty() {
                config.api_key = Some(api_key.to_string());
            }
        }
        // Load preferred language (fallback: "en")
        if let Some(lang) = value
            .get("metadata_language")
            .and_then(|l| l.as_str())
        {
            if !lang.is_empty() {
                config.language = lang.to_string();
            }
        }
    }

    config
}

pub(crate) async fn sync_series_metadata(
    state: &AppState,
    library_id: Uuid,
    series_name: &str,
    metadata_json: &serde_json::Value,
    total_volumes: Option<i32>,
) -> Result<SeriesSyncReport, ApiError> {
    let description = metadata_json
        .get("description")
        .and_then(|d| d.as_str());
    let authors: Vec<String> = metadata_json
        .get("authors")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let publishers: Vec<String> = metadata_json
        .get("publishers")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let start_year = metadata_json
        .get("start_year")
        .and_then(|y| y.as_i64())
        .map(|y| y as i32);
    let status = if let Some(raw) = metadata_json.get("status").and_then(|s| s.as_str()) {
        Some(normalize_series_status(&state.pool, raw).await)
    } else {
        None
    };

    // Fetch existing state before upsert
    let existing = sqlx::query(
        r#"SELECT description, publishers, start_year, total_volumes, status, authors, locked_fields
           FROM series_metadata WHERE library_id = $1 AND name = $2"#,
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(&state.pool)
    .await?;

    // Respect locked_fields: only update fields that are NOT locked
    sqlx::query(
        r#"
        INSERT INTO series_metadata (library_id, name, description, publishers, start_year, total_volumes, status, authors, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
        ON CONFLICT (library_id, name)
        DO UPDATE SET
            description = CASE
                WHEN (series_metadata.locked_fields->>'description')::boolean IS TRUE THEN series_metadata.description
                ELSE COALESCE(NULLIF(EXCLUDED.description, ''), series_metadata.description)
            END,
            publishers = CASE
                WHEN (series_metadata.locked_fields->>'publishers')::boolean IS TRUE THEN series_metadata.publishers
                WHEN array_length(EXCLUDED.publishers, 1) > 0 THEN EXCLUDED.publishers
                ELSE series_metadata.publishers
            END,
            start_year = CASE
                WHEN (series_metadata.locked_fields->>'start_year')::boolean IS TRUE THEN series_metadata.start_year
                ELSE COALESCE(EXCLUDED.start_year, series_metadata.start_year)
            END,
            total_volumes = CASE
                WHEN (series_metadata.locked_fields->>'total_volumes')::boolean IS TRUE THEN series_metadata.total_volumes
                ELSE COALESCE(EXCLUDED.total_volumes, series_metadata.total_volumes)
            END,
            status = CASE
                WHEN (series_metadata.locked_fields->>'status')::boolean IS TRUE THEN series_metadata.status
                ELSE COALESCE(EXCLUDED.status, series_metadata.status)
            END,
            authors = CASE
                WHEN (series_metadata.locked_fields->>'authors')::boolean IS TRUE THEN series_metadata.authors
                WHEN array_length(EXCLUDED.authors, 1) > 0 THEN EXCLUDED.authors
                ELSE series_metadata.authors
            END,
            updated_at = NOW()
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .bind(description)
    .bind(&publishers)
    .bind(start_year)
    .bind(total_volumes)
    .bind(&status)
    .bind(&authors)
    .execute(&state.pool)
    .await?;

    // Build report
    let mut report = SeriesSyncReport::default();
    let locked = existing
        .as_ref()
        .map(|r| r.get::<serde_json::Value, _>("locked_fields"))
        .unwrap_or(serde_json::json!({}));
    let is_locked = |field: &str| -> bool {
        locked.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
    };

    // Helper: compare and record field changes
    struct FieldDef {
        name: &'static str,
        old: Option<serde_json::Value>,
        new: Option<serde_json::Value>,
    }

    let fields = vec![
        FieldDef {
            name: "description",
            old: existing.as_ref().and_then(|r| r.get::<Option<String>, _>("description")).map(serde_json::Value::String),
            new: description.map(|s| serde_json::Value::String(s.to_string())),
        },
        FieldDef {
            name: "authors",
            old: existing.as_ref().map(|r| serde_json::json!(r.get::<Vec<String>, _>("authors"))),
            new: if authors.is_empty() { None } else { Some(serde_json::json!(authors)) },
        },
        FieldDef {
            name: "publishers",
            old: existing.as_ref().map(|r| serde_json::json!(r.get::<Vec<String>, _>("publishers"))),
            new: if publishers.is_empty() { None } else { Some(serde_json::json!(publishers)) },
        },
        FieldDef {
            name: "start_year",
            old: existing.as_ref().and_then(|r| r.get::<Option<i32>, _>("start_year")).map(|y| serde_json::json!(y)),
            new: start_year.map(|y| serde_json::json!(y)),
        },
        FieldDef {
            name: "total_volumes",
            old: existing.as_ref().and_then(|r| r.get::<Option<i32>, _>("total_volumes")).map(|y| serde_json::json!(y)),
            new: total_volumes.map(|y| serde_json::json!(y)),
        },
        FieldDef {
            name: "status",
            old: existing.as_ref().and_then(|r| r.get::<Option<String>, _>("status")).map(serde_json::Value::String),
            new: status.as_ref().map(|s: &String| serde_json::Value::String(s.clone())),
        },
    ];

    for f in fields {
        // Skip if no new value to apply
        if f.new.is_none() {
            continue;
        }
        let change = FieldChange {
            field: f.name.to_string(),
            old_value: f.old.clone(),
            new_value: f.new.clone(),
        };
        if is_locked(f.name) {
            report.fields_skipped.push(change);
        } else if f.old != f.new {
            report.fields_updated.push(change);
        }
    }

    Ok(report)
}

/// Normalize provider-specific status strings using the status_mappings table.
/// Returns None if no mapping is found — unknown statuses are not stored.
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

    // Try substring match (for Bédéthèque-style statuses like "Série finie")
    if let Ok(Some(row)) = sqlx::query_scalar::<_, String>(
        "SELECT mapped_status FROM status_mappings WHERE $1 LIKE '%' || provider_status || '%' AND mapped_status IS NOT NULL LIMIT 1",
    )
    .bind(&lower)
    .fetch_optional(pool)
    .await
    {
        return row;
    }

    // No mapping found — return the provider status as-is (lowercased)
    lower
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

    let provider_config = load_provider_config(state, provider_name).await;

    let books = provider
        .get_series_books(external_id, &provider_config)
        .await
        .map_err(|e| ApiError::internal(format!("provider error: {e}")))?;

    // Delete existing book metadata for this link
    sqlx::query("DELETE FROM external_book_metadata WHERE link_id = $1")
        .bind(link_id)
        .execute(&state.pool)
        .await?;

    let mut matched_count: i64 = 0;
    let mut book_reports: Vec<BookSyncReport> = Vec::new();

    // Pre-fetch all local books for this series, sorted like the backoffice
    // (volume ASC NULLS LAST, then natural title sort)
    let local_books: Vec<(Uuid, Option<i32>, String)> = sqlx::query_as(
        r#"
        SELECT id, volume, title FROM books
        WHERE library_id = $1
          AND COALESCE(NULLIF(series, ''), 'unclassified') = $2
        ORDER BY volume NULLS LAST,
                 REGEXP_REPLACE(LOWER(title), '[0-9].*$', ''),
                 COALESCE((REGEXP_MATCH(LOWER(title), '\d+'))[1]::int, 0),
                 title ASC
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_all(&state.pool)
    .await?;

    // Build effective position for each local book: use volume if set, otherwise 1-based sort order
    let local_books_with_pos: Vec<(Uuid, i32, String)> = local_books
        .iter()
        .enumerate()
        .map(|(idx, (id, vol, title))| (*id, vol.unwrap_or((idx + 1) as i32), title.clone()))
        .collect();

    // Track which local books have already been matched to avoid double-matching
    let mut matched_local_ids = std::collections::HashSet::new();

    for (ext_idx, book) in books.iter().enumerate() {
        // Effective volume for the external book: provider volume_number, or 1-based position
        let ext_vol = book.volume_number.unwrap_or((ext_idx + 1) as i32);

        // Strategy 1: Match by effective volume number
        let mut local_book_id: Option<Uuid> = local_books_with_pos
            .iter()
            .find(|(id, v, _)| *v == ext_vol && !matched_local_ids.contains(id))
            .map(|(id, _, _)| *id);

        // Strategy 2: External title contained in local title or vice-versa (case-insensitive)
        if local_book_id.is_none() {
            let ext_title_lower = book.title.to_lowercase();
            local_book_id = local_books_with_pos.iter().find(|(id, _, local_title)| {
                if matched_local_ids.contains(id) {
                    return false;
                }
                let local_lower = local_title.to_lowercase();
                local_lower.contains(&ext_title_lower) || ext_title_lower.contains(&local_lower)
            }).map(|(id, _, _)| *id);
        }

        if let Some(id) = local_book_id {
            matched_local_ids.insert(id);
        }

        sqlx::query(
            r#"
            INSERT INTO external_book_metadata
                (link_id, book_id, external_book_id, volume_number, title, authors, isbn, summary, cover_url, page_count, language, publish_date, metadata_json)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(link_id)
        .bind(local_book_id)
        .bind(&book.external_book_id)
        .bind(book.volume_number)
        .bind(&book.title)
        .bind(&book.authors)
        .bind(&book.isbn)
        .bind(&book.summary)
        .bind(&book.cover_url)
        .bind(book.page_count)
        .bind(&book.language)
        .bind(&book.publish_date)
        .bind(&book.metadata_json)
        .execute(&state.pool)
        .await?;

        // Push external metadata to matched local book (respecting locked fields)
        if let Some(book_id) = local_book_id {
            // Fetch current state for report
            let current = sqlx::query(
                "SELECT title, summary, isbn, publish_date, language, authors, locked_fields FROM books WHERE id = $1"
            )
            .bind(book_id)
            .fetch_one(&state.pool)
            .await?;

            sqlx::query(
                r#"
                UPDATE books SET
                    summary = CASE
                        WHEN (locked_fields->>'summary')::boolean IS TRUE THEN summary
                        ELSE COALESCE(NULLIF($2, ''), summary)
                    END,
                    isbn = CASE
                        WHEN (locked_fields->>'isbn')::boolean IS TRUE THEN isbn
                        ELSE COALESCE(NULLIF($3, ''), isbn)
                    END,
                    publish_date = CASE
                        WHEN (locked_fields->>'publish_date')::boolean IS TRUE THEN publish_date
                        ELSE COALESCE(NULLIF($4, ''), publish_date)
                    END,
                    language = CASE
                        WHEN (locked_fields->>'language')::boolean IS TRUE THEN language
                        ELSE COALESCE(NULLIF($5, ''), language)
                    END,
                    authors = CASE
                        WHEN (locked_fields->>'authors')::boolean IS TRUE THEN authors
                        WHEN CARDINALITY($6::text[]) > 0 THEN $6
                        ELSE authors
                    END,
                    author = CASE
                        WHEN (locked_fields->>'authors')::boolean IS TRUE THEN author
                        WHEN CARDINALITY($6::text[]) > 0 THEN $6[1]
                        ELSE author
                    END,
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(book_id)
            .bind(&book.summary)
            .bind(&book.isbn)
            .bind(&book.publish_date)
            .bind(&book.language)
            .bind(&book.authors)
            .execute(&state.pool)
            .await?;

            // Build per-book report
            let locked_fields = current.get::<serde_json::Value, _>("locked_fields");
            let is_locked = |field: &str| -> bool {
                locked_fields.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
            };

            let book_title: String = current.get("title");
            let mut fields_updated = Vec::new();
            let mut fields_skipped = Vec::new();

            // Check each syncable field
            let field_checks: Vec<(&str, Option<serde_json::Value>, Option<serde_json::Value>)> = vec![
                ("summary",
                    current.get::<Option<String>, _>("summary").map(|s| serde_json::json!(s)),
                    book.summary.as_ref().map(|s| serde_json::json!(s))),
                ("isbn",
                    current.get::<Option<String>, _>("isbn").map(|s| serde_json::json!(s)),
                    book.isbn.as_ref().map(|s| serde_json::json!(s))),
                ("publish_date",
                    current.get::<Option<String>, _>("publish_date").map(|s| serde_json::json!(s)),
                    book.publish_date.as_ref().map(|s| serde_json::json!(s))),
                ("language",
                    current.get::<Option<String>, _>("language").map(|s| serde_json::json!(s)),
                    book.language.as_ref().map(|s| serde_json::json!(s))),
                ("authors",
                    Some(serde_json::json!(current.get::<Vec<String>, _>("authors"))),
                    if book.authors.is_empty() { None } else { Some(serde_json::json!(&book.authors)) }),
            ];

            for (name, old, new) in field_checks {
                if new.is_none() { continue; }
                let change = FieldChange {
                    field: name.to_string(),
                    old_value: old.clone(),
                    new_value: new.clone(),
                };
                if is_locked(name) {
                    fields_skipped.push(change);
                } else if old != new {
                    fields_updated.push(change);
                }
            }

            // Only include books that had actual changes or skips
            if !fields_updated.is_empty() || !fields_skipped.is_empty() {
                book_reports.push(BookSyncReport {
                    book_id,
                    title: book_title,
                    volume: book.volume_number,
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
