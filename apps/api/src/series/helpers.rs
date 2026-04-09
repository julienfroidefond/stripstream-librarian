use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

/// Get or create a series row, returning its UUID.
/// Also checks `original_name` to prevent duplicates after user renames.
pub(crate) async fn get_or_create_series(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    name: &str,
) -> Result<Uuid, ApiError> {
    // Try to find existing by current name OR original_name (prevents duplicates after rename)
    if let Some(id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM series WHERE library_id = $1 \
         AND (LOWER(unaccent(name)) = LOWER(unaccent($2)) \
              OR LOWER(unaccent(original_name)) = LOWER(unaccent($2)))",
    )
    .bind(library_id)
    .bind(name)
    .fetch_optional(pool)
    .await?
    {
        return Ok(id);
    }

    // Create new
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3) \
         ON CONFLICT (library_id, name) DO UPDATE SET name = EXCLUDED.name \
         RETURNING id",
    )
    .bind(id)
    .bind(library_id)
    .bind(name)
    .execute(pool)
    .await?;

    // Re-fetch in case of conflict (ON CONFLICT won't return the existing id via execute)
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM series WHERE library_id = $1 AND LOWER(unaccent(name)) = LOWER(unaccent($2))",
    )
    .bind(library_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

/// Resolve the library_id for a given series UUID.
pub(crate) async fn resolve_library_id(
    pool: &sqlx::PgPool,
    series_id: Uuid,
) -> Result<Uuid, ApiError> {
    sqlx::query_scalar::<_, Uuid>("SELECT library_id FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("series not found"))
}

/// Parameters for creating a series with optional metadata linking.
pub(crate) struct CreateSeriesParams {
    pub library_id: Uuid,
    pub name: String,
    pub provider: Option<String>,
    pub external_id: Option<String>,
    pub external_url: Option<String>,
    pub confidence: Option<f32>,
    pub total_volumes: Option<i32>,
    pub metadata_json: Option<serde_json::Value>,
}

/// Result of creating a series.
pub(crate) struct CreateSeriesResult {
    pub series_id: Uuid,
    pub metadata_link_id: Option<Uuid>,
}

/// Create a series, optionally link metadata, sync series + book metadata.
/// Shared logic used by both `POST /series/create` and Discovery `add_to_library`.
pub(crate) async fn create_series_with_metadata(
    state: &AppState,
    params: CreateSeriesParams,
) -> Result<CreateSeriesResult, ApiError> {
    let pool = &state.pool;

    // 1. Create or find the series
    let series_id = get_or_create_series(pool, params.library_id, &params.name).await?;

    // 2. Create the physical directory on disk
    if let Ok(root_path) = sqlx::query_scalar::<_, String>("SELECT root_path FROM libraries WHERE id = $1")
        .bind(params.library_id)
        .fetch_one(pool)
        .await
    {
        let physical_root = stripstream_core::paths::remap_libraries_path(&root_path);
        let series_dir = std::path::Path::new(&physical_root).join(&params.name);
        if !series_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&series_dir) {
                tracing::warn!("[SERIES] Failed to create directory {}: {}", series_dir.display(), e);
            }
        }
    }

    // 3. If metadata info provided, create approved link + sync
    let mut metadata_link_id = None;
    if let (Some(ref provider), Some(ref external_id)) = (&params.provider, &params.external_id) {
        let metadata_json = params.metadata_json.clone().unwrap_or(serde_json::json!({}));

        let link_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO external_metadata_links
                (library_id, series_id, provider, external_id, external_url, status, confidence, metadata_json, total_volumes_external)
            VALUES ($1, $2, $3, $4, $5, 'approved', $6, $7, $8)
            ON CONFLICT (series_id, provider)
            DO UPDATE SET
                external_id = EXCLUDED.external_id,
                external_url = EXCLUDED.external_url,
                status = 'approved',
                confidence = EXCLUDED.confidence,
                metadata_json = EXCLUDED.metadata_json,
                total_volumes_external = EXCLUDED.total_volumes_external,
                matched_at = NOW(),
                approved_at = NOW(),
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(params.library_id)
        .bind(series_id)
        .bind(provider)
        .bind(external_id)
        .bind(&params.external_url)
        .bind(params.confidence)
        .bind(&metadata_json)
        .bind(params.total_volumes)
        .fetch_one(pool)
        .await?;

        metadata_link_id = Some(link_id);

        // Sync series metadata
        let _ = crate::metadata::sync_series_metadata(
            state, params.library_id, &params.name, &metadata_json, params.total_volumes,
        )
        .await;

        // Sync book metadata
        let _ = crate::metadata::sync_books_metadata(
            state, link_id, params.library_id, &params.name, provider, external_id,
        )
        .await;
    }

    Ok(CreateSeriesResult {
        series_id,
        metadata_link_id,
    })
}

/// Build the `missing_counts` CTE SQL fragment.
/// `library_id_param`: e.g. "$1" to filter by library, or None for all libraries.
pub(super) fn build_missing_counts_cte(library_id_param: Option<&str>) -> String {
    let where_clause = match library_id_param {
        Some(p) => format!("WHERE s.library_id = {p}"),
        None => String::new(),
    };
    format!(
        r#"missing_counts AS (
            SELECT s.id as series_id,
                GREATEST(COALESCE(s.total_volumes, 0) - COUNT(b.id) FILTER (WHERE b.volume_type = 'regular'), 0) as missing_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            {where_clause}
            GROUP BY s.id
        )"#
    )
}

/// The `metadata_links` CTE SQL fragment (approved links, latest per series+library).
pub(super) const METADATA_LINKS_CTE: &str = r#"metadata_links AS (
            SELECT DISTINCT ON (eml.series_id, eml.library_id)
                eml.series_id, eml.library_id, eml.provider
            FROM external_metadata_links eml
            WHERE eml.status = 'approved'
            ORDER BY eml.series_id, eml.library_id, eml.created_at DESC
        )"#;
