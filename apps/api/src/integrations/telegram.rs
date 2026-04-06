use axum::{extract::State, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{error::ApiError, state::AppState};

#[derive(Serialize, ToSchema)]
pub struct TelegramTestResponse {
    pub success: bool,
    pub message: String,
}

/// Test Telegram connection by sending a test message
#[utoipa::path(
    get,
    path = "/telegram/test",
    tag = "notifications",
    responses(
        (status = 200, body = TelegramTestResponse),
        (status = 400, description = "Telegram not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn test_telegram(
    State(state): State<AppState>,
) -> Result<Json<TelegramTestResponse>, ApiError> {
    let config = notifications::load_telegram_config(&state.pool)
        .await
        .ok_or_else(|| {
            ApiError::bad_request(
                "Telegram is not configured or disabled. Set bot_token, chat_id, and enable it.",
            )
        })?;

    match notifications::send_test_message(&config).await {
        Ok(()) => Ok(Json(TelegramTestResponse {
            success: true,
            message: "Test message sent successfully".to_string(),
        })),
        Err(e) => Ok(Json(TelegramTestResponse {
            success: false,
            message: format!("Failed to send: {e}"),
        })),
    }
}
