//! Shared persistence for the `available_downloads` table.
//!
//! Prowlarr detection, the RSS poll and the DDL provider search all write the
//! same shape: one row per series, whose releases are *merged* with the ones
//! already stored (so a run never drops releases another source found). This
//! module owns that SQL so the merge semantics stay identical everywhere.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::detection::AvailableReleaseDto;

/// Upserts a series' releases, merging them with the ones already stored.
pub(crate) async fn upsert_releases(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Uuid,
    missing_count: i32,
    releases: &[AvailableReleaseDto],
) -> Result<(), sqlx::Error> {
    let releases_json = serde_json::to_value(releases).unwrap_or_else(|_| Value::Array(Vec::new()));

    sqlx::query(
        "INSERT INTO available_downloads (library_id, series_id, missing_count, available_releases, updated_at) \
         VALUES ($1, $2, $3, $4, NOW()) \
         ON CONFLICT (series_id) DO UPDATE SET \
           missing_count = EXCLUDED.missing_count, \
           available_releases = merge_releases(available_downloads.available_releases, EXCLUDED.available_releases), \
           updated_at = NOW()",
    )
    .bind(library_id)
    .bind(series_id)
    .bind(missing_count)
    .bind(releases_json)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Updates only the stored `missing_count` for a series, leaving its releases
/// untouched. Used when a run found nothing, so previous results survive.
pub(crate) async fn update_missing_count(
    pool: &PgPool,
    series_id: Uuid,
    missing_count: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE available_downloads SET missing_count = $2, updated_at = NOW() WHERE series_id = $1",
    )
    .bind(series_id)
    .bind(missing_count)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Deletes entries that no longer hold any release, optionally scoped to a
/// single library.
pub(crate) async fn delete_empty(
    pool: &PgPool,
    library_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    let query = match library_id {
        Some(_) => {
            "DELETE FROM available_downloads \
             WHERE library_id = $1 AND (available_releases = '[]'::jsonb OR available_releases IS NULL)"
        }
        None => {
            "DELETE FROM available_downloads \
             WHERE available_releases = '[]'::jsonb OR available_releases IS NULL"
        }
    };

    let mut q = sqlx::query(query);
    if let Some(id) = library_id {
        q = q.bind(id);
    }
    q.execute(pool).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(title: &str) -> AvailableReleaseDto {
        AvailableReleaseDto {
            title: title.to_string(),
            size: 1,
            download_url: None,
            indexer: None,
            seeders: None,
            matched_missing_volumes: vec![],
            all_volumes: vec![],
            has_failed: false,
            detected_at: None,
            match_confidence: "high".to_string(),
            match_reasons: vec![],
            provider: None,
            external_id: None,
        }
    }

    async fn seed_library(pool: &PgPool, name: &str) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind(name)
            .bind(format!("/libraries/{name}"))
            .execute(pool)
            .await
            .unwrap();
        library_id
    }

    async fn seed_series(pool: &PgPool, library_id: Uuid, name: &str) -> Uuid {
        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        series_id
    }

    async fn stored_titles(pool: &PgPool, series_id: Uuid) -> Vec<String> {
        let value: serde_json::Value = sqlx::query_scalar(
            "SELECT available_releases FROM available_downloads WHERE series_id = $1",
        )
        .bind(series_id)
        .fetch_one(pool)
        .await
        .unwrap();
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["title"].as_str().unwrap().to_string())
            .collect()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn upsert_merges_new_releases_with_stored_ones(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;

        upsert_releases(&pool, library_id, series_id, 2, &[release("T01")])
            .await
            .unwrap();
        upsert_releases(&pool, library_id, series_id, 1, &[release("T02")])
            .await
            .unwrap();

        let mut titles = stored_titles(&pool, series_id).await;
        titles.sort();
        assert_eq!(titles, vec!["T01", "T02"]);

        let missing: i32 = sqlx::query_scalar(
            "SELECT missing_count FROM available_downloads WHERE series_id = $1",
        )
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(missing, 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn upsert_dedups_on_title(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;

        upsert_releases(&pool, library_id, series_id, 2, &[release("T01")])
            .await
            .unwrap();
        upsert_releases(&pool, library_id, series_id, 2, &[release("T01")])
            .await
            .unwrap();

        assert_eq!(stored_titles(&pool, series_id).await, vec!["T01"]);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn update_missing_count_keeps_stored_releases(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;

        upsert_releases(&pool, library_id, series_id, 3, &[release("T01")])
            .await
            .unwrap();
        update_missing_count(&pool, series_id, 1).await.unwrap();

        assert_eq!(stored_titles(&pool, series_id).await, vec!["T01"]);
        let missing: i32 = sqlx::query_scalar(
            "SELECT missing_count FROM available_downloads WHERE series_id = $1",
        )
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(missing, 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn delete_empty_can_be_scoped_to_a_library(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        let other_library = seed_library(&pool, "other").await;
        let other_series = seed_series(&pool, other_library, "Tintin").await;

        for (lib, series) in [(library_id, series_id), (other_library, other_series)] {
            sqlx::query(
                "INSERT INTO available_downloads (library_id, series_id, missing_count, available_releases, updated_at) \
                 VALUES ($1, $2, 0, '[]'::jsonb, NOW())",
            )
            .bind(lib)
            .bind(series)
            .execute(&pool)
            .await
            .unwrap();
        }

        delete_empty(&pool, Some(library_id)).await.unwrap();
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM available_downloads")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 1, "only the scoped library should be cleaned");

        delete_empty(&pool, None).await.unwrap();
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM available_downloads")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 0);
    }
}
