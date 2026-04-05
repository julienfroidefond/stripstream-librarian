use uuid::Uuid;

use crate::error::ApiError;

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
                GREATEST(COALESCE(s.total_volumes, 0) - COUNT(b.id), 0) as missing_count
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
