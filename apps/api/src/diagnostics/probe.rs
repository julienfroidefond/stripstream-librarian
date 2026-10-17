//! On-demand probes: external integrations and file-existence scan.
//!
//! Both are **not** part of `GET /diagnostics`: they contact third-party
//! services or stat every book file, which has no place on a page load. The
//! Backoffice triggers them explicitly with the "Run probe" button.

use std::time::Duration;

use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::state::AppState;

/// Hard ceiling for one external integration probe.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound of files stat'ed by the missing-file scan.
const MISSING_FILES_LIMIT: i64 = 5_000;
/// How many missing paths are returned as examples.
const MISSING_FILES_SAMPLES: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DiagnosticsProbe {
    pub sampled_at: DateTime<Utc>,
    pub integrations: Vec<IntegrationDiagnostics>,
    pub missing_files: MissingFilesReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IntegrationDiagnostics {
    /// `qbittorrent`, `prowlarr`, `telegram` or `ddl_<provider>`.
    pub id: String,
    /// Whether the integration has enough settings to be probed at all.
    pub configured: bool,
    /// Whether the probe got a usable answer.
    pub reachable: bool,
    pub latency_ms: u64,
    /// Short, sanitised detail (HTTP status, `timeout`, provider version…).
    /// Never contains a URL, token or credential.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct MissingFilesReport {
    /// Files stat'ed (capped at [`MISSING_FILES_LIMIT`]).
    pub checked: i64,
    pub missing: i64,
    /// Library-relative paths (`/libraries/…`), capped for display.
    pub samples: Vec<String>,
}

/// Probe configured integrations and look for book files that vanished.
#[utoipa::path(
    get,
    path = "/diagnostics/probe",
    tag = "diagnostics",
    responses(
        (status = 200, body = DiagnosticsProbe),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn diagnostics_probe(State(state): State<AppState>) -> Json<DiagnosticsProbe> {
    let (integrations, missing_files) = tokio::join!(
        integration_probes(&state.pool),
        missing_files_probe(&state.pool)
    );

    Json(DiagnosticsProbe {
        sampled_at: Utc::now(),
        integrations,
        missing_files,
    })
}

// ─── Integrations ────────────────────────────────────────────────────────────

async fn integration_probes(pool: &sqlx::PgPool) -> Vec<IntegrationDiagnostics> {
    let (qbittorrent, prowlarr, telegram, ddl) = tokio::join!(
        probe_qbittorrent(pool),
        probe_prowlarr(pool),
        probe_telegram(pool),
        probe_ddl_providers(pool),
    );

    let mut probes = vec![qbittorrent, prowlarr, telegram];
    probes.extend(ddl);
    probes
}

async fn probe_qbittorrent(pool: &sqlx::PgPool) -> IntegrationDiagnostics {
    const ID: &str = "qbittorrent";

    let Ok((base_url, username, password)) =
        crate::downloads::qbittorrent::load_qbittorrent_config(pool).await
    else {
        return not_configured(ID);
    };

    let client = match stripstream_core::http::build_http_client(PROBE_TIMEOUT) {
        Ok(client) => client,
        Err(_) => return failed(ID, 0, "invalid client configuration"),
    };

    let started = std::time::Instant::now();
    let probe = tokio::time::timeout(PROBE_TIMEOUT, async {
        let sid = crate::downloads::qbittorrent::qbittorrent_login(
            &client, &base_url, &username, &password,
        )
        .await
        .ok()?;

        let response = client
            .get(format!("{base_url}/api/v2/app/version"))
            .header("Cookie", format!("SID={sid}"))
            .send()
            .await
            .ok()?;

        let status = response.status();
        if !status.is_success() {
            return None;
        }
        response.text().await.ok().map(|v| v.trim().to_string())
    })
    .await;

    let latency = started.elapsed().as_millis() as u64;

    match probe {
        Ok(Some(version)) => ok(ID, latency, Some(version)),
        _ => failed(ID, latency, "login or version request failed"),
    }
}

async fn probe_prowlarr(pool: &sqlx::PgPool) -> IntegrationDiagnostics {
    const ID: &str = "prowlarr";

    let Ok((base_url, api_key, _categories)) =
        crate::downloads::prowlarr::load_prowlarr_config_internal(pool).await
    else {
        return not_configured(ID);
    };

    let Ok(client) = stripstream_core::http::build_http_client(PROBE_TIMEOUT) else {
        return failed(ID, 0, "invalid client configuration");
    };

    let started = std::time::Instant::now();
    let probe = tokio::time::timeout(PROBE_TIMEOUT, async {
        client
            .get(format!("{}/api/v1/indexer", base_url.trim_end_matches('/')))
            .header("X-Api-Key", api_key)
            .send()
            .await
            .ok()
    })
    .await;

    let latency = started.elapsed().as_millis() as u64;

    match probe {
        Ok(Some(response)) if response.status().is_success() => ok(ID, latency, None),
        Ok(Some(response)) => failed(ID, latency, http_status(&response.status())),
        _ => failed(ID, latency, "connection failed"),
    }
}

async fn probe_telegram(pool: &sqlx::PgPool) -> IntegrationDiagnostics {
    const ID: &str = "telegram";

    let Some(config) = notifications::load_telegram_config(pool).await else {
        return not_configured(ID);
    };

    let Ok(client) = stripstream_core::http::build_http_client(PROBE_TIMEOUT) else {
        return failed(ID, 0, "invalid client configuration");
    };

    let started = std::time::Instant::now();
    let probe = tokio::time::timeout(PROBE_TIMEOUT, async {
        client
            .get(format!(
                "https://api.telegram.org/bot{}/getMe",
                config.bot_token
            ))
            .send()
            .await
            .ok()
    })
    .await;

    let latency = started.elapsed().as_millis() as u64;

    match probe {
        Ok(Some(response)) if response.status().is_success() => ok(ID, latency, None),
        Ok(Some(response)) => failed(ID, latency, http_status(&response.status())),
        _ => failed(ID, latency, "connection failed"),
    }
}

async fn probe_ddl_providers(pool: &sqlx::PgPool) -> Vec<IntegrationDiagnostics> {
    let Ok(config) = crate::downloads::ddl_provider::load_config(pool).await else {
        return Vec::new();
    };

    let providers = crate::downloads::ddl_provider::enabled_providers(&config);
    if providers.is_empty() {
        return Vec::new();
    }

    let Ok(client) = stripstream_core::http::build_http_client(PROBE_TIMEOUT) else {
        return Vec::new();
    };

    let mut probes = Vec::with_capacity(providers.len());
    for (provider, base_url) in providers {
        let id = format!("ddl_{}", provider.meta().id);
        let started = std::time::Instant::now();

        let probe = tokio::time::timeout(PROBE_TIMEOUT, async {
            client.get(&base_url).send().await.ok()
        })
        .await;

        let latency = started.elapsed().as_millis() as u64;
        probes.push(match probe {
            // Any HTTP answer means the provider is reachable; a 403 is often
            // a bot wall rather than an outage, so it is surfaced verbatim.
            Ok(Some(response)) if response.status().is_success() => ok(&id, latency, None),
            Ok(Some(response)) => failed(&id, latency, http_status(&response.status())),
            _ => failed(&id, latency, "connection failed"),
        });
    }

    probes
}

// ─── Missing files ───────────────────────────────────────────────────────────

async fn missing_files_probe(pool: &sqlx::PgPool) -> MissingFilesReport {
    let paths =
        sqlx::query_scalar::<_, String>("SELECT abs_path FROM book_files ORDER BY id LIMIT $1")
            .bind(MISSING_FILES_LIMIT)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    let checked = paths.len() as i64;

    tokio::task::spawn_blocking(move || {
        let mut missing = 0;
        let mut samples = Vec::new();

        for db_path in &paths {
            let host_path = stripstream_core::paths::remap_libraries_path(db_path);
            if !std::path::Path::new(&host_path).exists() {
                missing += 1;
                if samples.len() < MISSING_FILES_SAMPLES {
                    samples.push(db_path.clone());
                }
            }
        }

        MissingFilesReport {
            checked,
            missing,
            samples,
        }
    })
    .await
    .unwrap_or(MissingFilesReport {
        checked,
        ..MissingFilesReport::default()
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn ok(id: &str, latency_ms: u64, detail: Option<String>) -> IntegrationDiagnostics {
    IntegrationDiagnostics {
        id: id.to_string(),
        configured: true,
        reachable: true,
        latency_ms,
        detail,
    }
}

fn failed(id: &str, latency_ms: u64, detail: &str) -> IntegrationDiagnostics {
    IntegrationDiagnostics {
        id: id.to_string(),
        configured: true,
        reachable: false,
        latency_ms,
        detail: Some(detail.to_string()),
    }
}

fn not_configured(id: &str) -> IntegrationDiagnostics {
    IntegrationDiagnostics {
        id: id.to_string(),
        configured: false,
        reachable: false,
        latency_ms: 0,
        detail: None,
    }
}

fn http_status(status: &reqwest::StatusCode) -> &'static str {
    match status.as_u16() {
        401 | 403 => "authentication or bot wall",
        404 => "not found",
        429 => "rate limited",
        _ if status.is_server_error() => "server error",
        _ => "unexpected status",
    }
}
