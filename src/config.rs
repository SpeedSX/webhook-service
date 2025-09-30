use anyhow::Result;
use tracing::info;

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: Option<String>,
    pub bind_addr: String,
    pub cors_permissive: bool,
    pub cors_allowed_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("BASE_URL").ok();
        if let Some(ref url) = base_url {
            info!("Using configured BASE_URL: {}", url);
        } else {
            info!("No BASE_URL configured, will derive from request headers");
        }

        let bind_addr = std::env::var("BIND_ADDR")
            .or_else(|_| std::env::var("PORT").map(|p| format!("0.0.0.0:{p}")))
            .unwrap_or_else(|_| "0.0.0.0:3000".to_string());

        let cors_permissive = std::env::var("CORS_PERMISSIVE").is_ok();

        let cors_allowed_origins = if cors_permissive {
            Vec::new() // Not used in permissive mode
        } else {
            std::env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:3000".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };

        Ok(Self {
            base_url,
            bind_addr,
            cors_permissive,
            cors_allowed_origins,
        })
    }

    pub fn log_startup_info(&self) {
        info!("Listening on {}", self.bind_addr);

        if let Some(ref url) = self.base_url {
            info!("Web interface available at: {}", url);
        } else {
            info!("No BASE_URL set; Web interface available at http://localhost:3000");
        }
    }
}
