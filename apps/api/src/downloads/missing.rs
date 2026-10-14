use std::collections::HashMap;

use sqlx::{PgPool, Row};
use uuid::Uuid;

pub(super) struct LinkMissingVolumes {
    pub missing_volumes: Vec<i32>,
    pub missing_count: i32,
}

pub(super) struct SeriesAvailability {
    pub owned_volumes: Vec<i32>,
    pub missing_count: i32,
    pub has_integral: bool,
}

pub(super) async fn load_link_missing_volumes(
    pool: &PgPool,
    link_id: Uuid,
) -> Result<LinkMissingVolumes, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT ebm.volume_number
        FROM external_book_metadata ebm
        JOIN external_metadata_links eml ON eml.id = ebm.link_id
        WHERE ebm.link_id = $1
          AND ebm.book_id IS NULL
          AND (ebm.volume_number IS NULL OR ebm.volume_number != 0)
          AND NOT EXISTS (
              SELECT 1 FROM books b
              WHERE b.series_id = eml.series_id
                AND b.volume_type = 'integral'
          )
        ORDER BY ebm.volume_number NULLS LAST
        "#,
    )
    .bind(link_id)
    .fetch_all(pool)
    .await?;

    let missing_volumes = rows
        .iter()
        .filter_map(|row| row.get::<Option<i32>, _>("volume_number"))
        .filter(|&volume| volume > 0)
        .collect();

    Ok(LinkMissingVolumes {
        missing_volumes,
        missing_count: rows.len() as i32,
    })
}

pub(super) async fn load_series_availability(
    pool: &PgPool,
    series_ids: &[Uuid],
) -> Result<HashMap<Uuid, SeriesAvailability>, sqlx::Error> {
    if series_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            s.id AS series_id,
            COALESCE(s.total_volumes, 0) AS total_volumes,
            COALESCE(
                ARRAY_AGG(DISTINCT b.volume ORDER BY b.volume)
                    FILTER (WHERE b.volume_type = 'regular' AND b.volume IS NOT NULL),
                ARRAY[]::int[]
            ) AS owned_volumes,
            COUNT(b.id) FILTER (WHERE b.volume_type = 'integral') > 0 AS has_integral
        FROM series s
        LEFT JOIN books b ON b.series_id = s.id
        WHERE s.id = ANY($1)
        GROUP BY s.id
        "#,
    )
    .bind(series_ids)
    .fetch_all(pool)
    .await?;

    let mut availability = HashMap::new();
    for row in rows {
        let series_id: Uuid = row.get("series_id");
        let total_volumes: i32 = row.get("total_volumes");
        let owned_volumes: Vec<i32> = row.get("owned_volumes");
        let has_integral: bool = row.get("has_integral");
        let missing_count = if has_integral {
            0
        } else {
            (total_volumes - owned_volumes.len() as i32).max(0)
        };

        availability.insert(
            series_id,
            SeriesAvailability {
                owned_volumes,
                missing_count,
                has_integral,
            },
        );
    }

    Ok(availability)
}

/// A series carrying an approved metadata link, with the link to resolve its
/// missing volumes from.
pub(super) struct ApprovedLink {
    pub library_id: Uuid,
    pub series_id: Uuid,
    pub series_name: String,
    pub link_id: Uuid,
}

/// Which approved links to load.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LinkScope {
    /// Every approved link, one row per series. The DDL refresh needs complete
    /// series too, so it can strip stale offers.
    All,
    /// Only series that still have unlinked external metadata and no integral
    /// volume — the perimeter the Prowlarr detection and the RSS poll match
    /// releases against.
    WithMissingVolumes,
}

