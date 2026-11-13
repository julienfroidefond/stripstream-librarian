//! The `torrent_downloads` lifecycle.
//!
//! The table is the single ledger for both download clients (`qbittorrent` and
//! `alldebrid`). This module owns the status vocabulary and every write that
//! moves a row through it, so a caller asks for a transition and gets a typed
//! error when the row is no longer in a state that allows it, instead of
//! silently writing a status the poller, the retry button or the UI would
//! misread.
//!
//! Progress writes live here too, but they are not transitions: they no-op on a
//! row that is no longer `downloading`, because a poller racing with an import
//! is normal.

use std::fmt;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

/// Status of a `torrent_downloads` row.
///
/// Mirrors the `torrent_downloads_status_check` constraint (migration 0082).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TorrentStatus {
    Downloading,
    Completed,
    Importing,
    Imported,
    Partial,
    NoFilesImported,
    Error,
}

impl TorrentStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Downloading => "downloading",
            Self::Completed => "completed",
            Self::Importing => "importing",
            Self::Imported => "imported",
            Self::Partial => "partial",
            Self::NoFilesImported => "no_files_imported",
            Self::Error => "error",
        }
    }

    /// A transfer or import that is still in flight: one active row per hash is
    /// allowed in this state.
    pub(super) const ACTIVE: [Self; 3] = [Self::Downloading, Self::Completed, Self::Importing];

    /// A row whose import the UI may launch again.
    ///
    /// `importing` is included because a crashed process leaves the row there;
    /// `imported` is not, because there is nothing left to fetch.
    pub(super) fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::Importing | Self::Error | Self::NoFilesImported | Self::Partial
        )
    }
}

impl fmt::Display for TorrentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a string read from the database is not a known status.
#[derive(Debug)]
pub struct UnknownTorrentStatus(pub String);

impl fmt::Display for UnknownTorrentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown torrent download status '{}'", self.0)
    }
}

impl std::error::Error for UnknownTorrentStatus {}

impl TryFrom<&str> for TorrentStatus {
    type Error = UnknownTorrentStatus;

    fn try_from(raw: &str) -> Result<Self, UnknownTorrentStatus> {
        Ok(match raw {
            "downloading" => Self::Downloading,
            "completed" => Self::Completed,
            "importing" => Self::Importing,
            "imported" => Self::Imported,
            "partial" => Self::Partial,
            "no_files_imported" => Self::NoFilesImported,
            "error" => Self::Error,
            other => return Err(UnknownTorrentStatus(other.to_string())),
        })
    }
}

/// Why a transition was refused.
#[derive(Debug)]
pub(super) enum TorrentTransitionError {
    /// The row does not exist (deleted while a poller was working on it).
    NotFound(Uuid),
    /// The row exists but its status is not one the action starts from.
    NotAllowed {
        id: Uuid,
        action: &'static str,
        from: TorrentStatus,
    },
    /// The database holds a status outside the vocabulary — schema/code drift.
    UnknownStatus {
        id: Uuid,
        status: String,
    },
    Database(sqlx::Error),
}

impl fmt::Display for TorrentTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "torrent download {id} not found"),
            Self::NotAllowed { id, action, from } => {
                write!(f, "cannot {action} torrent download {id}: status is {from}")
            }
            Self::UnknownStatus { id, status } => {
                write!(f, "torrent download {id} has unknown status '{status}'")
            }
            Self::Database(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TorrentTransitionError {}

impl From<sqlx::Error> for TorrentTransitionError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err)
    }
}

impl From<TorrentTransitionError> for ApiError {
    fn from(err: TorrentTransitionError) -> Self {
        match &err {
            TorrentTransitionError::NotFound(_) => ApiError::not_found(err.to_string()),
            TorrentTransitionError::NotAllowed { .. } => ApiError::bad_request(err.to_string()),
            TorrentTransitionError::UnknownStatus { .. } | TorrentTransitionError::Database(_) => {
                ApiError::internal(err.to_string())
            }
        }
    }
}

/// What one import produced, as far as the series is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImportOutcome {
    Imported,
    Partial,
    NoFilesImported,
}

impl ImportOutcome {
    fn status(self) -> TorrentStatus {
        match self {
            Self::Imported => TorrentStatus::Imported,
            Self::Partial => TorrentStatus::Partial,
            Self::NoFilesImported => TorrentStatus::NoFilesImported,
        }
    }
}

