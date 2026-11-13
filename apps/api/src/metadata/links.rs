//! Sole writer of `external_metadata_links` (ADR 0017).
//!
//! A link ties a series to one provider entry. Its lifecycle is
//! `pending → approved → (rejected | deleted)`, and at most one **approved**
//! link per series is the primary (partial unique index
//! `external_metadata_links_one_primary_per_series`, migration 0117).
//!
//! Every transition lives here so the invariant and the provider notes
//! (rating columns extracted from `metadata_json`) are written in one place:
//!
//! - [`propose`]: create or reset a link as `pending` (manual match);
//! - [`approve`]: create or refresh a link as `approved`, promoted to primary
//!   when the series has none; [`OnConflict`] says whether an existing link of
//!   the same provider is replaced (auto-match) or kept (series creation);
//! - [`approve_existing`]: approve a stored link (review queue);
//! - [`set_primary`]: switch the primary to an approved link;
//! - [`remove`]: delete / reject / drop unconfigured providers, then apply the
//!   single after-removal policy;
//! - [`reassign`]: move links during a series merge;
//! - [`record_provider_snapshot`] / [`mark_synced`]: refresh bookkeeping.
//!
//! **After-removal policy.** When a removal takes at least one *approved* link
//! out of the approved set, the oldest remaining approved link is promoted if
//! the series lost its primary, then the series fields are re-merged from the
//! stored links that remain (no provider call), or cleared when none remain.
//! Removing only pending or rejected links changes nothing else.

use std::fmt;

use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use super::field_policy;
use crate::error::ApiError;

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

const APPROVED: &str = "approved";

/// Provider data for a link being proposed or approved.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LinkCandidate<'a> {
    pub library_id: Uuid,
    pub series_id: Uuid,
    pub provider: &'a str,
    pub external_id: &'a str,
    pub external_url: Option<&'a str>,
    pub confidence: Option<f32>,
    pub metadata_json: &'a serde_json::Value,
    pub total_volumes: Option<i32>,
}

/// Provider rating notes denormalized from `metadata_json` into the
/// `provider_rating*` columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ProviderNotes {
    pub rating: Option<f64>,
    pub rating_count: Option<i64>,
    pub rating_scale: Option<f64>,
}

impl ProviderNotes {
    /// Extract the notes from a provider `metadata_json` blob. A rating of
    /// zero or less means "no rating".
    pub(crate) fn from_metadata(metadata_json: &serde_json::Value) -> Self {
        Self {
            rating: metadata_json
                .get("rating")
                .and_then(|v| v.as_f64())
                .filter(|&r| r > 0.0),
            rating_count: metadata_json.get("rating_count").and_then(|v| v.as_i64()),
            rating_scale: metadata_json.get("rating_scale").and_then(|v| v.as_f64()),
        }
    }
}

/// What [`approve`] does when the series already has a link for the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OnConflict {
    /// Auto-match: the new provider entry replaces the stored one (external
    /// id, url, data and notes).
    Replace,
    /// Series creation / Discovery: two provider entities with the same title
    /// land on one series; the stored identity and its own data are kept and
    /// only approved (#25).
    KeepExisting,
}

/// What to take out of the approved set.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Removal<'a> {
    /// Delete one link (and its book rows, by cascade).
    Delete(Uuid),
    /// Keep the row as `rejected` so it is not proposed again.
    Reject(Uuid),
    /// Rematch cleanup: delete the series links whose provider is not listed.
    ProvidersNotIn { series_id: Uuid, keep: &'a [String] },
}

/// What happened to the series metadata after a removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SeriesMetadata {
    /// No approved link was removed: nothing to recompute.
    Untouched,
    /// Approved links remain: series fields re-merged from them.
    Resynced,
    /// The last approved link was removed: external series fields cleared.
    Cleared,
}

/// Outcome of [`remove`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AfterRemoval {
    pub series_id: Uuid,
    /// Rows deleted or rejected.
    pub removed: u64,
    /// Link promoted to primary because the series lost its primary.
    pub promoted: Option<Uuid>,
    pub series_metadata: SeriesMetadata,
}

