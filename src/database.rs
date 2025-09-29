use anyhow::Result;
use sqlx::{sqlite::{SqlitePool, SqliteConnectOptions}, Row};
use std::collections::HashMap;
use std::time::Duration;

use crate::models::{MessageObject, TokenInfo, WebhookRequest};

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new() -> Result<Self> {
        // Get current directory and create database path
        let current_dir = std::env::current_dir()?;
        let db_path = current_dir.join("webhook_service.db");
        
        // Ensure the directory exists and is writable
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Create connection options with concurrency-friendly settings
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        
        // Create the database connection
        let pool = SqlitePool::connect_with(options).await?;
        
        // Create tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tokens (
                token TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                webhook_url TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS webhook_requests (
                id TEXT PRIMARY KEY,
                date TEXT NOT NULL,
                token_id TEXT NOT NULL,
                method TEXT NOT NULL,
                value TEXT NOT NULL,
                headers TEXT NOT NULL,
                query_parameters TEXT NOT NULL,
                body TEXT,
                body_object TEXT,
                message TEXT,
                FOREIGN KEY (token_id) REFERENCES tokens (token)
            )
            "#,
        )
        .execute(&pool)
        .await?;

        // Create index for faster queries
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_webhook_requests_token_id ON webhook_requests (token_id)")
            .execute(&pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_webhook_requests_date ON webhook_requests (date)")
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }

    pub async fn create_token(&self, token_info: &TokenInfo) -> Result<()> {
        sqlx::query(
            "INSERT INTO tokens (token, created_at, webhook_url) VALUES (?, ?, ?)",
        )
        .bind(&token_info.token)
        .bind(&token_info.created_at)
        .bind(&token_info.webhook_url)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_tokens(&self) -> Result<Vec<TokenInfo>> {
        let rows = sqlx::query("SELECT token, created_at, webhook_url FROM tokens ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;

        let tokens = rows
            .into_iter()
            .map(|row| TokenInfo {
                token: row.get("token"),
                created_at: row.get("created_at"),
                webhook_url: row.get("webhook_url"),
            })
            .collect();

        Ok(tokens)
    }

    pub async fn token_exists(&self, token: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tokens WHERE token = ?")
            .bind(token)
            .fetch_one(&self.pool)
            .await?;

        Ok(count > 0)
    }

    pub async fn delete_token(&self, token: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM webhook_requests WHERE token_id = ?")
            .bind(token)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM tokens WHERE token = ?")
            .bind(token)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn store_webhook_request(&self, request: &WebhookRequest) -> Result<()> {
        let headers_json = serde_json::to_string(&request.message_object.headers)?;
        let query_params_json = serde_json::to_string(&request.message_object.query_parameters)?;
        let body_object_json = request.message_object.body_object.as_ref()
            .map(|obj| serde_json::to_string(obj))
            .transpose()?;

        sqlx::query(
            r#"
            INSERT INTO webhook_requests 
            (id, date, token_id, method, value, headers, query_parameters, body, body_object, message)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&request.id)
        .bind(&request.date)
        .bind(&request.token_id)
        .bind(&request.message_object.method)
        .bind(&request.message_object.value)
        .bind(headers_json)
        .bind(query_params_json)
        .bind(&request.message_object.body)
        .bind(body_object_json)
        .bind(&request.message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_webhook_requests(&self, token: &str, count: u32) -> Result<Vec<WebhookRequest>> {
        let rows = sqlx::query(
            r#"
            SELECT id, date, token_id, method, value, headers, query_parameters, body, body_object, message
            FROM webhook_requests 
            WHERE token_id = ? 
            ORDER BY date DESC 
            LIMIT ?
            "#
        )
        .bind(token)
        .bind(count as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut requests = Vec::new();
        for row in rows {
            let headers: HashMap<String, Vec<String>> = serde_json::from_str(row.get("headers"))?;
            let query_parameters: Vec<String> = serde_json::from_str(row.get("query_parameters"))?;
            let body_object: Option<serde_json::Value> = row
                .get::<Option<String>, _>("body_object")
                .map(|s| serde_json::from_str(&s))
                .transpose()?;

            let request = WebhookRequest {
                id: row.get("id"),
                date: row.get("date"),
                token_id: row.get("token_id"),
                message_object: MessageObject {
                    method: row.get("method"),
                    value: row.get("value"),
                    headers,
                    query_parameters,
                    body: row.get("body"),
                    body_object,
                },
                message: row.get("message"),
            };

            requests.push(request);
        }

        Ok(requests)
    }
}
