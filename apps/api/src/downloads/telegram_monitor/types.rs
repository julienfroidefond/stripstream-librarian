use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramMonitorStatus {
    pub configured: bool,
    pub authorized: bool,
    pub phone: Option<String>,
    pub api_id: Option<i64>,
    pub sync_interval_minutes: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveTelegramMonitorSettingsRequest {
    pub api_id: Option<i64>,
    pub api_hash: Option<String>,
    pub phone: Option<String>,
    pub sync_interval_minutes: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VerifyCodeRequest {
    pub code: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramSourceDto {
    pub id: String,
    pub channel_username: String,
    pub channel_title: Option<String>,
    pub library_id: Option<String>,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddSourceRequest {
    pub channel_username: String,
    pub library_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramBookLinkDto {
    pub id: String,
    pub source_id: String,
    pub channel_username: String,
    pub message_id: i64,
    pub filename: String,
    pub file_size: Option<i64>,
    pub mime_type: Option<String>,
    pub message_text: Option<String>,
    pub status: String,
    pub library_id: Option<String>,
    pub book_id: Option<String>,
    pub error_message: Option<String>,
    pub series_name: Option<String>,
    pub volume_number: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramAvailableBookDto {
    pub id: String,
    pub channel_username: String,
    pub message_id: i64,
    pub filename: String,
    pub file_size: Option<i64>,
    pub volume_number: Option<i32>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramAvailableGroupDto {
    pub series_name: String,
    pub series_id: Option<String>,
    pub library_id: String,
    pub library_name: String,
    pub owned_volumes: Vec<i32>,
    pub series_missing_count: i32,
    pub books: Vec<TelegramAvailableBookDto>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramDownloadItemDto {
    pub id: String,
    pub series_name: Option<String>,
    pub series_id: Option<String>,
    pub library_id: Option<String>,
    pub library_name: Option<String>,
    pub channel_username: String,
    pub filename: String,
    pub file_size: Option<i64>,
    pub bytes_downloaded: i64,
    pub volume_number: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DownloadBookRequest {
    pub library_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SyncResult {
    pub synced: usize,
    pub new_books: usize,
    pub series_searched: usize,
    /// Series that returned at least one Telegram result: (query_name, message_count, extracted_names)
    #[serde(skip)]
    pub series_results: Vec<(String, usize, Vec<String>)>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChannelSuggestion {
    pub username: Option<String>,
    pub title: String,
    pub kind: String,
}

#[derive(Deserialize)]
pub struct DismissBookQuery {
    pub hard: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LiveSearchRequest {
    pub query: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramSearchResultDto {
    pub id: String,
    pub channel_username: String,
    pub filename: String,
    pub file_size: Option<i64>,
    pub volume_number: Option<i32>,
    pub series_name: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchAvailableQuery {
    pub q: Option<String>,
}

pub struct PendingAuth {
    pub client: grammers_client::Client,
    pub token: grammers_client::types::LoginToken,
}