/// The link [`approve_existing`] approved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApprovedLink {
    pub library_id: Uuid,
    pub series_id: Uuid,
    pub provider: String,
}

#[derive(Debug)]
pub(crate) enum LinkError {
    NotFound,
    /// Only an approved link can become primary.
    NotApproved,
    Database(sqlx::Error),
    /// Re-merging the series after a removal failed.
    Sync(ApiError),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("link not found"),
            Self::NotApproved => f.write_str("only approved links can be set as primary"),
            Self::Database(e) => write!(f, "database error: {e}"),
            Self::Sync(e) => write!(f, "series sync failed: {}", e.message),
        }
    }
}

impl std::error::Error for LinkError {}

impl From<sqlx::Error> for LinkError {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(e)
    }
}

impl From<LinkError> for ApiError {
    fn from(e: LinkError) -> Self {
        match e {
            LinkError::NotFound => ApiError::not_found("link not found"),
            LinkError::NotApproved => {
                ApiError::unprocessable_entity("only approved links can be set as primary")
            }
            LinkError::Database(e) => e.into(),
            LinkError::Sync(e) => e,
        }
    }
}

// ---------------------------------------------------------------------------
// Transitions
// ---------------------------------------------------------------------------

const APPROVE_REPLACE_SQL: &str = r#"
    INSERT INTO external_metadata_links
        (library_id, series_id, provider, external_id, external_url, status, confidence,
         metadata_json, total_volumes_external,
         provider_rating, provider_rating_count, provider_rating_scale, approved_at)
    VALUES ($1, $2, $3, $4, $5, 'approved', $6, $7, $8, $9, $10, $11, NOW())
    ON CONFLICT (series_id, provider)
    DO UPDATE SET
        external_id = EXCLUDED.external_id,
        external_url = EXCLUDED.external_url,
        status = 'approved',
        is_primary = external_metadata_links.is_primary
            AND external_metadata_links.status = 'approved',
        confidence = EXCLUDED.confidence,
        metadata_json = EXCLUDED.metadata_json,
        total_volumes_external = EXCLUDED.total_volumes_external,
        provider_rating = EXCLUDED.provider_rating,
        provider_rating_count = EXCLUDED.provider_rating_count,
        provider_rating_scale = EXCLUDED.provider_rating_scale,
        matched_at = NOW(),
        approved_at = NOW(),
        updated_at = NOW()
    RETURNING id
"#;

const APPROVE_KEEP_EXISTING_SQL: &str = r#"
    INSERT INTO external_metadata_links
        (library_id, series_id, provider, external_id, external_url, status, confidence,
         metadata_json, total_volumes_external,
         provider_rating, provider_rating_count, provider_rating_scale, approved_at)
    VALUES ($1, $2, $3, $4, $5, 'approved', $6, $7, $8, $9, $10, $11, NOW())
    ON CONFLICT (series_id, provider)
    DO UPDATE SET
        status = 'approved',
        is_primary = external_metadata_links.is_primary
            AND external_metadata_links.status = 'approved',
        matched_at = NOW(),
        approved_at = NOW(),
        updated_at = NOW()
    RETURNING id
"#;

