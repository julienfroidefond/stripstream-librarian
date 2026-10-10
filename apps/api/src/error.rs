use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    pub fn unprocessable_entity(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: &self.message,
            }),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => Self::not_found("resource not found"),
            sqlx::Error::PoolTimedOut => Self {
                status: StatusCode::SERVICE_UNAVAILABLE,
                message: "database pool timed out".to_string(),
            },
            sqlx::Error::Database(db_err) => {
                // PostgreSQL unique_violation = 23505, foreign_key_violation = 23503
                match db_err.code().unwrap_or_default().as_ref() {
                    "23505" => {
                        tracing::warn!("database unique violation: {db_err}");
                        Self::conflict("resource already exists")
                    }
                    "23503" => {
                        tracing::warn!("database foreign key violation: {db_err}");
                        Self::bad_request("referenced resource does not exist")
                    }
                    _ => {
                        tracing::error!("database error: {err}");
                        Self::internal("internal database error")
                    }
                }
            }
            _ => {
                tracing::error!("database error: {err}");
                Self::internal("internal database error")
            }
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        tracing::error!("IO error: {err}");
        Self::internal("internal IO error")
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        tracing::error!("application error: {err:#}");
        Self::internal("internal error")
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        // reqwest errors embed the request URL (which may carry API keys in the query string).
        tracing::error!("HTTP client error: {err}");
        Self::internal("upstream HTTP request failed")
    }
}

#[cfg(test)]
#[path = "tests/error.rs"]
mod tests;
