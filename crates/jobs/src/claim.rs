//! Owner-scoped claim queries.
//!
//! Each service claims only the types it owns, and only when its own exclusive
//! job types are idle. This replaces the per-service copies of the same SQL and
//! makes ownership an explicit registry property.

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::registry::{claimable_types, exclusive_types, optional_library_types, JobOwner};
use crate::status::{as_strings, JobState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedJob {
    pub id: Uuid,
    pub job_type: String,
    pub library_id: Option<Uuid>,
}

/// Claim the oldest pending job owned by `owner` and mark it `running`.
///
/// Returns `None` when the queue is empty, when an exclusive job owned by the
/// same service is already active, or when the owner has no claimable types.
pub async fn claim_next(pool: &PgPool, owner: JobOwner) -> Result<Option<ClaimedJob>, sqlx::Error> {
    match owner {
        JobOwner::Api => claim_api(pool).await,
        JobOwner::Indexer => claim_indexer(pool).await,
        JobOwner::Legacy => Ok(None),
    }
}

async fn has_active_exclusive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: JobOwner,
) -> Result<bool, sqlx::Error> {
    let exclusive = exclusive_types(owner);
    if exclusive.is_empty() {
        return Ok(false);
    }

    sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM index_jobs \
            WHERE status = ANY($1) AND type = ANY($2)\
         )",
    )
    .bind(as_strings(&JobState::ACTIVE))
    .bind(exclusive)
    .fetch_one(&mut **tx)
    .await
}

async fn mark_running(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE index_jobs SET status = 'running', started_at = NOW(), error_opt = NULL WHERE id = $1",
    )
    .bind(id)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}

async fn claim_api(pool: &PgPool) -> Result<Option<ClaimedJob>, sqlx::Error> {
    let types = claimable_types(JobOwner::Api);
    if types.is_empty() {
        return Ok(None);
    }
    let optional_library = optional_library_types(JobOwner::Api);
    let exclusive = exclusive_types(JobOwner::Api);

    let mut tx = pool.begin().await?;
    let blocked = has_active_exclusive(&mut tx, JobOwner::Api).await?;

    let row = sqlx::query(
        r#"
        SELECT id, type, library_id
        FROM index_jobs
        WHERE status = 'pending'
          AND type = ANY($1)
          AND (library_id IS NOT NULL OR type = ANY($2))
          AND (type != ALL($3) OR NOT $4::bool)
        ORDER BY created_at ASC
        FOR UPDATE SKIP LOCKED
        LIMIT 1
        "#,
    )
    .bind(&types)
    .bind(&optional_library)
    .bind(&exclusive)
    .bind(blocked)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let id: Uuid = row.get("id");
    let job_type: String = row.get("type");
    let library_id: Option<Uuid> = row.get("library_id");

    mark_running(&mut tx, id).await?;
    tx.commit().await?;

    Ok(Some(ClaimedJob {
        id,
        job_type,
        library_id,
    }))
}

async fn claim_indexer(pool: &PgPool) -> Result<Option<ClaimedJob>, sqlx::Error> {
    let definitions: Vec<_> = crate::registry::definitions()
        .iter()
        .filter(|definition| definition.owner == JobOwner::Indexer && definition.claimable)
        .collect();

    if definitions.is_empty() {
        return Ok(None);
    }

    let types: Vec<String> = definitions
        .iter()
        .map(|definition| definition.job_type.to_string())
        .collect();
    let priorities: Vec<i32> = definitions
        .iter()
        .map(|definition| definition.priority)
        .collect();
    let exclusive = exclusive_types(JobOwner::Indexer);

    let mut tx = pool.begin().await?;
    let blocked = has_active_exclusive(&mut tx, JobOwner::Indexer).await?;

    let row = sqlx::query(
        r#"
        SELECT j.id, j.type, j.library_id
        FROM index_jobs j
        JOIN unnest($1::text[], $2::int[]) AS queued(job_type, priority)
          ON queued.job_type = j.type
        WHERE j.status = 'pending'
          AND (j.type != ALL($3) OR NOT $4::bool)
        ORDER BY queued.priority ASC, j.created_at ASC
        FOR UPDATE OF j SKIP LOCKED
        LIMIT 1
        "#,
    )
    .bind(&types)
    .bind(&priorities)
    .bind(&exclusive)
    .bind(blocked)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let id: Uuid = row.get("id");
    let job_type: String = row.get("type");
    let library_id: Option<Uuid> = row.get("library_id");

    mark_running(&mut tx, id).await?;
    tx.commit().await?;

    Ok(Some(ClaimedJob {
        id,
        job_type,
        library_id,
    }))
}
