//! The `release_blacklist` table: releases the user never wants to see again.
//!
//! The Prowlarr detection, the RSS poll and the DDL provider search all skip
//! blacklisted titles before storing their results. This module owns that
//! shared read so the filter stays identical everywhere.

use std::collections::HashSet;

use sqlx::PgPool;

/// Load every blacklisted release title as a set for O(1) membership checks.
pub(crate) async fn load_blacklisted_titles(pool: &PgPool) -> Result<HashSet<String>, sqlx::Error> {
    let titles = sqlx::query_scalar("SELECT title FROM release_blacklist")
        .fetch_all(pool)
        .await?;
    Ok(titles.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn returns_every_blacklisted_title(pool: PgPool) {
        sqlx::query(
            "INSERT INTO release_blacklist (title) VALUES ('Astérix T01'), ('Astérix T02')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let titles = load_blacklisted_titles(&pool).await.unwrap();

        assert_eq!(titles.len(), 2);
        assert!(titles.contains("Astérix T01"));
        assert!(titles.contains("Astérix T02"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn empty_blacklist_yields_empty_set(pool: PgPool) {
        let titles = load_blacklisted_titles(&pool).await.unwrap();

        assert!(titles.is_empty());
    }
}
