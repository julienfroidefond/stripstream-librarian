use super::*;
use sqlx::PgPool;

#[test]
fn sqlx_row_not_found_maps_to_404() {
    let err: ApiError = sqlx::Error::RowNotFound.into();
    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[test]
fn sqlx_pool_timeout_maps_to_503() {
    let err: ApiError = sqlx::Error::PoolTimedOut.into();
    assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn sqlx_other_error_maps_to_500() {
    let err: ApiError = sqlx::Error::PoolClosed.into();
    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn api_error_constructors() {
    assert_eq!(ApiError::bad_request("x").status, StatusCode::BAD_REQUEST);
    assert_eq!(ApiError::not_found("x").status, StatusCode::NOT_FOUND);
    assert_eq!(ApiError::conflict("x").status, StatusCode::CONFLICT);
    assert_eq!(ApiError::unauthorized("x").status, StatusCode::UNAUTHORIZED);
    assert_eq!(ApiError::forbidden("x").status, StatusCode::FORBIDDEN);
    assert_eq!(
        ApiError::internal("x").status,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        ApiError::unprocessable_entity("x").status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[test]
fn sqlx_generic_error_message_is_scrubbed() {
    let err: ApiError = sqlx::Error::PoolClosed.into();

    assert_eq!(err.message, "internal database error");
}

#[test]
fn io_error_message_is_scrubbed() {
    let io = std::io::Error::other("cannot open /secret/path");
    let err: ApiError = io.into();

    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(err.message, "internal IO error");
    assert!(!err.message.contains("/secret/path"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sqlx_unique_violation_does_not_leak_constraint(pool: PgPool) {
    sqlx::query("INSERT INTO users (id, username) VALUES ($1, $2)")
        .bind(uuid::Uuid::new_v4())
        .bind("dup-user")
        .execute(&pool)
        .await
        .unwrap();

    let db_err = sqlx::query("INSERT INTO users (id, username) VALUES ($1, $2)")
        .bind(uuid::Uuid::new_v4())
        .bind("dup-user")
        .execute(&pool)
        .await
        .unwrap_err();

    let err: ApiError = db_err.into();

    assert_eq!(err.status, StatusCode::CONFLICT);
    assert_eq!(err.message, "resource already exists");
    assert!(!err.message.contains("users_username_key"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sqlx_foreign_key_violation_does_not_leak_constraint(pool: PgPool) {
    let db_err = sqlx::query(
        "INSERT INTO api_tokens (id, name, prefix, token_hash, scope, user_id) \
         VALUES ($1, 'tok', $2, 'hash', 'read', $3)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(format!("p{}", uuid::Uuid::new_v4().simple()))
    .bind(uuid::Uuid::new_v4())
    .execute(&pool)
    .await
    .unwrap_err();

    let err: ApiError = db_err.into();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.message, "referenced resource does not exist");
    assert!(!err.message.contains("api_tokens_user_id_fkey"));
}
