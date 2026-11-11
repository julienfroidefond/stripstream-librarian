use sqlx::PgPool;
use uuid::Uuid;

pub async fn create_library(pool: &PgPool, name: &str) -> Uuid {
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

pub async fn insert_job(
    pool: &PgPool,
    library_id: Option<Uuid>,
    job_type: &str,
    status: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(library_id)
        .bind(job_type)
        .bind(status)
        .execute(pool)
        .await
        .unwrap();
    id
}

pub async fn age_created_at(pool: &PgPool, job_id: Uuid, seconds: i64) {
    sqlx::query(
        "UPDATE index_jobs SET created_at = NOW() - ($2 * INTERVAL '1 second') WHERE id = $1",
    )
    .bind(job_id)
    .bind(seconds)
    .execute(pool)
    .await
    .unwrap();
}

pub async fn age_started_at(pool: &PgPool, job_id: Uuid, seconds: i64) {
    sqlx::query(
        "UPDATE index_jobs SET started_at = NOW() - ($2 * INTERVAL '1 second') WHERE id = $1",
    )
    .bind(job_id)
    .bind(seconds)
    .execute(pool)
    .await
    .unwrap();
}

pub async fn status(pool: &PgPool, job_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

pub async fn job_row(pool: &PgPool, job_id: Uuid) -> sqlx::postgres::PgRow {
    sqlx::query(
        "SELECT status, error_opt, stats_json, progress_percent, finished_at, \
                started_at, phase2_started_at, current_file, processed_files, \
                total_files \
         FROM index_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
    .unwrap()
}
