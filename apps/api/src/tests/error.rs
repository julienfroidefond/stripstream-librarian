use super::*;

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
