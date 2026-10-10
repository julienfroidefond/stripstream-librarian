use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Deserialize, IntoParams)]
pub struct StatsQuery {
    /// Granularity: "day", "week" or "month" (default: "week")
    pub period: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct StatsOverview {
    pub total_books: i64,
    pub total_series: i64,
    pub total_libraries: i64,
    pub total_pages: i64,
    pub total_size_bytes: i64,
    pub total_authors: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ReadingStatusStats {
    pub unread: i64,
    pub reading: i64,
    pub read: i64,
}

#[derive(Serialize, ToSchema)]
pub struct FormatCount {
    pub format: String,
    pub count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct LanguageCount {
    pub language: Option<String>,
    pub count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct LibraryStats {
    pub library_name: String,
    pub book_count: i64,
    pub size_bytes: i64,
    pub read_count: i64,
    pub reading_count: i64,
    pub unread_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct TopSeries {
    pub series: String,
    pub book_count: i64,
    pub read_count: i64,
    pub total_pages: i64,
}

#[derive(Serialize, ToSchema)]
pub struct MonthlyAdditions {
    pub month: String,
    pub books_added: i64,
}

#[derive(Serialize, ToSchema)]
pub struct MetadataStats {
    pub total_series: i64,
    pub series_linked: i64,
    pub series_unlinked: i64,
    pub books_with_summary: i64,
    pub books_with_isbn: i64,
    pub by_provider: Vec<ProviderCount>,
}

#[derive(Serialize, ToSchema)]
pub struct ProviderCount {
    pub provider: String,
    pub count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct CurrentlyReadingItem {
    pub book_id: String,
    pub title: String,
    pub series: Option<String>,
    pub current_page: i32,
    pub page_count: i32,
    pub username: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct RecentlyReadItem {
    pub book_id: String,
    pub title: String,
    pub series: Option<String>,
    pub last_read_at: String,
    pub username: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct MonthlyReading {
    pub month: String,
    pub books_read: i64,
    pub pages_read: i64,
}

#[derive(Serialize, ToSchema)]
pub struct UserMonthlyReading {
    pub month: String,
    pub username: String,
    pub books_read: i64,
    pub pages_read: i64,
}

#[derive(Serialize, ToSchema)]
pub struct JobTimePoint {
    pub label: String,
    pub scan: i64,
    pub rebuild: i64,
    pub thumbnail: i64,
    pub metadata: i64,
    pub downloads: i64,
    pub reading: i64,
    pub conversion: i64,
}

#[derive(Serialize, ToSchema)]
pub struct DownloadStats {
    pub active_downloads: i64,
    pub imported_downloads: i64,
    pub error_downloads: i64,
    pub total_downloads: i64,
    pub available_series: i64,
    pub total_missing_volumes: i64,
    pub recent_downloads: Vec<RecentDownloadItem>,
}

#[derive(Serialize, ToSchema)]
pub struct RecentDownloadItem {
    pub id: String,
    pub series_name: String,
    pub status: String,
    pub expected_volumes: Vec<i32>,
    pub created_at: String,
}

#[derive(Serialize, ToSchema)]
pub struct StatsResponse {
    pub overview: StatsOverview,
    pub reading_status: ReadingStatusStats,
    pub currently_reading: Vec<CurrentlyReadingItem>,
    pub recently_read: Vec<RecentlyReadItem>,
    pub reading_over_time: Vec<MonthlyReading>,
    pub by_format: Vec<FormatCount>,
    pub by_language: Vec<LanguageCount>,
    pub by_library: Vec<LibraryStats>,
    pub top_series: Vec<TopSeries>,
    pub additions_over_time: Vec<MonthlyAdditions>,
    pub jobs_over_time: Vec<JobTimePoint>,
    pub metadata: MetadataStats,
    pub users_reading_over_time: Vec<UserMonthlyReading>,
    pub downloads: DownloadStats,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserReadingOverviewItem {
    pub book_id: String,
    pub title: String,
    pub series: Option<String>,
    pub series_id: Option<String>,
    pub current_page: i32,
    pub page_count: i32,
    pub last_read_at: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserReadingOverviewSeriesBook {
    pub book_id: String,
    pub title: String,
    pub volume: Option<i32>,
    pub volume_type: String,
    pub status: String,
    pub current_page: i32,
    pub page_count: i32,
    pub last_read_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct StatsOverviewResponse {
    pub overview: StatsOverview,
    pub reading_status: ReadingStatusStats,
    pub by_format: Vec<FormatCount>,
    pub by_language: Vec<LanguageCount>,
    pub metadata: MetadataStats,
    pub currently_reading: Vec<CurrentlyReadingItem>,
    pub recently_read: Vec<RecentlyReadItem>,
}

#[derive(Serialize, ToSchema)]
pub struct StatsBreakdownResponse {
    pub by_library: Vec<LibraryStats>,
    pub top_series: Vec<TopSeries>,
    pub downloads: DownloadStats,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserReadingOverviewSeries {
    pub series_id: Option<String>,
    pub series_name: String,
    pub books_total: i64,
    pub books_read: i64,
    pub books_reading: i64,
    pub books_unread: i64,
    pub last_read_at: Option<String>,
    pub books: Vec<UserReadingOverviewSeriesBook>,
}

#[derive(Serialize, ToSchema)]
pub struct UserReadingOverview {
    #[schema(value_type = String)]
    pub user_id: uuid::Uuid,
    pub username: String,
    pub books_read: i64,
    pub books_reading: i64,
    pub series_in_progress: i64,
    pub last_read_at: Option<String>,
    pub currently_reading: Vec<UserReadingOverviewItem>,
    pub recently_read: Vec<UserReadingOverviewItem>,
    pub series_progress: Vec<UserReadingOverviewSeries>,
}
