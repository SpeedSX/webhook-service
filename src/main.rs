use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{Html, Json, Response},
    routing::{delete, get, post},
};
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use url::form_urlencoded;
use uuid::Uuid;

mod database;
mod models;

use database::Database;
use models::{MessageObject, TokenInfo, WebhookRequest};

/// Generate webhook URL based on configuration or request headers
fn generate_webhook_url(base_url: &Option<String>, headers: &HeaderMap, token: &str) -> String {
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
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or("").trim())
    };
    let fwd_proto = first("x-forwarded-proto");
    let fwd_host = first("x-forwarded-host");
    let (scheme, host) = match (fwd_proto, fwd_host) {
        (Some(proto), Some(h)) if matches!(proto, "http" | "https") && !h.is_empty() => (proto, h),
        _ => {
            let host = headers.get("host").and_then(|h| h.to_str().ok());
            let host = host.unwrap_or("localhost:3000");
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
    return format!("{}/{}", normalized_base, token);
}

#[derive(Clone)]
pub struct AppState {
    db: Arc<Database>,
    base_url: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize database
    let db = Arc::new(Database::new().await?);

    // Read base URL from environment variable
    let base_url = std::env::var("BASE_URL").ok();
    if let Some(ref url) = base_url {
        info!("Using configured BASE_URL: {}", url);
    } else {
        info!("No BASE_URL configured, will derive from request headers");
    }

    let app_state = AppState { db, base_url };

    // Clone base_url for logging after app_state is moved
    let base_url_for_log = app_state.base_url.clone();

    // Build the application
    let app = Router::new()
        // Web interface first (more specific routes)
        .route("/", get(web_interface))
        .route("/static/{*path}", get(static_files))
        // API routes
        .route("/api/tokens", post(create_token))
        .route("/api/tokens", get(list_tokens))
        .route("/api/tokens/{token}", delete(delete_token))
        // CLI-compatible logs endpoint
        .route("/{token}/log/{count}", get(get_webhook_logs))
        // Webhook endpoint - accepts any HTTP method at /{token}
        .route("/{token}", axum::routing::any(webhook_handler))
        // Webhook endpoint with additional path - accepts any HTTP method at /{token}/*path
        .route("/{token}/{*path}", axum::routing::any(webhook_handler))
        // Apply middleware
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer({
                    if std::env::var("CORS_PERMISSIVE").is_ok() {
                        CorsLayer::permissive()
                    } else {
                        use axum::http::HeaderValue;
                        let allowed = std::env::var("CORS_ALLOWED_ORIGINS")
                            .unwrap_or_else(|_| "http://localhost:3000".into());
                        let origins: Vec<HeaderValue> = allowed
                            .split(',')
                            .filter_map(|s| match s.trim().parse() {
                                Ok(v) => Some(v),
                                Err(e) => {
                                    warn!("Ignoring invalid origin '{}': {e}", s.trim());
                                    None
                                }
                            })
                            .collect();
                        CorsLayer::new()
                            .allow_origin(origins)
                            .allow_methods([
                                Method::GET,
                                Method::POST,
                                Method::DELETE,
                                Method::OPTIONS,
                            ])
                            .allow_headers([header::CONTENT_TYPE])
                    }
                }),
        )
        .with_state(app_state);

    let bind = std::env::var("BIND_ADDR")
        .or_else(|_| std::env::var("PORT").map(|p| format!("0.0.0.0:{p}")))
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!("Listening on {}", bind);

    if let Some(ref url) = base_url_for_log {
        info!("Web interface available at: {}", url);
    } else {
        info!("No BASE_URL set; Web interface available at http://localhost:3000");
    }

    axum::serve(listener, app).await?;

    Ok(())
}

async fn webhook_handler(
    State(state): State<AppState>,
    Path(params): Path<HashMap<String, String>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Extract token from path parameters
    let token = params.get("token").ok_or(StatusCode::BAD_REQUEST)?;

    // Validate token format (should be a UUID)
    let token_uuid = Uuid::parse_str(token).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify token exists in the database
    match state.db.token_exists(token).await {
        Ok(exists) if !exists => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            warn!("Failed to check if token exists: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        _ => {} // Token exists, continue
    }

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
        header_map
            .entry(key_str)
            .or_insert_with(Vec::new)
            .push(value_str);
    }

    // Parse body with a basic size cap (1 MiB)
    if body.len() > 1_048_576 {
        warn!("Request body too large: {} bytes", body.len());
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let body_str = String::from_utf8(body.to_vec()).unwrap_or_default();
    let body_object = if body_str.is_empty() {
        None
    } else {
        serde_json::from_str(&body_str).ok()
    };

    // Create webhook request
    let webhook_request = WebhookRequest {
        id: Uuid::new_v4().to_string(),
        date: chrono::Utc::now().to_rfc3339(),
        token_id: token_uuid.to_string(),
        message_object: MessageObject {
            method: method.to_string(),
            value: uri.to_string(),
            headers: header_map,
            query_parameters: query_params,
            body: if body_str.is_empty() {
                None
            } else {
                Some(body_str)
            },
            body_object,
        },
        message: None,
    };

    // Store the request
    if let Err(e) = state.db.store_webhook_request(&webhook_request).await {
        warn!("Failed to store webhook request: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    info!(
        "Received {} request for token {}: {}",
        method, token, webhook_request.id
    );

    // Return a simple response
    Ok(Json(serde_json::json!({
        "status": "received",
        "id": webhook_request.id,
        "timestamp": webhook_request.date
    })))
}

async fn create_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TokenInfo>, StatusCode> {
    let token = Uuid::new_v4();

    // Generate webhook URL based on configuration or request
    let webhook_url = generate_webhook_url(&state.base_url, &headers, &token.to_string());

    let token_info = TokenInfo {
        token: token.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        webhook_url,
    };

    if let Err(e) = state.db.create_token(&token_info).await {
        warn!("Failed to create token: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    info!("Created new token: {}", token);
    Ok(Json(token_info))
}

async fn list_tokens(State(state): State<AppState>) -> Result<Json<Vec<TokenInfo>>, StatusCode> {
    match state.db.list_tokens().await {
        Ok(tokens) => Ok(Json(tokens)),
        Err(e) => {
            warn!("Failed to list tokens: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn delete_token(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Err(e) = state.db.delete_token(&token).await {
        warn!("Failed to delete token: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    info!("Deleted token: {}", token);
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

async fn get_webhook_logs(
    State(state): State<AppState>,
    Path((token, count)): Path<(String, u32)>,
) -> Result<Json<Vec<WebhookRequest>>, StatusCode> {
    let count = count.min(1000);
    match state.db.get_webhook_requests(&token, count).await {
        Ok(requests) => Ok(Json(requests)),
        Err(e) => {
            warn!("Failed to get webhook requests: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn web_interface() -> Html<&'static str> {
    Html(include_str!("web_interface.html"))
}

async fn static_files(Path(path): Path<String>) -> Result<Response<String>, StatusCode> {
    match path.as_str() {
        "style.css" => {
            let content = include_str!("style.css").to_string();
            Response::builder()
                .header("content-type", "text/css; charset=utf-8")
                .body(content)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        "script.js" => {
            let content = include_str!("script.js").to_string();
            Response::builder()
                .header("content-type", "application/javascript; charset=utf-8")
                .body(content)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        _ => Err(StatusCode::NOT_FOUND),
    }
}