/// Create or reset the series link for `candidate.provider` as `pending`.
///
/// The provider data is replaced, the approval and sync timestamps reset and
/// the primary flag dropped. Re-proposing an approved link takes it out of the
/// approved set, so the after-removal policy applies.
pub(crate) async fn propose(
    pool: &PgPool,
    candidate: &LinkCandidate<'_>,
) -> Result<Uuid, LinkError> {
    let notes = ProviderNotes::from_metadata(candidate.metadata_json);
    let mut tx = pool.begin().await?;
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT status FROM external_metadata_links \
         WHERE series_id = $1 AND provider = $2 FOR UPDATE",
    )
    .bind(candidate.series_id)
    .bind(candidate.provider)
    .fetch_optional(&mut *tx)
    .await?;

    let link_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO external_metadata_links
            (library_id, series_id, provider, external_id, external_url, status, confidence,
             metadata_json, total_volumes_external,
             provider_rating, provider_rating_count, provider_rating_scale)
        VALUES ($1, $2, $3, $4, $5, 'pending', $6, $7, $8, $9, $10, $11)
        ON CONFLICT (series_id, provider)
        DO UPDATE SET
            external_id = EXCLUDED.external_id,
            external_url = EXCLUDED.external_url,
            status = 'pending',
            is_primary = false,
            confidence = EXCLUDED.confidence,
            metadata_json = EXCLUDED.metadata_json,
            total_volumes_external = EXCLUDED.total_volumes_external,
            provider_rating = EXCLUDED.provider_rating,
            provider_rating_count = EXCLUDED.provider_rating_count,
            provider_rating_scale = EXCLUDED.provider_rating_scale,
            matched_at = NOW(),
            updated_at = NOW(),
            approved_at = NULL,
            synced_at = NULL
        RETURNING id
        "#,
    )
    .bind(candidate.library_id)
    .bind(candidate.series_id)
    .bind(candidate.provider)
    .bind(candidate.external_id)
    .bind(candidate.external_url)
    .bind(candidate.confidence)
    .bind(candidate.metadata_json)
    .bind(candidate.total_volumes)
    .bind(notes.rating)
    .bind(notes.rating_count)
    .bind(notes.rating_scale)
    .fetch_one(&mut *tx)
    .await?;

    let demoted = previous.as_deref() == Some(APPROVED);
    if demoted {
        promote_oldest_approved(&mut tx, candidate.series_id).await?;
    }
    tx.commit().await?;

    if demoted {
        resync_or_clear(pool, candidate.series_id).await?;
    }
    Ok(link_id)
}

/// Create or refresh the series link for `candidate.provider` as `approved`
/// and promote it to primary when the series has none. An approved primary
/// stays primary; a stale primary flag on a non-approved row is dropped. The
/// caller syncs the series.
pub(crate) async fn approve(
    pool: &PgPool,
    candidate: &LinkCandidate<'_>,
    on_conflict: OnConflict,
) -> Result<Uuid, LinkError> {
    let notes = ProviderNotes::from_metadata(candidate.metadata_json);
    let sql = match on_conflict {
        OnConflict::Replace => APPROVE_REPLACE_SQL,
        OnConflict::KeepExisting => APPROVE_KEEP_EXISTING_SQL,
    };
    let mut tx = pool.begin().await?;
    let link_id: Uuid = sqlx::query_scalar(sql)
        .bind(candidate.library_id)
        .bind(candidate.series_id)
        .bind(candidate.provider)
        .bind(candidate.external_id)
        .bind(candidate.external_url)
        .bind(candidate.confidence)
        .bind(candidate.metadata_json)
        .bind(candidate.total_volumes)
        .bind(notes.rating)
        .bind(notes.rating_count)
        .bind(notes.rating_scale)
        .fetch_one(&mut *tx)
        .await?;

    promote_if_no_primary(&mut tx, candidate.series_id, link_id).await?;
    tx.commit().await?;
    Ok(link_id)
}

/// Approve a stored link. It becomes primary when `make_primary` is set, or
/// when the series has no approved primary yet. The caller syncs the series.
pub(crate) async fn approve_existing(
    pool: &PgPool,
    link_id: Uuid,
    make_primary: bool,
) -> Result<ApprovedLink, LinkError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        r#"
        UPDATE external_metadata_links
        SET status = 'approved',
            is_primary = is_primary AND status = 'approved',
            approved_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        RETURNING library_id, series_id, provider
        "#,
    )
    .bind(link_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(LinkError::NotFound)?;

    let approved = ApprovedLink {
        library_id: row.get("library_id"),
        series_id: row.get("series_id"),
        provider: row.get("provider"),
    };

    if make_primary {
        switch_primary(&mut tx, approved.series_id, link_id).await?;
    } else {
        promote_if_no_primary(&mut tx, approved.series_id, link_id).await?;
    }
    tx.commit().await?;
    Ok(approved)
}

