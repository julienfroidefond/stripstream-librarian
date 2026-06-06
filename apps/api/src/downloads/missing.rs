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

#[cfg(test)]
mod tests {
    use super::*;

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
