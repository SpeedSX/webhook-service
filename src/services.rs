use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::database::Database;
use crate::error::AppError;
use crate::models::{MessageObject, TokenInfo, WebhookRequest};

/// Generate webhook URL based on configuration or request headers
pub fn generate_webhook_url(
    base_url: &Option<String>,
    headers: &HashMap<String, Vec<String>>,
    token: &str,
) -> String {
    // First try to use configured BASE_URL
    if let Some(configured_base) = base_url {
        let normalized_base = configured_base.trim_end_matches('/');
        return format!("{}/{}", normalized_base, token);
    }

    // Fallback: extract from request headers and URI
    // Prefer forwarded headers set by proxies/CDNs
    let first = |name: &str| {
        headers
            .get(name)
            .and_then(|values| values.first())
            .map(|s| s.split(',').next().unwrap_or("").trim())
    };
    let fwd_proto = first("x-forwarded-proto");
    let fwd_host = first("x-forwarded-host");
    let (scheme, host) = match (fwd_proto, fwd_host) {
        (Some(proto), Some(h)) if matches!(proto, "http" | "https") && !h.is_empty() => (proto, h),
        _ => {
            let host = headers.get("host").and_then(|values| values.first());
            let host = host.map(|s| s.as_str()).unwrap_or("localhost:3000");
            let scheme = if host.starts_with("localhost") || host.starts_with("127.0.0.1") {
                "http"
            } else {
                "https"
            };
            (scheme, host)
        }
    };

    let base = format!("{}://{}", scheme, host);
    let normalized_base = base.trim_end_matches('/');
    format!("{}/{}", normalized_base, token)
}

#[derive(Clone)]
pub struct WebhookService {
    db: Arc<Database>,
}

impl WebhookService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_webhook(
        &self,
        token: &str,
        method: &str,
        uri: &str,
        headers: HashMap<String, Vec<String>>,
        query_params: Vec<String>,
        body: Option<String>,
        body_object: Option<serde_json::Value>,
    ) -> Result<String, AppError> {
        // Validate token format (should be a UUID)
        Uuid::parse_str(token).map_err(|e| {
            warn!(
                "Invalid UUID token received in webhook processing: '{}' - {}",
                token, e
            );
            AppError::InvalidToken
        })?;

        // Verify token exists in the database
        if !self.db.token_exists(token).await.map_err(|e| {
            warn!("Failed to check if token exists: {}", e);
            AppError::InternalServerError
        })? {
            return Err(AppError::TokenNotFound);
        }

        // Create webhook request
        let webhook_request = WebhookRequest {
            id: Uuid::new_v4().to_string(),
            date: chrono::Utc::now().to_rfc3339(),
            token_id: token.to_string(),
            message_object: MessageObject {
                method: method.to_string(),
                value: uri.to_string(),
                headers,
                query_parameters: query_params,
                body,
                body_object,
            },
            message: None,
        };

        // Store the request
        self.db
            .store_webhook_request(&webhook_request)
            .await
            .map_err(|e| {
                warn!("Failed to store webhook request: {}", e);
                AppError::InternalServerError
            })?;

        info!(
            "Received {} request for token {}: {}",
            method, token, webhook_request.id
        );

        Ok(webhook_request.id)
    }

    pub async fn get_webhook_logs(
        &self,
        token: &str,
        count: u32,
    ) -> Result<Vec<WebhookRequest>, AppError> {
        let count = count.min(1000);
        let requests = self
            .db
            .get_webhook_requests(token, count)
            .await
            .map_err(|e| {
                warn!("Failed to get webhook requests: {}", e);
                AppError::InternalServerError
            })?;
        Ok(requests)
    }
}

#[derive(Clone)]
pub struct TokenService {
    db: Arc<Database>,
    base_url: Option<String>,
}

impl TokenService {
    pub fn new(db: Arc<Database>, base_url: Option<String>) -> Self {
        Self { db, base_url }
    }

    pub async fn create_token(
        &self,
        headers: &HashMap<String, Vec<String>>,
    ) -> Result<TokenInfo, AppError> {
        let token = Uuid::new_v4();

        // Generate webhook URL based on configuration or request
        let webhook_url = generate_webhook_url(&self.base_url, headers, &token.to_string());

        let token_info = TokenInfo {
            token: token.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            webhook_url,
        };

        self.db.create_token(&token_info).await.map_err(|e| {
            warn!("Failed to create token: {}", e);
            AppError::InternalServerError
        })?;

        info!("Created new token: {}", token);
        Ok(token_info)
    }

    pub async fn list_tokens(&self) -> Result<Vec<TokenInfo>, AppError> {
        let tokens = self.db.list_tokens().await.map_err(|e| {
            warn!("Failed to list tokens: {}", e);
            AppError::InternalServerError
        })?;
        Ok(tokens)
    }

    pub async fn delete_token(&self, token: &str) -> Result<(), AppError> {
        self.db.delete_token(token).await.map_err(|e| {
            warn!("Failed to delete token: {}", e);
            AppError::InternalServerError
        })?;

        info!("Deleted token: {}", token);
        Ok(())
    }
}
