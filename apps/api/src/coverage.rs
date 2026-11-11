//! Database adapters for the canonical [`SeriesCoverage`] rule.
//!
//! [`stripstream_core::coverage`] owns what counts as a tome. This module reads
//! those facts from PostgreSQL once, so handlers do not re-encode the volume
//! types, the integral rule or the arithmetic.

use std::collections::HashMap;

use sqlx::{PgPool, Row};
use stripstream_core::coverage::{SeriesCoverage, COUNTED_VOLUME_TYPES, FIRST_TOME};
use uuid::Uuid;

/// `'regular', 'integral'`, ready for an `IN (...)` clause.
pub(crate) fn counted_volume_types_sql() -> String {
    COUNTED_VOLUME_TYPES
        .iter()
        .map(|volume_type| format!("'{volume_type}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The canonical `missing_counts` CTE.
///
/// `where_clause` is the predicate applied to `series` before aggregation, for
/// example `s.library_id = $1` or `s.id = $1`.
fn missing_counts_cte(where_clause: Option<&str>) -> String {
    let counted = counted_volume_types_sql();
    let where_clause = where_clause
        .map(|clause| format!("WHERE {clause}"))
        .unwrap_or_default();
    format!(
        r#"missing_counts AS (
            SELECT s.id AS series_id,
                CASE
                    WHEN COUNT(b.id) FILTER (WHERE b.volume_type = 'integral' AND b.volume IS NULL) > 0 THEN 0
                    ELSE GREATEST(
                        COALESCE(s.total_volumes, 0)
                        - COUNT(DISTINCT b.volume) FILTER (WHERE b.volume_type IN ({counted}) AND b.volume >= {FIRST_TOME})
                        - COUNT(b.id) FILTER (WHERE b.volume_type IN ({counted}) AND b.volume IS NULL),
                        0
                    )
                END AS missing_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            {where_clause}
            GROUP BY s.id
        )"#
    )
}

/// Missing counts for every series, optionally scoped to one library.
pub(crate) fn missing_counts_cte_for_library(library_id_param: Option<&str>) -> String {
    let where_clause = library_id_param.map(|param| format!("s.library_id = {param}"));
    missing_counts_cte(where_clause.as_deref())
}

/// Missing count for one series.
pub(crate) fn missing_counts_cte_for_series(series_id_param: &str) -> String {
    missing_counts_cte(Some(&format!("s.id = {series_id_param}")))
}

/// Load the coverage facts of several series at once.
///
/// Series without books are present with an empty coverage. Unknown ids are
/// absent from the map.
pub(crate) async fn load_series_coverages(
    pool: &PgPool,
    series_ids: &[Uuid],
) -> Result<HashMap<Uuid, SeriesCoverage>, sqlx::Error> {
    if series_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let counted = counted_volume_types_sql();
    let sql = format!(
        r#"
        SELECT
            s.id AS series_id,
            s.total_volumes,
            COALESCE(
                ARRAY_AGG(DISTINCT b.volume ORDER BY b.volume)
                    FILTER (WHERE b.volume_type IN ({counted}) AND b.volume >= $2),
                '{{}}'::int[]
            ) AS owned_volumes,
            (COUNT(b.id) FILTER (WHERE b.volume_type IN ({counted}) AND b.volume IS NULL))::int AS unnumbered_books,
            COUNT(b.id) FILTER (WHERE b.volume_type = 'integral' AND b.volume IS NULL) > 0 AS has_unnumbered_integral
        FROM series s
        LEFT JOIN books b ON b.series_id = s.id
        WHERE s.id = ANY($1)
        GROUP BY s.id
        "#
    );

    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(series_ids)
        .bind(FIRST_TOME)
        .fetch_all(pool)
        .await?;

    let mut coverages = HashMap::with_capacity(rows.len());
    for row in rows {
        let series_id: Uuid = row.get("series_id");
        let total_volumes: Option<i32> = row.get("total_volumes");
        let owned_volumes: Vec<i32> = row.get("owned_volumes");
        let unnumbered_books: i32 = row.get("unnumbered_books");
        let has_unnumbered_integral: bool = row.get("has_unnumbered_integral");

        coverages.insert(
            series_id,
            SeriesCoverage::from_parts(
                total_volumes,
                owned_volumes,
                unnumbered_books,
                has_unnumbered_integral,
            ),
        );
    }

    Ok(coverages)
}

/// Load the coverage facts of one series, or `None` when it does not exist.
pub(crate) async fn load_series_coverage(
    pool: &PgPool,
    series_id: Uuid,
) -> Result<Option<SeriesCoverage>, sqlx::Error> {
    Ok(load_series_coverages(pool, &[series_id])
        .await?
        .remove(&series_id))
}

/// Owned tome numbers for a series identified by its library and name.
///
/// DDL search receives a series name from the UI rather than a series id. The
/// result follows the same counted-volume rule as every other coverage read.
pub(crate) async fn load_owned_volumes_for_series_name(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
) -> Result<Vec<i32>, sqlx::Error> {
    let counted = counted_volume_types_sql();
    let sql = format!(
        r#"
        SELECT COALESCE(
            ARRAY_AGG(DISTINCT b.volume ORDER BY b.volume)
                FILTER (WHERE b.volume_type IN ({counted}) AND b.volume >= $3),
            '{{}}'::int[]
        ) AS owned_volumes
        FROM series s
        LEFT JOIN books b ON b.series_id = s.id
        WHERE s.library_id = $1 AND norm_text(s.name) = norm_text($2)
        GROUP BY s.id
        "#
    );

    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(library_id)
        .bind(series_name)
        .bind(FIRST_TOME)
        .fetch_all(pool)
        .await?;

    let mut owned: Vec<i32> = rows
        .into_iter()
        .flat_map(|row| row.get::<Vec<i32>, _>("owned_volumes"))
        .collect();
    owned.sort_unstable();
    owned.dedup();
    Ok(owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_counts_cte_uses_the_canonical_rule() {
        let cte = missing_counts_cte_for_library(Some("$1"));

        assert!(cte.contains("b.volume_type IN ('regular', 'integral')"));
        assert!(cte.contains("b.volume >= 1"));
        assert!(cte.contains("b.volume IS NULL"));
        assert!(cte.contains("b.volume_type = 'integral' AND b.volume IS NULL"));
        assert!(cte.contains("s.library_id = $1"));
    }

    #[test]
    fn missing_counts_cte_for_series_filters_on_the_series() {
        let cte = missing_counts_cte_for_series("$1");

        assert!(cte.contains("s.id = $1"));
        assert!(!cte.contains("s.library_id"));
    }
}