/// Every series with an approved metadata link, one row per series (primary
/// link first). Pass a library id to scope the list, or `None` for all
/// libraries — the Prowlarr detection searches per library, the DDL search
/// globally.
pub(super) async fn load_approved_links(
    pool: &PgPool,
    library_id: Option<Uuid>,
    scope: LinkScope,
) -> Result<Vec<ApprovedLink>, sqlx::Error> {
    // `$2` toggles the perimeter: `false` (All) keeps every approved link,
    // `true` (WithMissingVolumes) keeps only series whose *chosen* link has
    // unlinked metadata and that have no integral volume. The filter is applied
    // after the lateral join, so it looks at the primary link only.
    let rows = sqlx::query(
        r#"
        SELECT s.id           AS series_id,
               s.name         AS series_name,
               eml.id         AS link_id,
               eml.library_id AS library_id
        FROM series s
        JOIN LATERAL (
            SELECT id, library_id FROM external_metadata_links
            WHERE series_id = s.id
              AND ($1::uuid IS NULL OR library_id = $1)
              AND status = 'approved'
            ORDER BY is_primary DESC, approved_at ASC NULLS LAST, id ASC
            LIMIT 1
        ) eml ON TRUE
        WHERE NOT $2
           OR (
                EXISTS (
                    SELECT 1 FROM external_book_metadata ebm
                    WHERE ebm.link_id = eml.id AND ebm.book_id IS NULL
                )
                AND NOT EXISTS (
                    SELECT 1 FROM books b
                    WHERE b.series_id = s.id AND b.volume_type = 'integral'
                )
              )
        ORDER BY s.name
        "#,
    )
    .bind(library_id)
    .bind(matches!(scope, LinkScope::WithMissingVolumes))
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ApprovedLink {
            library_id: row.get("library_id"),
            series_id: row.get("series_id"),
            series_name: row.get("series_name"),
            link_id: row.get("link_id"),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed_library(pool: &PgPool, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(name)
            .bind(format!("/libraries/{name}"))
            .execute(pool)
            .await
            .unwrap();
        id
    }

    async fn seed_series(pool: &PgPool, library_id: Uuid, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(library_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        id
    }

    async fn seed_link(
        pool: &PgPool,
        library_id: Uuid,
        series_id: Uuid,
        provider: &str,
        is_primary: bool,
        approved_at: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO external_metadata_links \
                (id, library_id, series_id, provider, external_id, status, is_primary, approved_at) \
             VALUES ($1, $2, $3, $4, $5, 'approved', $6, $7::timestamptz)",
        )
        .bind(id)
        .bind(library_id)
        .bind(series_id)
        .bind(provider)
        .bind(format!("ext:{id}"))
        .bind(is_primary)
        .bind(approved_at)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn seed_unlinked_volume(pool: &PgPool, link_id: Uuid, volume: i32) {
        sqlx::query(
            "INSERT INTO external_book_metadata \
                (id, link_id, external_book_id, title, volume_number, book_id) \
             VALUES (gen_random_uuid(), $1, $2, $3, $4, NULL)",
        )
        .bind(link_id)
        .bind(format!("ebm:{link_id}:{volume}"))
        .bind(format!("Volume {volume}"))
        .bind(volume)
        .execute(pool)
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn load_approved_links_prefers_primary_and_can_require_missing_volumes(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;

        // S1: two approved links; the primary one carries the missing volume.
        let s1 = seed_series(&pool, library_id, "S1").await;
        seed_link(&pool, library_id, s1, "secondary", false, "2024-01-01").await;
        let primary = seed_link(&pool, library_id, s1, "primary", true, "2024-02-01").await;
        seed_unlinked_volume(&pool, primary, 1).await;

        // S2: approved link but every volume is already linked → complete.
        let s2 = seed_series(&pool, library_id, "S2").await;
        let s2_link = seed_link(&pool, library_id, s2, "test", true, "2024-01-01").await;
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, series_id, title, kind, format) \
             VALUES ($1, $2, $3, 'Vol 1', 'comic', 'cbz')",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(s2)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO external_book_metadata \
                (id, link_id, external_book_id, title, volume_number, book_id) \
             VALUES (gen_random_uuid(), $1, 's2v1', 'Volume 1', 1, $2)",
        )
        .bind(s2_link)
        .bind(book_id)
        .execute(&pool)
        .await
        .unwrap();

        // S3: integral series with an unlinked volume → excluded by the missing scope.
        let s3 = seed_series(&pool, library_id, "S3").await;
        let s3_link = seed_link(&pool, library_id, s3, "test", true, "2024-01-01").await;
        seed_unlinked_volume(&pool, s3_link, 1).await;
        sqlx::query(
            "INSERT INTO books (id, library_id, series_id, title, kind, format, volume_type) \
             VALUES ($1, $2, $3, 'Integral', 'comic', 'cbz', 'integral')",
        )
        .bind(Uuid::new_v4())
        .bind(library_id)
        .bind(s3)
        .execute(&pool)
        .await
        .unwrap();

        let all = load_approved_links(&pool, None, LinkScope::All)
            .await
            .unwrap();
        assert_eq!(all.len(), 3, "one row per series");
        assert_eq!(
            all.iter()
                .find(|link| link.series_id == s1)
                .unwrap()
                .link_id,
            primary,
            "the primary approved link wins"
        );

        let with_missing = load_approved_links(&pool, None, LinkScope::WithMissingVolumes)
            .await
            .unwrap();
        let ids: Vec<Uuid> = with_missing.iter().map(|link| link.series_id).collect();
        assert!(ids.contains(&s1), "a series with a missing volume is kept");
        assert!(!ids.contains(&s2), "a complete series is excluded");
        assert!(
            !ids.contains(&s3),
            "a series with an integral volume is excluded"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn load_approved_links_scopes_to_a_library(pool: PgPool) {
        let l1 = seed_library(&pool, "lib1").await;
        let l2 = seed_library(&pool, "lib2").await;
        let s1 = seed_series(&pool, l1, "S1").await;
        let s2 = seed_series(&pool, l2, "S2").await;
        seed_link(&pool, l1, s1, "test", true, "2024-01-01").await;
        seed_link(&pool, l2, s2, "test", true, "2024-01-01").await;

        let scoped = load_approved_links(&pool, Some(l1), LinkScope::All)
            .await
            .unwrap();
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].series_id, s1);

        let all = load_approved_links(&pool, None, LinkScope::All)
            .await
            .unwrap();
        assert_eq!(all.len(), 2);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn link_missing_volumes_are_empty_when_series_has_integral(pool: PgPool) {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Test Lib', '/libraries/test')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name, total_volumes) VALUES ($1, $2, 'Integral Series', 3)")
            .bind(series_id)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO books (id, library_id, series_id, title, kind, format, volume_type) \
             VALUES ($1, $2, $3, 'Integral', 'comic', 'cbz', 'integral')",
        )
        .bind(Uuid::new_v4())
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        let link_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO external_metadata_links (id, library_id, series_id, provider, external_id, status) \
             VALUES ($1, $2, $3, 'test', 'ext:1', 'approved')",
        )
        .bind(link_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        for volume in 1..=3 {
            sqlx::query(
                "INSERT INTO external_book_metadata (id, link_id, external_book_id, title, volume_number, book_id) \
                 VALUES (gen_random_uuid(), $1, $2, $3, $4, NULL)",
            )
            .bind(link_id)
            .bind(format!("ext_{volume}"))
            .bind(format!("Volume {volume}"))
            .bind(volume)
            .execute(&pool)
            .await
            .unwrap();
        }

        let missing = load_link_missing_volumes(&pool, link_id).await.unwrap();

        assert!(missing.missing_volumes.is_empty());
        assert_eq!(missing.missing_count, 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn series_availability_marks_integral_series_complete(pool: PgPool) {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Test Lib', '/libraries/test')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name, total_volumes) VALUES ($1, $2, 'Integral Series', 5)")
            .bind(series_id)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO books (id, library_id, series_id, title, kind, format, volume_type) \
             VALUES ($1, $2, $3, 'Integral', 'comic', 'cbz', 'integral')",
        )
        .bind(Uuid::new_v4())
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        let availability = load_series_availability(&pool, &[series_id]).await.unwrap();
        let series = availability.get(&series_id).unwrap();

        assert_eq!(series.missing_count, 0);
        assert!(series.has_integral);
        assert!(series.owned_volumes.is_empty());
    }
}
