//! HTTP handlers for DDL providers.
//!
//! Routes are registered on the admin router in `main.rs`:
//!
//! * `GET    /ddl-providers`
//! * `POST   /ddl-providers/{id}/toggle`
//! * `POST   /ddl-providers/{id}/test`
//! * `POST   /ddl-providers/{id}/search`
//! * `POST   /ddl-providers/{id}/downloads`
//! * `GET    /ddl-provider-downloads`
//! * `DELETE /ddl-provider-downloads/{id}`

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::ApiError;
use crate::jobs::lifecycle::{job_in_flight, JobScope};
use crate::state::AppState;

use super::service;
use super::types::*;
use super::{load_config, registry, resolve_base_url, save_config};

/// `GET /ddl-providers` — every known provider with its enabled state.
#[utoipa::path(
    get,
    path = "/ddl-providers",
    tag = "downloads",
    responses((status = 200, description = "Registered DDL providers", body = [DdlProviderDto]))
)]
pub async fn list_ddl_providers(
    State(state): State<AppState>,
) -> Result<Json<Vec<DdlProviderDto>>, ApiError> {
    let config = load_config(&state.pool).await?;
    let providers = registry()
        .into_iter()
        .map(|provider| {
            let meta = provider.meta();
            let settings = config.providers.get(meta.id);
            DdlProviderDto {
                id: meta.id.to_string(),
                label: meta.label.to_string(),
                kind: meta.kind.to_string(),
                description: meta.description.to_string(),
                base_url: resolve_base_url(&config, &meta),
                requires_auth: meta.requires_auth,
                enabled: settings.map(|s| s.enabled).unwrap_or(false),
            }
        })
        .collect();

    Ok(Json(providers))
}

/// `POST /ddl-providers/{id}/toggle` — enable/disable a provider.
#[utoipa::path(
    post,
    path = "/ddl-providers/{id}/toggle",
    tag = "downloads",
    params(("id" = String, Path, description = "Provider id")),
    request_body = ToggleDdlProviderRequest,
    responses(
        (status = 200, description = "Updated provider list", body = [DdlProviderDto]),
        (status = 404, description = "Unknown provider")
    )
)]
pub async fn toggle_ddl_provider(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<ToggleDdlProviderRequest>,
) -> Result<Json<Vec<DdlProviderDto>>, ApiError> {
    let mut config = load_config(&state.pool).await?;

    if super::get_ddl_provider(&id).is_none() {
        return Err(ApiError::not_found(format!("unknown DDL provider: {id}")));
    }

    let entry = config.providers.entry(id).or_default();
    entry.enabled = body.enabled;
    if let Some(base_url) = body.base_url {
        let trimmed = base_url.trim().to_string();
        entry.base_url = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
    }

    save_config(&state.pool, &config).await?;
    list_ddl_providers(State(state)).await
}

/// `POST /ddl-providers/{id}/test` — reachability / result-count probe.
#[utoipa::path(
    post,
    path = "/ddl-providers/{id}/test",
    tag = "downloads",
    params(("id" = String, Path, description = "Provider id")),
    request_body = DdlProviderTestRequest,
    responses((status = 200, description = "Probe result", body = DdlProviderTestResponse))
)]
pub async fn test_ddl_provider(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<DdlProviderTestRequest>,
) -> Result<Json<DdlProviderTestResponse>, ApiError> {
    let query = body.query.unwrap_or_else(|| "asterix".to_string());

    match service::probe(&state, &id, &query).await {
        Ok(count) => Ok(Json(DdlProviderTestResponse {
            success: true,
            message: format!("{count} résultat(s) pour « {query} »"),
            result_count: Some(count),
        })),
        Err(err) => Ok(Json(DdlProviderTestResponse {
            success: false,
            message: err.message,
            result_count: None,
        })),
    }
}

/// `POST /ddl-providers/{id}/search` — search a provider and rank results
/// against the target series' missing volumes.
#[utoipa::path(
    post,
    path = "/ddl-providers/{id}/search",
    tag = "downloads",
    params(("id" = String, Path, description = "Provider id")),
    request_body = DdlProviderSearchRequest,
    responses((status = 200, description = "Ranked search results", body = DdlProviderSearchResponse))
)]
pub async fn search_ddl_provider(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<DdlProviderSearchRequest>,
) -> Result<Json<DdlProviderSearchResponse>, ApiError> {
    let response = service::search(&state, &id, body).await?;
    Ok(Json(response))
}

