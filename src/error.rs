use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JSON parsing error: {0}")]
    JsonParsing(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[error("Environment variable error: {0}")]
    EnvVar(#[from] std::env::VarError),

    #[error("Token not found")]
    TokenNotFound,

    #[error("Invalid token format - tokens must be valid UUIDs")]
    InvalidToken,

    #[error("Request body too large")]
    PayloadTooLarge,

    #[error("Internal server error")]
    InternalServerError,

    #[error("Resource not found")]
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
            AppError::JsonParsing(_) => (StatusCode::BAD_REQUEST, "Invalid JSON"),
            AppError::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, "IO error"),
            AppError::InvalidUuid(_) => (StatusCode::BAD_REQUEST, "Invalid UUID format"),
            AppError::EnvVar(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error"),
            AppError::TokenNotFound => (StatusCode::NOT_FOUND, "Token not found"),
            AppError::InvalidToken => (
                StatusCode::BAD_REQUEST,
                "Invalid token format. Tokens must be valid UUIDs (e.g., 550e8400-e29b-41d4-a716-446655440000)",
            ),
            AppError::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large"),
            AppError::InternalServerError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
            AppError::NotFound => (StatusCode::NOT_FOUND, "Resource not found"),
        };

        tracing::warn!("Error occurred: {}", self);

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}
