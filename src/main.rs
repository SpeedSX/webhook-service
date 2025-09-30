use std::sync::Arc;

mod config;
mod database;
mod error;
mod handlers;
mod models;
mod services;

use config::Config;
use database::Database;
use handlers::create_router;
use services::{TokenService, WebhookService};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize configuration
    let config = Config::from_env()?;

    // Initialize database
    let db = Arc::new(Database::new().await?);

    let app_state = handlers::AppState {
        webhook_service: WebhookService::new(db.clone()),
        token_service: TokenService::new(db, config.base_url.clone()),
    };

    // Build the application
    let app = create_router(app_state, &config);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;

    // Log startup information
    config.log_startup_info();

    axum::serve(listener, app).await?;

    Ok(())
}
