use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::books::list_books,
        crate::books::get_book,
        crate::books::list_series,
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
        crate::tokens::list_tokens,
        crate::tokens::create_token,
        crate::tokens::revoke_token,
    ),
    components(
        schemas(
            crate::books::ListBooksQuery,
            crate::books::BookItem,
            crate::books::BooksPage,
            crate::books::BookDetails,
            crate::books::SeriesItem,
            crate::pages::PageQuery,
            crate::search::SearchQuery,
            crate::search::SearchResponse,
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
            crate::tokens::CreateTokenRequest,
            crate::tokens::TokenResponse,
            crate::tokens::CreatedTokenResponse,
            ErrorResponse,
        )
    ),
    security(
        ("Bearer" = [])
    ),
    tags(
        (name = "books", description = "Read-only endpoints for browsing and searching books"),
        (name = "libraries", description = "Library management endpoints (Admin only)"),
        (name = "indexing", description = "Search index management and job control (Admin only)"),
        (name = "tokens", description = "API token management (Admin only)"),
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

        // Check that there are no references to non-existent schemas
        assert!(
            !json.contains("\"/components/schemas/Uuid\""),
            "Uuid schema should not be referenced"
        );
        assert!(
            !json.contains("\"/components/schemas/DateTime\""),
            "DateTime schema should not be referenced"
        );

        // Save to file for inspection
        std::fs::write("/tmp/openapi.json", &json).expect("Failed to write file");
        println!("OpenAPI JSON saved to /tmp/openapi.json");
    }
}
