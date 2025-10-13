use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::borrow::Cow;

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

    #[error("Common browser file not found: {0}")]
    CommonFileNotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message): (StatusCode, Cow<str>) = match &self {
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error".into()),
            AppError::JsonParsing(_) => (StatusCode::BAD_REQUEST, "Invalid JSON".into()),
            AppError::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, "IO error".into()),
            AppError::InvalidUuid(_) => (StatusCode::BAD_REQUEST, "Invalid UUID format".into()),
            AppError::EnvVar(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error".into()),
            AppError::TokenNotFound => (StatusCode::NOT_FOUND, "Token not found".into()),
            AppError::InvalidToken => (
                StatusCode::BAD_REQUEST,
                "Invalid token format. Tokens must be valid UUIDs (e.g., 550e8400-e29b-41d4-a716-446655440000)".into(),
            ),
            AppError::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large".into()),
            AppError::InternalServerError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into())
            }
            AppError::NotFound => (StatusCode::NOT_FOUND, "Resource not found".into()),
            AppError::CommonFileNotFound(path) => (
                StatusCode::NOT_FOUND,
                format!("Common browser file not found: {}", path).into()
            ),
        };

        tracing::warn!("Error occurred: {}", self);

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}
