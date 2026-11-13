//! Canonical series identity: one normalized key for every series-name lookup.
//!
//! The `series` table stores the display name; matching uses the SQL
//! `norm_text` function (lowercase + unaccent), which backs the
//! `(library_id, norm_text(name))` and `(library_id, norm_text(original_name))`
//! indexes. Callers must pass raw names to this module and never compare
//! `LOWER(...)` or `norm_text(...)` by hand. The database normalizes the raw
//! side, so Rust and SQL cannot drift on accents.
//!
//! # Identity rule
//!
//! A raw name designates the series of the library whose `norm_text(name)`
//! **or** `norm_text(original_name)` equals `norm_text(raw)`. `original_name`
//! keeps the filesystem name after a user rename, so the scanner, the API and
//! the download pipeline keep resolving the old folder name to the renamed
//! series instead of recreating it. When several rows match, a match on the
//! current `name` wins over a match on `original_name`, then the oldest row.
//!
//! Use [`find`] / [`find_id`] for a single name, [`find_or_create`] to resolve
//! or insert one, [`find_many`] for one library, [`resolve_many`] for a batch
//! spanning libraries, and [`existing_names`] to filter a candidate list
//! against every library.

use std::collections::{HashMap, HashSet};

use sqlx::{Acquire, PgPool, Postgres, Row};
use uuid::Uuid;

/// A resolved series row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesIdentity {
    pub id: Uuid,
    pub library_id: Uuid,
    pub name: String,
}

/// Point lookup for the identity rule: `$1` library, `$2` raw name. A match on
/// the current name wins over `original_name`, then the oldest row.
const FIND_SQL: &str = "SELECT id, library_id, name FROM series \
     WHERE library_id = $1 \
       AND (norm_text(name) = norm_text($2) OR norm_text(original_name) = norm_text($2)) \
     ORDER BY (norm_text(name) = norm_text($2)) DESC, created_at, id \
     LIMIT 1";

/// Resolve one raw series name inside a library (see the identity rule).
pub async fn find(
    pool: &PgPool,
    library_id: Uuid,
    raw_name: &str,
) -> sqlx::Result<Option<SeriesIdentity>> {
    let row = sqlx::query(FIND_SQL)
        .bind(library_id)
        .bind(raw_name)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(identity_from_row))
}

/// Resolve a raw series name inside a library, creating the series when no row
/// matches the identity rule. The new row keeps `raw_name` as display name.
///
/// Creation is serialized per `(library, norm_text(raw_name))` with a
/// transaction-scoped advisory lock, then the lookup is repeated under the
/// lock: two concurrent calls for `One Piece` and `one piece` resolve to the
/// same row even though the unique constraint only covers the exact name.
/// Pass a pool, a connection or an open transaction; inside a transaction the
/// lock is held until the caller commits.
pub async fn find_or_create<'c, A>(
    conn: A,
    library_id: Uuid,
    raw_name: &str,
) -> sqlx::Result<SeriesIdentity>
where
    A: Acquire<'c, Database = Postgres>,
{
    Ok(find_or_create_tracked(conn, library_id, raw_name)
        .await?
        .identity)
}

/// Result of [`find_or_create_tracked`]: the series and whether this call inserted it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundOrCreated {
    pub identity: SeriesIdentity,
    /// `true` only when this call inserted the row; a match on another spelling or on
    /// `original_name` is a lookup, not a creation.
    pub created: bool,
}