/// Make an approved link the primary of its series. Returns the series id.
pub(crate) async fn set_primary(pool: &PgPool, link_id: Uuid) -> Result<Uuid, LinkError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT series_id, status FROM external_metadata_links WHERE id = $1 FOR UPDATE",
    )
    .bind(link_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(LinkError::NotFound)?;

    let series_id: Uuid = row.get("series_id");
    let status: String = row.get("status");
    if status != APPROVED {
        return Err(LinkError::NotApproved);
    }
    switch_primary(&mut tx, series_id, link_id).await?;
    tx.commit().await?;
    Ok(series_id)
}

/// Take links out of the approved set, then apply the after-removal policy
/// (see the module docs). `Delete` and `Reject` fail with `NotFound` on an
/// unknown id; `ProvidersNotIn` removing nothing is not an error.
pub(crate) async fn remove(pool: &PgPool, removal: Removal<'_>) -> Result<AfterRemoval, LinkError> {
    let mut tx = pool.begin().await?;

    let (series_id, removed_statuses): (Uuid, Vec<String>) = match removal {
        Removal::Delete(link_id) => {
            let row = sqlx::query(
                "DELETE FROM external_metadata_links WHERE id = $1 RETURNING series_id, status",
            )
            .bind(link_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(LinkError::NotFound)?;
            (row.get("series_id"), vec![row.get("status")])
        }
        Removal::Reject(link_id) => {
            let row = sqlx::query(
                r#"
                WITH previous AS (
                    SELECT id, status FROM external_metadata_links WHERE id = $1 FOR UPDATE
                )
                UPDATE external_metadata_links eml
                SET status = 'rejected', is_primary = false, updated_at = NOW()
                FROM previous
                WHERE eml.id = previous.id
                RETURNING eml.series_id, previous.status
                "#,
            )
            .bind(link_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(LinkError::NotFound)?;
            (row.get("series_id"), vec![row.get("status")])
        }
        Removal::ProvidersNotIn { series_id, keep } => {
            let statuses: Vec<String> = sqlx::query_scalar(
                "DELETE FROM external_metadata_links \
                 WHERE series_id = $1 AND provider <> ALL($2) \
                 RETURNING status",
            )
            .bind(series_id)
            .bind(keep)
            .fetch_all(&mut *tx)
            .await?;
            (series_id, statuses)
        }
    };

    let approved_removed = removed_statuses.iter().any(|s| s == APPROVED);
    let promoted = if approved_removed {
        promote_oldest_approved(&mut tx, series_id).await?
    } else {
        None
    };
    tx.commit().await?;

    let series_metadata = if approved_removed {
        resync_or_clear(pool, series_id).await?
    } else {
        SeriesMetadata::Untouched
    };

    Ok(AfterRemoval {
        series_id,
        removed: removed_statuses.len() as u64,
        promoted,
        series_metadata,
    })
}

/// Series merge: move the links of `from` to `to` inside the caller's
/// transaction. A provider the target already has keeps the target's link
/// (the source duplicate is deleted); a moved primary stays primary only when
/// the target has none. Returns the number of links moved.
pub(crate) async fn reassign(
    conn: &mut PgConnection,
    from: Uuid,
    to: Uuid,
) -> Result<u64, sqlx::Error> {
    let moved = sqlx::query(
        r#"
        UPDATE external_metadata_links src
        SET series_id = $1,
            is_primary = src.is_primary
                AND src.status = 'approved'
                AND NOT EXISTS (
                    SELECT 1 FROM external_metadata_links t
                    WHERE t.series_id = $1 AND t.status = 'approved' AND t.is_primary
                ),
            updated_at = NOW()
        WHERE src.series_id = $2
          AND src.provider NOT IN (
              SELECT provider FROM external_metadata_links WHERE series_id = $1
          )
        "#,
    )
    .bind(to)
    .bind(from)
    .execute(&mut *conn)
    .await?
    .rows_affected();

    sqlx::query("DELETE FROM external_metadata_links WHERE series_id = $1")
        .bind(from)
        .execute(&mut *conn)
        .await?;

    promote_oldest_approved(conn, to).await?;
    Ok(moved)
}

/// Refresh: store the provider's latest series data and notes on the link.
pub(crate) async fn record_provider_snapshot(
    pool: &PgPool,
    link_id: Uuid,
    metadata_json: &serde_json::Value,
    total_volumes: Option<i32>,
) -> Result<(), LinkError> {
    let notes = ProviderNotes::from_metadata(metadata_json);
    let result = sqlx::query(
        r#"
        UPDATE external_metadata_links
        SET metadata_json = $2,
            total_volumes_external = $3,
            provider_rating = $4,
            provider_rating_count = $5,
            provider_rating_scale = $6,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(link_id)
    .bind(metadata_json)
    .bind(total_volumes)
    .bind(notes.rating)
    .bind(notes.rating_count)
    .bind(notes.rating_scale)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(LinkError::NotFound);
    }
    Ok(())
}

/// Stamp the link as synced (its book metadata was just pulled).
pub(crate) async fn mark_synced(pool: &PgPool, link_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE external_metadata_links SET synced_at = NOW(), updated_at = NOW() WHERE id = $1",
    )
    .bind(link_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Primary invariant (one approved primary per series)
// ---------------------------------------------------------------------------

/// Promote `link_id` if it is approved and the series has no approved primary.
async fn promote_if_no_primary(
    conn: &mut PgConnection,
    series_id: Uuid,
    link_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE external_metadata_links
        SET is_primary = true, updated_at = NOW()
        WHERE id = $2
          AND status = 'approved'
          AND NOT EXISTS (
              SELECT 1 FROM external_metadata_links e2
              WHERE e2.series_id = $1 AND e2.status = 'approved' AND e2.is_primary
          )
        "#,
    )
    .bind(series_id)
    .bind(link_id)
    .execute(conn)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Promote the oldest approved link when the series has no approved primary.
async fn promote_oldest_approved(
    conn: &mut PgConnection,
    series_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        UPDATE external_metadata_links
        SET is_primary = true, updated_at = NOW()
        WHERE id = (
            SELECT e2.id FROM external_metadata_links e2
            WHERE e2.series_id = $1
              AND e2.status = 'approved'
              AND NOT EXISTS (
                  SELECT 1 FROM external_metadata_links e3
                  WHERE e3.series_id = $1 AND e3.status = 'approved' AND e3.is_primary
              )
            ORDER BY e2.approved_at ASC NULLS LAST, e2.id ASC
            LIMIT 1
        )
        RETURNING id
        "#,
    )
    .bind(series_id)
    .fetch_optional(conn)
    .await
}

/// Clear the current primary, then flag `link_id` (which must be approved).
async fn switch_primary(
    conn: &mut PgConnection,
    series_id: Uuid,
    link_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE external_metadata_links SET is_primary = false, updated_at = NOW() \
         WHERE series_id = $1 AND is_primary AND id <> $2",
    )
    .bind(series_id)
    .bind(link_id)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE external_metadata_links SET is_primary = true, updated_at = NOW() \
         WHERE id = $1 AND series_id = $2 AND status = 'approved'",
    )
    .bind(link_id)
    .bind(series_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// After-removal policy
// ---------------------------------------------------------------------------

/// Re-merge the series fields from the approved links that remain (stored
/// data only, no provider call), or clear them when none remain.
async fn resync_or_clear(pool: &PgPool, series_id: Uuid) -> Result<SeriesMetadata, LinkError> {
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1 AND status = 'approved'",
    )
    .bind(series_id)
    .fetch_one(pool)
    .await?;

    if remaining > 0 {
        super::sync_series_from_links(pool, series_id, true, false)
            .await
            .map_err(LinkError::Sync)?;
        Ok(SeriesMetadata::Resynced)
    } else {
        field_policy::clear_series_metadata(pool, series_id).await?;
        Ok(SeriesMetadata::Cleared)
    }
}

#[cfg(test)]
#[path = "tests/links.rs"]
mod tests;
