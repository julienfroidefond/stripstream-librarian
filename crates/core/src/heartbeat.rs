//! Liveness heartbeats shared by the API and the indexer.
//!
//! Both processes run in separate containers and can die independently. A dead
//! indexer is otherwise invisible: the job queue is simply empty, which looks
//! the same as an idle healthy system. Each service therefore upserts its row
//! in `service_heartbeats` every [`HEARTBEAT_INTERVAL`]; the diagnostics page
//! flags it offline once `last_seen_at` is older than [`HEARTBEAT_TIMEOUT`].

use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// How often each service refreshes its heartbeat row.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
/// A service silent for longer than this is reported offline.
pub const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(90);

/// Heartbeat key of the REST API.
pub const API_SERVICE: &str = "api";
/// Heartbeat key of the background indexer.
pub const INDEXER_SERVICE: &str = "indexer";

/// One `service_heartbeats` row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ServiceHeartbeat {
    pub service: String,
    pub version: String,
    pub started_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

impl ServiceHeartbeat {
    /// Whether the service beat recently enough to be considered alive.
    pub fn is_online(&self, now: DateTime<Utc>) -> bool {
        self.seconds_since_seen(now) < HEARTBEAT_TIMEOUT.as_secs() as i64
    }

    /// Seconds elapsed since the last beat (clamped to zero for clock skew).
    pub fn seconds_since_seen(&self, now: DateTime<Utc>) -> i64 {
        now.signed_duration_since(self.last_seen_at)
            .num_seconds()
            .max(0)
    }
}

/// Upsert the heartbeat of one service.
pub async fn record(
    pool: &PgPool,
    service: &str,
    version: &str,
    started_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO service_heartbeats (service, version, started_at, last_seen_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (service) DO UPDATE
           SET version = EXCLUDED.version,
               started_at = EXCLUDED.started_at,
               last_seen_at = NOW()
        "#,
    )
    .bind(service)
    .bind(version)
    .bind(started_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// Spawn the background loop keeping a service heartbeat fresh.
///
/// The first beat is written immediately so a freshly started service shows up
/// on the diagnostics page without waiting for the first interval.
pub fn spawn(pool: PgPool, service: &'static str, version: &'static str) {
    let started_at = Utc::now();

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(HEARTBEAT_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;
            if let Err(error) = record(&pool, service, version, started_at).await {
                tracing::warn!(service, %error, "failed to record heartbeat");
            }
        }
    });
}

/// Every known heartbeat, ordered by service name.
pub async fn load_all(pool: &PgPool) -> sqlx::Result<Vec<ServiceHeartbeat>> {
    sqlx::query_as::<_, ServiceHeartbeat>(
        "SELECT service, version, started_at, last_seen_at \
         FROM service_heartbeats ORDER BY service",
    )
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;

    fn heartbeat(now: DateTime<Utc>, seconds_ago: i64) -> ServiceHeartbeat {
        ServiceHeartbeat {
            service: API_SERVICE.to_string(),
            version: "1.2.3".to_string(),
            started_at: now - TimeDelta::hours(1),
            last_seen_at: now - TimeDelta::seconds(seconds_ago),
        }
    }

    #[test]
    fn fresh_heartbeat_is_online() {
        let now = Utc::now();
        let hb = heartbeat(now, HEARTBEAT_INTERVAL.as_secs() as i64);
        assert!(hb.is_online(now));
        assert_eq!(
            hb.seconds_since_seen(now),
            HEARTBEAT_INTERVAL.as_secs() as i64
        );
    }

    #[test]
    fn silent_heartbeat_is_offline() {
        let now = Utc::now();
        let hb = heartbeat(now, HEARTBEAT_TIMEOUT.as_secs() as i64);
        assert!(!hb.is_online(now));
    }

    #[test]
    fn clock_skew_never_reports_negative_age() {
        let now = Utc::now();
        let mut hb = heartbeat(now, 0);
        hb.last_seen_at = now + TimeDelta::seconds(10);
        assert_eq!(hb.seconds_since_seen(now), 0);
    }
}
