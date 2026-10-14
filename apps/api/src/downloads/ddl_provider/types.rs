use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A DDL provider as exposed to the settings UI.
#[derive(Debug, Serialize, ToSchema)]
pub struct DdlProviderDto {
    pub id: String,
    pub label: String,
    /// `json` for a native API, `html` for a scraped page.
    pub kind: String,
    pub description: String,
    pub base_url: String,
    pub requires_auth: bool,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ToggleDdlProviderRequest {
    pub enabled: bool,
    /// Optional base-URL override (mirror / self-hosted instance).
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DdlProviderSearchRequest {
    /// Free-form query; defaults to `series_name` when omitted.
    pub query: Option<String>,
    pub series_name: Option<String>,
    pub library_id: Option<String>,
    /// Missing volume numbers of the target series, used for match ranking.
    #[serde(default)]
    pub missing_volumes: Vec<i32>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DdlProviderSearchResultDto {
    pub provider: String,
    pub external_id: String,
    pub filename: String,
    pub size: Option<i64>,
    pub series_name: Option<String>,
    pub volume_number: Option<i32>,
    /// Every volume extracted from the filename (not only the missing ones).
    pub all_volumes: Vec<i32>,
    pub matched_missing_volumes: Vec<i32>,
    pub match_confidence: Option<String>,
    /// The resolved volume is already present in the target library.
    pub already_owned: bool,
    /// Id of an existing `ddl_provider_downloads` row for this file, if any.
    pub download_id: Option<String>,
    pub download_status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DdlProviderSearchResponse {
    pub provider: String,
    pub query: String,
    pub results: Vec<DdlProviderSearchResultDto>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DdlProviderTestResponse {
    pub success: bool,
    pub message: String,
    pub result_count: Option<usize>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DdlProviderTestRequest {
    /// Query used to probe the provider; defaults to a common series name.
    pub query: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartDdlProviderDownloadRequest {
    pub external_id: String,
    pub filename: String,
    pub size: Option<i64>,
    pub library_id: String,
    pub series_name: String,
    pub volume_number: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DdlProviderDownloadDto {
    pub id: String,
    pub provider: String,
    pub external_id: String,
    pub filename: String,
    pub file_size: Option<i64>,
    pub bytes_downloaded: i64,
    pub series_name: Option<String>,
    pub series_id: Option<String>,
    pub volume_number: Option<i32>,
    pub library_id: Option<String>,
    pub library_name: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct DismissDownloadQuery {
    pub hard: Option<bool>,
}

/// Cadence of the background DDL search (`0` disables it).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DdlSearchSettingsDto {
    pub search_interval_minutes: i32,
}
