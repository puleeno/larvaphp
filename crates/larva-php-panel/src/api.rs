//! Data models and API route definitions for the control panel.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::Utc;

// ============================================================================
// Data Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Website {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub document_root: String,
    pub php_version: String,
    pub ssl_enabled: bool,
    pub ssl_cert_path: Option<String>,
    pub ssl_key_path: Option<String>,
    pub acme_email: Option<String>,
    pub acme_domains: Option<String>,
    pub worker_count: u32,
    pub listen_port: Option<u16>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateWebsite {
    pub name: String,
    pub domain: String,
    pub document_root: String,
    pub php_version: Option<String>,
    pub ssl_enabled: Option<bool>,
    pub acme_email: Option<String>,
    pub acme_domains: Option<String>,
    pub worker_count: Option<u32>,
    pub listen_port: Option<u16>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebsite {
    pub name: Option<String>,
    pub document_root: Option<String>,
    pub php_version: Option<String>,
    pub ssl_enabled: Option<bool>,
    pub ssl_cert_path: Option<String>,
    pub ssl_key_path: Option<String>,
    pub acme_email: Option<String>,
    pub acme_domains: Option<String>,
    pub worker_count: Option<u32>,
    pub listen_port: Option<u16>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslCertificate {
    pub id: String,
    pub domain: String,
    pub cert_path: String,
    pub key_path: String,
    pub issuer: String,
    pub expires_at: Option<String>,
    pub auto_renew: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AcmeRequest {
    pub email: String,
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub id: String,
    pub name: String,
    pub status: String, // "running", "stopped", "error"
    pub pid: Option<u32>,
    pub requests: u64,
    pub uptime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_websites: u64,
    pub active_websites: u64,
    pub total_workers: u64,
    pub running_workers: u64,
    pub ssl_certificates: u64,
    pub requests_today: u64,
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub disk_usage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub role: String,
    pub email: Option<String>,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSetting {
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub user_id: Option<String>,
    pub action: String,
    pub target: Option<String>,
    pub details: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg),
        }
    }
}

// ============================================================================
// API Route Helpers
// ============================================================================

pub fn api_json<T: Serialize>(data: T) -> axum::response::Json<ApiResponse<T>> {
    axum::response::Json(ApiResponse::ok(data))
}

pub fn api_err<T>(msg: &str) -> axum::response::Json<ApiResponse<T>> {
    axum::response::Json(ApiResponse::err(msg.to_string()))
}

// Generate a short ID
pub fn short_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}
