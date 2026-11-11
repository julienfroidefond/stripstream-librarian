mod claim;
mod cleanup;
mod helpers;
mod lifecycle;
mod registry;
mod support;

use sqlx::PgPool;
use uuid::Uuid;

use crate::status::as_strings;
use crate::JobState;

#[sqlx::test(migrations = "../../infra/migrations")]
async fn every_job_state_is_accepted_by_the_database(pool: PgPool) {
    for state in JobState::ALL {
        insert_state_job(&pool, state).await;
    }
}

async fn insert_state_job(pool: &PgPool, state: JobState) {
    sqlx::query("INSERT INTO index_jobs (id, type, status) VALUES ($1, 'scan', $2)")
        .bind(Uuid::new_v4())
        .bind(state.as_str())
        .execute(pool)
        .await
        .unwrap();
}

#[test]
fn job_state_classification_is_consistent() {
    for state in JobState::ALL {
        assert_eq!(
            state.is_active(),
            state.is_in_flight() && !matches!(state, JobState::Pending)
        );
        assert_ne!(state.is_terminal(), state.is_in_flight());
        let parsed: JobState = state.as_str().parse().unwrap();
        assert_eq!(parsed, state);
    }
    assert_eq!(as_strings(&JobState::ACTIVE).len(), 3);
    assert_eq!(as_strings(&JobState::IN_FLIGHT).len(), 4);
    assert_eq!(as_strings(&JobState::TERMINAL).len(), 4);
}
