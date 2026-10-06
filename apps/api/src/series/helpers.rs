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
         AND (norm_text(name) = norm_text($2) \
              OR norm_text(original_name) = norm_text($2))",
    )
    .bind(library_id)
    .bind(name)
    .fetch_optional(pool)
    .await?
    {
        return Ok(id);
    }

    let id = Uuid::new_v4();
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3) \
         ON CONFLICT (library_id, name) DO UPDATE SET name = EXCLUDED.name \
         RETURNING id",
    )
    .bind(id)
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

/// Restore metadata from archive for a newly created series, then clean up the archive record.
/// Only fills empty fields — does not overwrite existing values.
async fn restore_series_from_archive(pool: &sqlx::PgPool, library_id: Uuid, name: &str) {
    let _ = sqlx::query(
        r#"
        UPDATE series s
        SET
            description   = COALESCE(s.description, aseries.description),
            authors       = CASE WHEN s.authors = '{}' THEN aseries.authors ELSE s.authors END,
            publishers    = CASE WHEN s.publishers = '{}' THEN aseries.publishers ELSE s.publishers END,
            genres        = CASE WHEN s.genres = '{}' THEN aseries.genres ELSE s.genres END,
            total_volumes = COALESCE(s.total_volumes, aseries.total_volumes),
            status        = COALESCE(s.status, aseries.status),
            cover_url     = COALESCE(s.cover_url, aseries.cover_url),
            locked_fields = CASE WHEN s.locked_fields = '{}' THEN aseries.locked_fields ELSE s.locked_fields END,
            updated_at    = NOW()
        FROM archived_series aseries
        WHERE s.library_id = $1
          AND aseries.library_id = $1
          AND norm_text(s.name) = norm_text($2)
          AND norm_text(aseries.name) = norm_text($2)
        "#,
    )
    .bind(library_id)
    .bind(name)
    .execute(pool)
    .await;

    let _ = sqlx::query(
        "DELETE FROM archived_series WHERE library_id = $1 AND norm_text(name) = norm_text($2)",
    )
    .bind(library_id)
    .bind(name)
    .execute(pool)
    .await;
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

    // 2. Restore metadata from archive if this series was previously deleted
    restore_series_from_archive(pool, params.library_id, &params.name).await;

    // 2. Create the physical directory on disk
    if let Ok(root_path) =
        sqlx::query_scalar::<_, String>("SELECT root_path FROM libraries WHERE id = $1")
            .bind(params.library_id)
            .fetch_one(pool)
            .await
    {
        let physical_root = stripstream_core::paths::remap_libraries_path(&root_path);
        let series_dir = std::path::Path::new(&physical_root).join(&params.name);
        if !series_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&series_dir) {
                tracing::warn!(
                    "[SERIES] Failed to create directory {}: {}",
                    series_dir.display(),
                    e
                );
            }
        }
    }

    // 3. If metadata info provided, create approved link + sync
    let mut metadata_link_id = None;
    if let (Some(ref provider), Some(ref external_id)) = (&params.provider, &params.external_id) {
        let metadata_json = params
            .metadata_json
            .clone()
            .unwrap_or(serde_json::json!({}));

        let link_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO external_metadata_links
                (library_id, series_id, provider, external_id, external_url, status, confidence, metadata_json, total_volumes_external)
            VALUES ($1, $2, $3, $4, $5, 'approved', $6, $7, $8)
            ON CONFLICT (series_id, provider)
            DO UPDATE SET
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

        // Promote this link to primary if the series has none yet.
        let _ = crate::metadata::shared_sync::promote_if_no_primary(pool, series_id, link_id).await;

        // Sync the whole series from all approved links (primary + fallback).
        let _ = crate::metadata::sync_series_from_links(pool, series_id, true, true).await;

        // Override series.cover_url with the provider's tome 1 cover (more authoritative
        // than the discovery thumbnail that may have been sent in the request).
        let _ = sqlx::query(
            "UPDATE series
             SET cover_url = (
                 SELECT cover_url FROM external_book_metadata
                 WHERE link_id = $1
                   AND cover_url IS NOT NULL AND cover_url != ''
                 ORDER BY volume_number NULLS LAST, id
                 LIMIT 1
             )
             WHERE id = $2
               AND (locked_fields->>'cover_url')::boolean IS NOT TRUE
               AND EXISTS (
                   SELECT 1 FROM external_book_metadata
                   WHERE link_id = $1
                     AND cover_url IS NOT NULL AND cover_url != ''
               )",
        )
        .bind(link_id)
        .bind(series_id)
        .execute(pool)
        .await;
    }

    Ok(CreateSeriesResult {
        series_id,
        metadata_link_id,
    })
}

/// Build the `missing_counts` CTE SQL fragment.
/// `library_id_param`: e.g. "$1" to filter by library, or None for all libraries.
pub(crate) fn build_missing_counts_cte(library_id_param: Option<&str>) -> String {
    let where_clause = match library_id_param {
        Some(p) => format!("WHERE s.library_id = {p}"),
        None => String::new(),
    };
    format!(
        r#"missing_counts AS (
            SELECT s.id as series_id,
                CASE
                    WHEN COUNT(b.id) FILTER (WHERE b.volume_type = 'integral' AND b.volume IS NULL) > 0 THEN 0
                    ELSE GREATEST(
                        COALESCE(s.total_volumes, 0)
                        - COUNT(DISTINCT b.volume) FILTER (WHERE b.volume_type IN ('regular', 'integral') AND b.volume IS NOT NULL)
                        - COUNT(b.id) FILTER (WHERE b.volume_type IN ('regular', 'integral') AND b.volume IS NULL),
                        0
                    )
                END as missing_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            {where_clause}
            GROUP BY s.id
        )"#
    )
}