/// Decide the status of a finished import.
///
/// `partial` means "some files landed, but at least one expected tome is still
/// missing", so retrying can still make progress. Files skipped for a normal
/// reason (an existing more recent volume, a lower-priority duplicate format)
/// do not make an import partial: only volume coverage does.
pub(super) fn import_outcome(
    imported_volumes: &[i32],
    expected_volumes: &[i32],
    replace_existing: bool,
) -> ImportOutcome {
    if imported_volumes.is_empty() {
        return ImportOutcome::NoFilesImported;
    }
    if replace_existing || expected_volumes.is_empty() {
        return ImportOutcome::Imported;
    }
    if expected_volumes
        .iter()
        .all(|volume| imported_volumes.contains(volume))
    {
        ImportOutcome::Imported
    } else {
        ImportOutcome::Partial
    }
}

/// Which rows a restart accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RestartScope {
    /// The AllDebrid restart button: only a failed row.
    Failed,
    /// The retry button: every status [`TorrentStatus::is_retryable`] accepts.
    Retryable,
}

impl RestartScope {
    fn allowed_from(self) -> &'static [TorrentStatus] {
        match self {
            Self::Failed => &[TorrentStatus::Error],
            Self::Retryable => &[
                TorrentStatus::Importing,
                TorrentStatus::Error,
                TorrentStatus::NoFilesImported,
                TorrentStatus::Partial,
            ],
        }
    }
}

/// Live progress of a transfer.
#[derive(Debug, Clone, Copy)]
pub(super) struct ProgressUpdate<'a> {
    pub progress: f32,
    pub speed: i64,
    pub eta: i64,
    /// Total size in bytes.
    ///
    /// The size is never erased: `None` (unknown) and `Some(0)` (the backend
    /// does not know it) both keep the stored value, and only a positive value
    /// replaces it.
    pub file_size: Option<i64>,
    /// Debrid peers seeding the torrent; `None` keeps the stored value.
    pub seeders: Option<i32>,
    /// Torrent name reported by the backend; `None` keeps the stored value.
    pub name: Option<&'a str>,
    /// Bytes already transferred; any value `> 0` clears the stall marker.
    pub downloaded: i64,
}

/// The status names to bind for a `status = ANY($n)` guard.
pub(super) fn status_names(statuses: &[TorrentStatus]) -> Vec<String> {
    statuses
        .iter()
        .map(|status| status.as_str().to_string())
        .collect()
}

