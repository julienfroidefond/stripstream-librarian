use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::books::list_books,
        crate::books::get_book,
        crate::reading_progress::get_reading_progress,
        crate::reading_progress::update_reading_progress,
        crate::reading_progress::mark_series_read,
        crate::books::get_thumbnail,
        crate::series::list_series,
        crate::series::list_all_series,
        crate::series::ongoing_series,
        crate::series::ongoing_books,
        crate::books::convert_book,
        crate::books::update_book,
        crate::series::get_series_metadata,
        crate::series::get_series_by_name,
        crate::series::update_series,
        crate::series::delete_series,
        crate::series::get_series_by_id,
        crate::series::get_series_metadata_by_id,
        crate::series::update_series_by_id,
        crate::series::delete_series_by_id,
        crate::pages::get_page,
        crate::search::search_books,
        crate::index_jobs::enqueue_rebuild,
        crate::thumbnails::start_thumbnails_rebuild,
        crate::thumbnails::start_thumbnails_regenerate,
        crate::index_jobs::list_index_jobs,
        crate::index_jobs::get_active_jobs,
        crate::index_jobs::get_job_details,
        crate::index_jobs::stream_job_progress,
        crate::index_jobs::get_job_errors,
        crate::index_jobs::cancel_job,
        crate::index_jobs::list_folders,
        crate::libraries::list_libraries,
        crate::libraries::create_library,
        crate::libraries::delete_library,
        crate::libraries::scan_library,
        crate::libraries::update_monitoring,
        crate::libraries::update_metadata_provider,
        crate::tokens::list_tokens,
        crate::tokens::create_token,
        crate::tokens::revoke_token,
        crate::tokens::delete_token,
        crate::authors::list_authors,
        crate::stats::get_stats,
        crate::settings::get_settings,
        crate::settings::get_setting,
        crate::settings::update_setting,
        crate::settings::clear_cache,
        crate::settings::get_cache_stats,
        crate::settings::get_thumbnail_stats,
        crate::metadata::search_metadata,
        crate::metadata::create_metadata_match,
        crate::metadata::approve_metadata,
        crate::metadata::reject_metadata,
        crate::metadata::get_metadata_links,
        crate::metadata::get_missing_books,
        crate::metadata::delete_metadata_link,
        crate::series::series_statuses,
        crate::series::provider_statuses,
        crate::settings::list_status_mappings,
        crate::settings::upsert_status_mapping,
        crate::settings::delete_status_mapping,
        crate::prowlarr::search_prowlarr,
        crate::prowlarr::test_prowlarr,
        crate::qbittorrent::add_torrent,
        crate::qbittorrent::test_qbittorrent,
        crate::metadata_batch::start_batch,
        crate::metadata_batch::get_batch_report,
        crate::metadata_batch::get_batch_results,
        crate::metadata_refresh::start_refresh,
        crate::metadata_refresh::get_refresh_report,
        crate::komga::sync_komga_read_books,
        crate::komga::list_sync_reports,
        crate::komga::get_sync_report,
        crate::users::list_users,
        crate::users::create_user,
        crate::users::update_user,
        crate::users::delete_user,
        crate::tokens::update_token,
        crate::libraries::update_reading_status_provider,
        crate::reading_status_match::start_match,
        crate::reading_status_match::get_match_report,
        crate::reading_status_match::get_match_results,
        crate::reading_status_push::start_push,
        crate::reading_status_push::get_push_report,
        crate::reading_status_push::get_push_results,
        crate::anilist::get_status,
        crate::anilist::search_manga,
        crate::anilist::get_series_link,
        crate::anilist::link_series,
        crate::anilist::unlink_series,
        crate::anilist::get_series_link_by_id,
        crate::anilist::link_series_by_id,
        crate::anilist::unlink_series_by_id,
        crate::anilist::toggle_library,
        crate::anilist::list_unlinked,
        crate::anilist::preview_sync,
        crate::anilist::sync_to_anilist,
        crate::anilist::pull_from_anilist,
        crate::anilist::list_links,
    ),
    components(
        schemas(
            crate::books::ListBooksQuery,
            crate::books::BookItem,
            crate::books::BooksPage,
            crate::books::BookDetails,
            crate::reading_progress::ReadingProgressResponse,
            crate::reading_progress::UpdateReadingProgressRequest,
            crate::reading_progress::MarkSeriesReadRequest,
            crate::reading_progress::MarkSeriesReadResponse,
            crate::series::SeriesItem,
            crate::series::SeriesPage,
            crate::series::ListAllSeriesQuery,
            crate::series::OngoingQuery,
            crate::books::UpdateBookRequest,
            crate::series::SeriesMetadata,
            crate::series::SeriesLookup,
            crate::series::UpdateSeriesRequest,
            crate::series::UpdateSeriesResponse,
            crate::pages::PageQuery,
            crate::search::SearchQuery,
            crate::search::SearchResponse,
            crate::search::SeriesHit,
            crate::index_jobs::RebuildRequest,
            crate::thumbnails::ThumbnailsRebuildRequest,
            crate::index_jobs::IndexJobResponse,
            crate::index_jobs::IndexJobDetailResponse,
            crate::index_jobs::JobErrorResponse,
            crate::index_jobs::ProgressEvent,
            crate::index_jobs::FolderItem,
            crate::libraries::LibraryResponse,
            crate::libraries::CreateLibraryRequest,
            crate::libraries::UpdateMonitoringRequest,
            crate::libraries::UpdateMetadataProviderRequest,
            crate::tokens::CreateTokenRequest,
            crate::tokens::TokenResponse,
            crate::tokens::CreatedTokenResponse,
            crate::settings::UpdateSettingRequest,
            crate::settings::ClearCacheResponse,
            crate::settings::CacheStats,
            crate::settings::ThumbnailStats,
            crate::settings::StatusMappingDto,
            crate::settings::UpsertStatusMappingRequest,
            crate::authors::ListAuthorsQuery,
            crate::authors::AuthorItem,
            crate::authors::AuthorsPageResponse,
            crate::stats::StatsResponse,
            crate::stats::StatsOverview,
            crate::stats::ReadingStatusStats,
            crate::stats::FormatCount,
            crate::stats::LanguageCount,
            crate::stats::LibraryStats,
            crate::stats::TopSeries,
            crate::stats::MonthlyAdditions,
            crate::stats::MetadataStats,
            crate::stats::ProviderCount,
            crate::stats::CurrentlyReadingItem,
            crate::stats::RecentlyReadItem,
            crate::stats::MonthlyReading,
            crate::stats::UserMonthlyReading,
            crate::stats::JobTimePoint,
            crate::stats::DownloadStats,
            crate::stats::RecentDownloadItem,
            crate::metadata::ApproveRequest,
            crate::metadata::ApproveResponse,
            crate::metadata::SyncReport,
            crate::metadata::SeriesSyncReport,
            crate::metadata::BookSyncReport,
            crate::metadata::FieldChange,
            crate::metadata::MetadataSearchRequest,
            crate::metadata::SeriesCandidateDto,
            crate::metadata::MetadataMatchRequest,
            crate::metadata::ExternalMetadataLinkDto,
            crate::metadata::MissingBooksDto,
            crate::metadata::MissingBookItem,
            crate::qbittorrent::QBittorrentAddRequest,
            crate::qbittorrent::QBittorrentAddResponse,
            crate::qbittorrent::QBittorrentTestResponse,
            crate::prowlarr::ProwlarrSearchRequest,
            crate::prowlarr::ProwlarrRelease,
            crate::prowlarr::ProwlarrCategory,
            crate::prowlarr::ProwlarrSearchResponse,
            crate::prowlarr::MissingVolumeInput,
            crate::prowlarr::ProwlarrTestResponse,
            crate::metadata_batch::MetadataBatchRequest,
            crate::metadata_batch::MetadataBatchReportDto,
            crate::metadata_batch::MetadataBatchResultDto,
            crate::metadata_refresh::MetadataRefreshRequest,
            crate::metadata_refresh::MetadataRefreshReportDto,
            crate::komga::KomgaSyncRequest,
            crate::komga::KomgaSyncResponse,
            crate::komga::KomgaSyncReportSummary,
            crate::users::UserResponse,
            crate::users::CreateUserRequest,
            crate::tokens::UpdateTokenRequest,
            crate::libraries::UpdateReadingStatusProviderRequest,
            crate::reading_status_match::ReadingStatusMatchRequest,
            crate::reading_status_match::ReadingStatusMatchReportDto,
            crate::reading_status_match::ReadingStatusMatchResultDto,
            crate::reading_status_push::ReadingStatusPushRequest,
            crate::reading_status_push::ReadingStatusPushReportDto,
            crate::reading_status_push::ReadingStatusPushResultDto,
            crate::anilist::AnilistStatusResponse,
            crate::anilist::AnilistMediaResult,
            crate::anilist::AnilistSeriesLinkResponse,
            crate::anilist::AnilistSyncPreviewItem,
            crate::anilist::AnilistSyncItem,
            crate::anilist::AnilistSyncReport,
            crate::anilist::AnilistPullItem,
            crate::anilist::AnilistPullReport,
            crate::anilist::AnilistSearchRequest,
            crate::anilist::AnilistLinkRequest,
            crate::anilist::AnilistLibraryToggleRequest,
            ErrorResponse,
        )
    ),
    security(
        ("Bearer" = [])
    ),
    tags(
        (name = "books", description = "Book browsing, details and management"),
        (name = "series", description = "Series browsing, filtering and management"),
        (name = "search", description = "Full-text search across books and series"),
        (name = "reading-progress", description = "Reading progress tracking per book"),
        (name = "authors", description = "Author browsing and listing"),
        (name = "stats", description = "Collection statistics and dashboard data"),
        (name = "libraries", description = "Library listing, scanning, and management (create/delete/settings: Admin only)"),
        (name = "indexing", description = "Search index management and job control (Admin only)"),
        (name = "metadata", description = "External metadata providers and matching (Admin only)"),
        (name = "komga", description = "Komga read-status sync (Admin only)"),
        (name = "tokens", description = "API token management (Admin only)"),
        (name = "settings", description = "Application settings and cache management (Admin only)"),
        (name = "prowlarr", description = "Prowlarr indexer integration (Admin only)"),
        (name = "qbittorrent", description = "qBittorrent download client integration (Admin only)"),
        (name = "users", description = "Reader user management (Admin only)"),
        (name = "reading_status", description = "Reading status match and push jobs (Admin only)"),
        (name = "anilist", description = "AniList integration for reading status sync (Admin only)"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

pub struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "Bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some(
                            "Enter your API Bearer token (format: stl_<prefix>_<secret>)",
                        ))
                        .build(),
                ),
            );
        }
    }
}

#[derive(utoipa::ToSchema)]
pub struct ErrorResponse {
    #[allow(dead_code)]
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::OpenApi;

    #[test]
    fn test_openapi_generation() {
        let api_doc = ApiDoc::openapi();
        let json = api_doc
            .to_pretty_json()
            .expect("Failed to serialize OpenAPI");

        // Check that all $ref targets exist in components/schemas
        let doc: serde_json::Value =
            serde_json::from_str(&json).expect("OpenAPI JSON should be valid");
        let empty = serde_json::Map::new();
        let schemas = doc["components"]["schemas"]
            .as_object()
            .unwrap_or(&empty);
        let prefix = "#/components/schemas/";
        let mut broken: Vec<String> = Vec::new();
        for part in json.split(prefix).skip(1) {
            if let Some(name) = part.split('"').next() {
                if !schemas.contains_key(name) {
                    broken.push(name.to_string());
                }
            }
        }
        broken.dedup();
        assert!(broken.is_empty(), "Unresolved schema refs: {:?}", broken);

        // Save to file for inspection
        std::fs::write("/tmp/openapi.json", &json).expect("Failed to write file");
        println!("OpenAPI JSON saved to /tmp/openapi.json");
    }
}