/// [`find_or_create`], also reporting whether the series was created. Use it when
/// "new series" matters (scan notifications): comparing raw names would count
/// `one piece` as new next to `One Piece`.
pub async fn find_or_create_tracked<'c, A>(
    conn: A,
    library_id: Uuid,
    raw_name: &str,
) -> sqlx::Result<FoundOrCreated>
where
    A: Acquire<'c, Database = Postgres>,
{
    let mut tx = conn.begin().await?;

    sqlx::query(
        "SELECT pg_advisory_xact_lock(\
             hashtextextended('series_identity:' || $1::text || ':' || norm_text($2), 0))",
    )
    .bind(library_id)
    .bind(raw_name)
    .execute(&mut *tx)
    .await?;

    let existing = sqlx::query(FIND_SQL)
        .bind(library_id)
        .bind(raw_name)
        .fetch_optional(&mut *tx)
        .await?;

    let outcome = match existing {
        Some(row) => FoundOrCreated {
            identity: identity_from_row(row),
            created: false,
        },
        None => {
            // The exact-name constraint still guards writers that bypass this
            // module; on conflict the existing row is returned (`xmax <> 0`).
            let row = sqlx::query(
                "INSERT INTO series (id, library_id, name) VALUES (gen_random_uuid(), $1, $2) \
                 ON CONFLICT (library_id, name) DO UPDATE SET name = EXCLUDED.name \
                 RETURNING id, library_id, name, (xmax = 0) AS inserted",
            )
            .bind(library_id)
            .bind(raw_name)
            .fetch_one(&mut *tx)
            .await?;
            let created: bool = row.get("inserted");
            FoundOrCreated {
                identity: identity_from_row(row),
                created,
            }
        }
    };

    tx.commit().await?;
    Ok(outcome)
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
        "SELECT DISTINCT ON (q.library_id, q.raw_name) \
                q.library_id, q.raw_name, s.id, s.library_id, s.name \
         FROM UNNEST($1::uuid[], $2::text[]) AS q(library_id, raw_name) \
         JOIN series s ON s.library_id = q.library_id \
                      AND (norm_text(s.name) = norm_text(q.raw_name) \
                           OR norm_text(s.original_name) = norm_text(q.raw_name)) \
         ORDER BY q.library_id, q.raw_name, \
                  (norm_text(s.name) = norm_text(q.raw_name)) DESC, s.created_at, s.id",
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
        // Two equi-joins (hash-joinable) instead of one OR join across every library.
        "SELECT q.raw_name FROM UNNEST($1::text[]) AS q(raw_name) \
         JOIN series s ON norm_text(s.name) = norm_text(q.raw_name) \
         UNION \
         SELECT q.raw_name FROM UNNEST($1::text[]) AS q(raw_name) \
         JOIN series s ON norm_text(s.original_name) = norm_text(q.raw_name)",
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

    async fn rename(pool: &PgPool, series_id: Uuid, name: &str, original_name: &str) {
        sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
            .bind(name)
            .bind(original_name)
            .bind(series_id)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn series_count(pool: &PgPool, library_id: Uuid) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM series WHERE library_id = $1")
            .bind(library_id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn find_resolves_the_original_name_after_a_rename(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let series_id = seed_series(&pool, library_id, "LES MYTHICS").await;
        rename(&pool, series_id, "Mythics", "LES MYTHICS").await;

        let identity = find(&pool, library_id, "les mythics")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(identity.id, series_id);
        assert_eq!(identity.name, "Mythics");
        assert_eq!(
            resolve_many(&pool, &[(library_id, "Les Mythics".to_string())])
                .await
                .unwrap()[&(library_id, "Les Mythics".to_string())]
                .id,
            series_id
        );
        assert!(existing_names(&pool, &["les mythics".to_string()])
            .await
            .unwrap()
            .contains("les mythics"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_current_name_wins_over_an_original_name(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let renamed = seed_series(&pool, library_id, "Dragon Ball").await;
        rename(&pool, renamed, "Dragon Ball Z", "Dragon Ball").await;
        let current = seed_series(&pool, library_id, "Dragon Ball").await;

        assert_eq!(
            find_id(&pool, library_id, "dragon ball").await.unwrap(),
            Some(current)
        );
        let resolved = find_many(&pool, library_id, &["dragon ball".to_string()])
            .await
            .unwrap();
        assert_eq!(resolved["dragon ball"].id, current);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn find_or_create_inserts_once_then_matches_case_and_accents(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;

        let created = find_or_create(&pool, library_id, "Astérix").await.unwrap();
        assert_eq!(created.name, "Astérix");
        assert_eq!(created.library_id, library_id);

        for name in ["Astérix", "asterix", "ASTÉRIX"] {
            let identity = find_or_create(&pool, library_id, name).await.unwrap();
            assert_eq!(identity, created, "raw name {name} must reuse the series");
        }
        assert_eq!(series_count(&pool, library_id).await, 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn find_or_create_tracked_reports_creation_only_once(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let renamed = seed_series(&pool, library_id, "LES MYTHICS").await;
        rename(&pool, renamed, "Mythics", "LES MYTHICS").await;

        let first = find_or_create_tracked(&pool, library_id, "One Piece")
            .await
            .unwrap();
        assert!(first.created);
        for name in ["One Piece", "one piece", "ONE PIÈCE"] {
            let again = find_or_create_tracked(&pool, library_id, name)
                .await
                .unwrap();
            assert!(!again.created, "{name} matches the existing series");
            assert_eq!(again.identity, first.identity);
        }
        let by_original = find_or_create_tracked(&pool, library_id, "les mythics")
            .await
            .unwrap();
        assert!(
            !by_original.created,
            "original_name is a match, not a creation"
        );
        assert_eq!(by_original.identity.id, renamed);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn find_or_create_is_scoped_to_the_library(pool: PgPool) {
        let first = seed_library(&pool, "first").await;
        let second = seed_library(&pool, "second").await;

        let a = find_or_create(&pool, first, "Naruto").await.unwrap();
        let b = find_or_create(&pool, second, "Naruto").await.unwrap();

        assert_ne!(a.id, b.id);
        assert_eq!(b.library_id, second);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn find_or_create_follows_renames(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let created = find_or_create(&pool, library_id, "Series A").await.unwrap();
        // Chained rename A -> B -> C keeps the first filesystem name.
        rename(&pool, created.id, "Series C", "Series A").await;

        let by_original = find_or_create(&pool, library_id, "series a").await.unwrap();
        let by_name = find_or_create(&pool, library_id, "Series C").await.unwrap();

        assert_eq!(by_original.id, created.id);
        assert_eq!(by_original.name, "Series C");
        assert_eq!(by_name.id, created.id);
        assert_eq!(series_count(&pool, library_id).await, 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn find_or_create_joins_a_caller_transaction(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let mut tx = pool.begin().await.unwrap();

        let identity = find_or_create(&mut *tx, library_id, "One Piece")
            .await
            .unwrap();
        tx.rollback().await.unwrap();

        assert_eq!(find_id(&pool, library_id, "One Piece").await.unwrap(), None);
        assert_ne!(identity.id, Uuid::nil());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn concurrent_find_or_create_yields_one_series(pool: PgPool) {
        let library_id = seed_library(&pool, "lib").await;
        let spellings = ["One Piece", "one piece", "ONE PIECE", "Óne Piece"];

        let tasks: Vec<_> = (0..16)
            .map(|i| {
                let pool = pool.clone();
                let name = spellings[i % spellings.len()];
                tokio::spawn(async move { find_or_create(&pool, library_id, name).await })
            })
            .collect();

        let mut ids = HashSet::new();
        for task in tasks {
            ids.insert(task.await.unwrap().unwrap().id);
        }

        assert_eq!(ids.len(), 1, "every spelling must resolve to one series");
        assert_eq!(series_count(&pool, library_id).await, 1);
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