/// `GET /ddl-provider-downloads` — download history.
#[utoipa::path(
    get,
    path = "/ddl-provider-downloads",
    tag = "downloads",
    responses((status = 200, description = "Provider downloads", body = [DdlProviderDownloadDto]))
)]
pub async fn list_ddl_downloads(
    State(state): State<AppState>,
) -> Result<Json<Vec<DdlProviderDownloadDto>>, ApiError> {
    Ok(Json(service::list_ddl_downloads(&state).await?))
}

/// `POST /ddl-providers/{id}/downloads` — download a file and import it.
#[utoipa::path(
    post,
    path = "/ddl-providers/{id}/downloads",
    tag = "downloads",
    params(("id" = String, Path, description = "Provider id")),
    request_body = StartDdlProviderDownloadRequest,
    responses((status = 200, description = "Download queued", body = serde_json::Value))
)]
pub async fn start_ddl_download(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<StartDdlProviderDownloadRequest>,
) -> Result<Json<Value>, ApiError> {
    let download_id = service::start_ddl_download(&state, &id, body).await?;
    Ok(Json(
        json!({ "id": download_id.to_string(), "status": "queued" }),
    ))
}

/// `DELETE /ddl-provider-downloads/{id}` — dismiss (soft) or delete.
#[utoipa::path(
    delete,
    path = "/ddl-provider-downloads/{id}",
    tag = "downloads",
    params(
        ("id" = Uuid, Path, description = "Download id"),
        ("hard" = Option<bool>, Query, description = "Delete the row instead of dismissing it")
    ),
    responses(
        (status = 204, description = "Download dismissed"),
        (status = 404, description = "Download not found")
    )
)]
pub async fn dismiss_ddl_download(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
    Query(query): Query<DismissDownloadQuery>,
) -> Result<StatusCode, ApiError> {
    service::dismiss_ddl_download(&state, id, query.hard.unwrap_or(false)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /ddl-providers/search-settings` — background search cadence.
#[utoipa::path(
    get,
    path = "/ddl-providers/search-settings",
    tag = "downloads",
    responses((status = 200, description = "DDL background search settings", body = DdlSearchSettingsDto))
)]
pub async fn get_ddl_search_settings(
    State(state): State<AppState>,
) -> Result<Json<DdlSearchSettingsDto>, ApiError> {
    let config = load_config(&state.pool).await?;
    Ok(Json(DdlSearchSettingsDto {
        search_interval_minutes: config.search_interval_minutes,
    }))
}

/// `POST /ddl-providers/search-settings` — set the background search cadence
/// (`0` disables it).
#[utoipa::path(
    post,
    path = "/ddl-providers/search-settings",
    tag = "downloads",
    request_body = DdlSearchSettingsDto,
    responses((status = 200, description = "Updated settings", body = DdlSearchSettingsDto))
)]
pub async fn update_ddl_search_settings(
    State(state): State<AppState>,
    Json(body): Json<DdlSearchSettingsDto>,
) -> Result<Json<DdlSearchSettingsDto>, ApiError> {
    if body.search_interval_minutes < 0 {
        return Err(ApiError::bad_request(
            "search_interval_minutes must be >= 0",
        ));
    }

    let mut config = load_config(&state.pool).await?;
    config.search_interval_minutes = body.search_interval_minutes;
    save_config(&state.pool, &config).await?;

    Ok(Json(DdlSearchSettingsDto {
        search_interval_minutes: config.search_interval_minutes,
    }))
}

/// `POST /ddl-providers/search-all` — queue a background search over every
/// enabled provider. Returns the job id, or the id of the run already in flight.
#[utoipa::path(
    post,
    path = "/ddl-providers/search-all",
    tag = "downloads",
    responses((status = 200, description = "Search job queued", body = serde_json::Value))
)]
pub async fn start_ddl_search_all(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let config = load_config(&state.pool).await?;
    if super::enabled_providers(&config).is_empty() {
        return Err(ApiError::bad_request("no DDL provider is enabled"));
    }

    if let Some(existing) = job_in_flight(&state.pool, JobScope::Global, &["ddl_search"]).await? {
        return Ok(Json(json!({
            "id": existing.to_string(),
            "status": "already_running",
        })));
    }

    let job_id = Uuid::new_v4();
    sqlx::query("INSERT INTO index_jobs (id, type, status) VALUES ($1, 'ddl_search', 'pending')")
        .bind(job_id)
        .execute(&state.pool)
        .await?;

    Ok(Json(
        json!({ "id": job_id.to_string(), "status": "pending" }),
    ))
}
