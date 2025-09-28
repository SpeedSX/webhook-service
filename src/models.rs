use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookRequest {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "TokenId")]
    pub token_id: String,
    #[serde(rename = "MessageObject")]
    pub message_object: MessageObject,
    #[serde(rename = "Message")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageObject {
    #[serde(rename = "Method")]
    pub method: String,
    #[serde(rename = "Value")]
    pub value: String,
    #[serde(rename = "Headers")]
    pub headers: HashMap<String, Vec<String>>,
    #[serde(rename = "QueryParameters")]
    pub query_parameters: Vec<String>,
    #[serde(rename = "Body")]
    pub body: Option<String>,
    #[serde(rename = "BodyObject")]
    pub body_object: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenInfo {
    pub token: String,
    pub created_at: String,
    pub webhook_url: String,
}
