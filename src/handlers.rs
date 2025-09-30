use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, Method, Uri, header},
    response::{Html, Json, Response},
    routing::{any, delete, get, post},
};
use std::collections::HashMap;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use url::form_urlencoded;
use uuid::Uuid;

use crate::config::Config;
use crate::error::AppError;
use crate::models::{TokenInfo, WebhookRequest};
use crate::services::{TokenService, WebhookService};

#[derive(Clone)]
pub struct AppState {
    pub webhook_service: WebhookService,
    pub token_service: TokenService,
}

pub fn create_router(app_state: AppState, config: &Config) -> Router {
    Router::new()
        // Web interface first (more specific routes)
        .route("/", get(web_interface))
        .route("/static/{*path}", get(static_files))
        // Common browser files that should return 404
        .route(
            "/favicon.ico",
            get(|uri: Uri| not_found_handler_with_path(uri, "favicon.ico")),
        )
        .route(
            "/robots.txt",
            get(|uri: Uri| not_found_handler_with_path(uri, "robots.txt")),
        )
        // API routes
        .route("/api/tokens", post(create_token))
        .route("/api/tokens", get(list_tokens))
        .route("/api/tokens/{token}", delete(delete_token))
        // CLI-compatible logs endpoint
        .route("/{token}/log/{count}", get(get_webhook_logs))
        // Webhook endpoint - accepts any HTTP method at /{token}
        .route("/{token}", any(webhook_handler))
        // Webhook endpoint with additional path - accepts any HTTP method at /{token}/*path
        .route("/{token}/{*path}", any(webhook_handler))
        // Apply middleware
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(create_cors_layer(config)),
        )
        .with_state(app_state)
}

fn create_cors_layer(config: &Config) -> CorsLayer {
    if config.cors_permissive {
        CorsLayer::permissive()
    } else {
        use axum::http::HeaderValue;
        let origins: Vec<HeaderValue> = config
            .cors_allowed_origins
            .iter()
            .filter_map(|s| match s.parse() {
                Ok(v) => Some(v),
                Err(e) => {
                    warn!("Ignoring invalid origin '{}': {e}", s);
                    None
                }
            })
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE])
    }
}

async fn webhook_handler(
    State(state): State<AppState>,
    Path(params): Path<HashMap<String, String>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> std::result::Result<Json<serde_json::Value>, AppError> {
    // Extract token from path parameters
    let token = params.get("token").ok_or(AppError::InvalidToken)?;

    // Quick check for common browser-requested files to avoid unnecessary UUID parsing
    let common_files = ["favicon.ico", "robots.txt", "sitemap.xml", "manifest.json"];
    if common_files.contains(&token.as_str()) {
        tracing::debug!(
            "Browser file request detected in webhook handler: '{}'",
            token
        );
        return Err(AppError::NotFound);
    }

    // Validate token format (should be a UUID)
    Uuid::parse_str(token).map_err(|e| {
        tracing::warn!("Invalid UUID token received: '{}' - {}", token, e);
        AppError::InvalidToken
    })?;

    // Parse query parameters
    let query_params: Vec<String> = uri
        .query()
        .map(|q| {
            form_urlencoded::parse(q.as_bytes())
                .map(|(k, v)| format!("{}={}", k, v))
                .collect()
        })
        .unwrap_or_default();

    // Convert headers to the expected format
    let mut header_map: HashMap<String, Vec<String>> = HashMap::new();
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_string();
        let value_str = String::from_utf8_lossy(value.as_bytes()).to_string();
        header_map.entry(key_str).or_default().push(value_str);
    }

    // Parse body with a basic size cap (1 MiB)
    if body.len() > 1_048_576 {
        return Err(AppError::PayloadTooLarge);
    }
    let body_str = String::from_utf8(body.to_vec()).unwrap_or_default();
    let body_object = if body_str.is_empty() {
        None
    } else {
        serde_json::from_str(&body_str).ok()
    };

    // Process webhook through service layer
    let request_id = state
        .webhook_service
        .process_webhook(
            token,
            method.as_ref(),
            &uri.to_string(),
            header_map,
            query_params,
            if body_str.is_empty() {
                None
            } else {
                Some(body_str)
            },
            body_object,
        )
        .await?;

    info!(
        "Received {} request for token {}: {}",
        method, token, request_id
    );

    // Return a simple response
    Ok(Json(serde_json::json!({
        "status": "received",
        "id": request_id,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

async fn create_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<TokenInfo>, AppError> {
    let token_info = state.token_service.create_token(&headers).await?;
    Ok(Json(token_info))
}

async fn list_tokens(
    State(state): State<AppState>,
) -> std::result::Result<Json<Vec<TokenInfo>>, AppError> {
    let tokens = state.token_service.list_tokens().await?;
    Ok(Json(tokens))
}

async fn delete_token(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, AppError> {
    state.token_service.delete_token(&token).await?;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

async fn get_webhook_logs(
    State(state): State<AppState>,
    Path((token, count)): Path<(String, u32)>,
) -> std::result::Result<Json<Vec<WebhookRequest>>, AppError> {
    let requests = state
        .webhook_service
        .get_webhook_logs(&token, count)
        .await?;
    Ok(Json(requests))
}

async fn web_interface() -> Html<&'static str> {
    Html(include_str!("web_interface.html"))
}

async fn not_found_handler_with_path(
    _uri: Uri,
    resource: &str,
) -> std::result::Result<Json<serde_json::Value>, AppError> {
    tracing::debug!("Browser requested common file: {}", resource);
    Err(AppError::NotFound)
}

async fn static_files(Path(path): Path<String>) -> std::result::Result<Response<String>, AppError> {
    match path.as_str() {
        "style.css" => {
            let content = include_str!("style.css").to_string();
            Ok(Response::builder()
                .header("content-type", "text/css; charset=utf-8")
                .body(content)
                .map_err(|_| AppError::InternalServerError)?)
        }
        "script.js" => {
            let content = include_str!("script.js").to_string();
            Ok(Response::builder()
                .header("content-type", "application/javascript; charset=utf-8")
                .body(content)
                .map_err(|_| AppError::InternalServerError)?)
        }
        _ => Err(AppError::NotFound),
    }
}