/// Check that a guarded `UPDATE` matched a row, and explain why it did not.
async fn expect_transition(
    pool: &PgPool,
    id: Uuid,
    action: &'static str,
    rows_affected: u64,
) -> Result<(), TorrentTransitionError> {
    if rows_affected > 0 {
        return Ok(());
    }

    let current: Option<String> =
        sqlx::query_scalar("SELECT status FROM torrent_downloads WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

    match current {
        None => Err(TorrentTransitionError::NotFound(id)),
        Some(raw) => match TorrentStatus::try_from(raw.as_str()) {
            Ok(status) => Err(TorrentTransitionError::NotAllowed {
                id,
                action,
                from: status,
            }),
            Err(_) => Err(TorrentTransitionError::UnknownStatus { id, status: raw }),
        },
    }
}

/// The backend finished the transfer and the files are at `content_path`.
pub(super) async fn mark_completed(
    pool: &PgPool,
    id: Uuid,
    content_path: &str,
) -> Result<(), TorrentTransitionError> {
    let from = [TorrentStatus::Downloading];
    let result = sqlx::query(
        "UPDATE torrent_downloads SET status = 'completed', content_path = $2, progress = 1, \
         download_speed = 0, eta = 0, stall_started_at = NULL, updated_at = NOW() \
         WHERE id = $1 AND status = ANY($3)",
    )
    .bind(id)
    .bind(content_path)
    .bind(status_names(&from))
    .execute(pool)
    .await?;

    expect_transition(pool, id, "complete", result.rows_affected()).await
}

/// The import pipeline took ownership of the row.
pub(super) async fn begin_import(pool: &PgPool, id: Uuid) -> Result<(), TorrentTransitionError> {
    let from = [
        TorrentStatus::Completed,
        TorrentStatus::Importing,
        TorrentStatus::Error,
        TorrentStatus::NoFilesImported,
        TorrentStatus::Partial,
    ];
    let result = sqlx::query(
        "UPDATE torrent_downloads SET status = 'importing', updated_at = NOW() \
         WHERE id = $1 AND status = ANY($2)",
    )
    .bind(id)
    .bind(status_names(&from))
    .execute(pool)
    .await?;

    expect_transition(pool, id, "start an import for", result.rows_affected()).await
}

/// The import finished with `outcome`; store its status and file report.
pub(super) async fn finish_import(
    pool: &PgPool,
    id: Uuid,
    outcome: ImportOutcome,
    imported_files: &serde_json::Value,
) -> Result<(), TorrentTransitionError> {
    let from = [TorrentStatus::Importing];
    let result = sqlx::query(
        "UPDATE torrent_downloads SET status = $2, imported_files = $3, updated_at = NOW() \
         WHERE id = $1 AND status = ANY($4)",
    )
    .bind(id)
    .bind(outcome.status().as_str())
    .bind(imported_files)
    .bind(status_names(&from))
    .execute(pool)
    .await?;

    expect_transition(pool, id, "finish the import of", result.rows_affected()).await
}

/// Fail the row with `message`.
///
/// `imported` is refused: a finished import must not be overwritten by a
/// poller that raced with it.
pub(super) async fn fail(
    pool: &PgPool,
    id: Uuid,
    message: &str,
) -> Result<(), TorrentTransitionError> {
    let from = [
        TorrentStatus::Downloading,
        TorrentStatus::Completed,
        TorrentStatus::Importing,
        TorrentStatus::Error,
        TorrentStatus::NoFilesImported,
        TorrentStatus::Partial,
    ];
    let result = sqlx::query(
        "UPDATE torrent_downloads SET status = 'error', error_message = $2, \
         download_speed = 0, eta = 0, stall_started_at = NULL, updated_at = NOW() \
         WHERE id = $1 AND status = ANY($3)",
    )
    .bind(id)
    .bind(message)
    .bind(status_names(&from))
    .execute(pool)
    .await?;

    expect_transition(pool, id, "fail", result.rows_affected()).await
}

/// Re-arm the transfer in the backend and reset its progress.
pub(super) async fn restart(
    pool: &PgPool,
    id: Uuid,
    scope: RestartScope,
) -> Result<(), TorrentTransitionError> {
    let from = scope.allowed_from();
    let result = sqlx::query(
        "UPDATE torrent_downloads SET status = 'downloading', error_message = NULL, \
         progress = 0, download_speed = 0, eta = 0, stall_started_at = NULL, updated_at = NOW() \
         WHERE id = $1 AND status = ANY($2)",
    )
    .bind(id)
    .bind(status_names(from))
    .execute(pool)
    .await?;

    expect_transition(pool, id, "restart", result.rows_affected()).await
}

/// Store the live progress of a running transfer.
///
/// A no-op when the row is no longer `downloading`; that race is expected and
/// the caller only needs to log a real database failure.
pub(super) async fn record_progress(
    pool: &PgPool,
    id: Uuid,
    update: &ProgressUpdate<'_>,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE torrent_downloads SET progress = $2, download_speed = $3, eta = $4, \
         seeders = COALESCE($5, seeders), \
         file_size = CASE WHEN $6::bigint IS NULL OR $6 = 0 THEN file_size ELSE $6 END, \
         name = COALESCE($7, name), \
         stall_started_at = CASE WHEN $8 > 0 THEN NULL ELSE stall_started_at END, \
         updated_at = NOW() \
         WHERE id = $1 AND status = 'downloading'",
    )
    .bind(id)
    .bind(update.progress)
    .bind(update.speed)
    .bind(update.eta)
    .bind(update.seeders)
    .bind(update.file_size)
    .bind(update.name)
    .bind(update.downloaded)
    .execute(pool)
    .await?;

    Ok(())
}

/// Store progress for a magnet that has not transferred a byte yet and return
/// the instant it was first seen at zero.
///
/// The marker is armed once (`COALESCE`) and kept across polls so a torrent the
/// debrid service can never fetch does not sit `downloading` forever;
/// [`record_progress`] clears it as soon as a byte arrives, and so does
/// [`restart`].
pub(super) async fn arm_stall(
    pool: &PgPool,
    id: Uuid,
    update: &ProgressUpdate<'_>,
) -> sqlx::Result<Option<DateTime<Utc>>> {
    let stalled = sqlx::query_scalar::<_, DateTime<Utc>>(
        "UPDATE torrent_downloads SET progress = $2, download_speed = $3, eta = $4, \
         seeders = COALESCE($5, seeders), \
         file_size = CASE WHEN $6::bigint IS NULL OR $6 = 0 THEN file_size ELSE $6 END, \
         name = COALESCE($7, name), \
         stall_started_at = COALESCE(stall_started_at, NOW()), updated_at = NOW() \
         WHERE id = $1 AND status = 'downloading' \
         RETURNING stall_started_at",
    )
    .bind(id)
    .bind(update.progress)
    .bind(update.speed)
    .bind(update.eta)
    .bind(update.seeders)
    .bind(update.file_size)
    .bind(update.name)
    .fetch_optional(pool)
    .await?;

    Ok(stalled)
}

/// Record the hash qBittorrent reported late for a category-based row.
pub(super) async fn set_hash(pool: &PgPool, id: Uuid, hash: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE torrent_downloads SET qb_hash = $2, updated_at = NOW() WHERE id = $1")
        .bind(id)
        .bind(hash)
        .execute(pool)
        .await?;

    Ok(())
}

#[cfg(test)]
#[path = "tests/torrent_ledger.rs"]
mod tests;
