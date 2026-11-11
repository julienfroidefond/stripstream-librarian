//! Canonical series identity: one normalized key for every series-name lookup.
//!
//! The `series` table stores the display name; matching uses the SQL
//! `norm_text` function (lowercase + unaccent), which backs the
//! `(library_id, norm_text(name))` index. Callers must pass raw names to this
//! module and never compare `LOWER(...)` or `norm_text(...)` by hand. The
//! database normalizes the raw side, so Rust and SQL cannot drift on accents.
//!
//! Use [`find`] / [`find_id`] for a single name, [`find_many`] for one library,
//! [`resolve_many`] for a batch spanning libraries, and [`existing_names`] to
//! filter a candidate list against every library.

use std::collections::{HashMap, HashSet};

use sqlx::{PgPool, Row};
use uuid::Uuid;

/// A resolved series row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesIdentity {
    pub id: Uuid,
    pub library_id: Uuid,
    pub name: String,
}

/// Resolve one raw series name inside a library.
pub async fn find(
    pool: &PgPool,
    library_id: Uuid,
    raw_name: &str,
) -> sqlx::Result<Option<SeriesIdentity>> {
    let row = sqlx::query(
        "SELECT id, library_id, name FROM series \
         WHERE library_id = $1 AND norm_text(name) = norm_text($2)",
    )
    .bind(library_id)
    .bind(raw_name)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(identity_from_row))
}

/// Resolve a raw series name to its id inside a library.
pub async fn find_id(
    pool: &PgPool,
    library_id: Uuid,
    raw_name: &str,
) -> sqlx::Result<Option<Uuid>> {
    Ok(find(pool, library_id, raw_name)
        .await?
        .map(|identity| identity.id))
}

/// Resolve many raw names inside one library in a single round trip.
///
/// The map is keyed by the raw name the caller supplied, so two spellings that
/// normalize to the same series both resolve to the same identity.
pub async fn find_many(
    pool: &PgPool,
    library_id: Uuid,
    raw_names: &[String],
) -> sqlx::Result<HashMap<String, SeriesIdentity>> {
    let requests: Vec<(Uuid, String)> = raw_names
        .iter()
        .map(|name| (library_id, name.clone()))
        .collect();
    let resolved = resolve_many(pool, &requests).await?;

    Ok(resolved
        .into_iter()
        .map(|((_, raw_name), identity)| (raw_name, identity))
        .collect())
}

/// Resolve a batch of `(library, raw name)` pairs in a single round trip.
///
/// This is the library-scoped batch lookup: the same raw name can resolve to
/// different series in different libraries. The map is keyed by the exact pair
/// the caller supplied.
pub async fn resolve_many(
    pool: &PgPool,
    requests: &[(Uuid, String)],
) -> sqlx::Result<HashMap<(Uuid, String), SeriesIdentity>> {
    if requests.is_empty() {
        return Ok(HashMap::new());
    }

    let library_ids: Vec<Uuid> = requests.iter().map(|(library_id, _)| *library_id).collect();
    let raw_names: Vec<String> = requests
        .iter()
        .map(|(_, raw_name)| raw_name.clone())
        .collect();

    let rows = sqlx::query(
        "SELECT q.library_id, q.raw_name, s.id, s.library_id, s.name \
         FROM UNNEST($1::uuid[], $2::text[]) AS q(library_id, raw_name) \
         JOIN series s ON s.library_id = q.library_id \
                      AND norm_text(s.name) = norm_text(q.raw_name)",
    )
    .bind(&library_ids)
    .bind(&raw_names)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let library_id: Uuid = row.get("library_id");
            let raw_name: String = row.get("raw_name");
            ((library_id, raw_name), identity_from_row(row))
        })
        .collect())
}

/// Return the raw candidate names that already exist in any library.
///
/// Discovery suggestions arrive with provider spellings (`Astérix`,
/// `Asterix`, ...); this keeps the raw name in the result so the caller can
/// filter its own list without recomputing a key.
pub async fn existing_names(pool: &PgPool, raw_names: &[String]) -> sqlx::Result<HashSet<String>> {
    if raw_names.is_empty() {
        return Ok(HashSet::new());
    }

    let rows = sqlx::query(
        "SELECT DISTINCT q.raw_name \
         FROM UNNEST($1::text[]) AS q(raw_name) \
         JOIN series s ON norm_text(s.name) = norm_text(q.raw_name)",
    )
    .bind(raw_names)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("raw_name"))
        .collect())
}

fn identity_from_row(row: sqlx::postgres::PgRow) -> SeriesIdentity {
    SeriesIdentity {
        id: row.get("id"),
        library_id: row.get("library_id"),
        name: row.get("name"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed_library(pool: &PgPool, name: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO libraries (id, name, root_path) \
             VALUES (gen_random_uuid(), $1, $2) RETURNING id",
        )
        .bind(name)
        .bind(format!("/libraries/{name}"))
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn seed_series(pool: &PgPool, library_id: Uuid, name: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO series (id, library_id, name) \
             VALUES (gen_random_uuid(), $1, $2) RETURNING id",
        )
        .bind(library_id)
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn find_matches_case_and_accents(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;

        for name in ["Astérix", "asterix", "ASTÉRIX"] {
            let identity = find(&pool, library_id, name).await.unwrap().unwrap();
            assert_eq!(identity.id, series_id, "raw name {name} must resolve");
            assert_eq!(identity.name, "Astérix");
        }
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn find_is_scoped_to_the_library(pool: PgPool) {
        let first = seed_library(&pool, "first").await;
        let second = seed_library(&pool, "second").await;
        let first_series = seed_series(&pool, first, "Astérix").await;
        seed_series(&pool, second, "Astérix").await;

        assert_eq!(
            find_id(&pool, first, "Asterix").await.unwrap(),
            Some(first_series)
        );
        assert_eq!(find_id(&pool, first, "Tintin").await.unwrap(), None);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn find_many_keys_by_the_raw_name(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "Astérix").await;
        let names = vec!["Asterix".to_string(), "Tintin".to_string()];

        let resolved = find_many(&pool, library_id, &names).await.unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved["Asterix"].id, series_id);
        assert!(!resolved.contains_key("Tintin"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn resolve_many_spans_libraries(pool: PgPool) {
        let first = seed_library(&pool, "first").await;
        let second = seed_library(&pool, "second").await;
        let first_series = seed_series(&pool, first, "Astérix").await;
        let second_series = seed_series(&pool, second, "Astérix").await;

        let requests = vec![
            (first, "Asterix".to_string()),
            (second, "ASTÉRIX".to_string()),
        ];
        let resolved = resolve_many(&pool, &requests).await.unwrap();

        assert_eq!(resolved[&(first, "Asterix".to_string())].id, first_series);
        assert_eq!(resolved[&(second, "ASTÉRIX".to_string())].id, second_series);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn existing_names_returns_the_raw_candidate(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        seed_series(&pool, library_id, "Astérix").await;
        let names = vec!["Asterix".to_string(), "Tintin".to_string()];

        let existing = existing_names(&pool, &names).await.unwrap();

        assert!(existing.contains("Asterix"));
        assert!(!existing.contains("Tintin"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn empty_batches_do_not_query_the_database(pool: PgPool) {
        assert!(find_many(&pool, Uuid::new_v4(), &[])
            .await
            .unwrap()
            .is_empty());
        assert!(resolve_many(&pool, &[]).await.unwrap().is_empty());
        assert!(existing_names(&pool, &[]).await.unwrap().is_empty());
    }
}
