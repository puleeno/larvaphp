//! Axum-based web control panel server.
//! Serves the SPA HTML and JSON API for managing LarvaPHP v2.

use std::net::SocketAddr;
use axum::{
    Router,
    routing::{get, post, delete},
    extract::{State, Query, Path},
    response::{Html, Json},
    http::{Method, header},
};
use tower_http::cors::CorsLayer;
use serde::Deserialize;
use crate::db::PanelDb;
use crate::api::{
    Website, CreateWebsite, UpdateWebsite, SslCertificate, AcmeRequest,
    WorkerStatus, DashboardStats, User, CreateUser, LoginRequest, LoginResponse,
    Setting, UpdateSetting, ApiResponse, api_json, api_err, short_id, now_iso,
};

/// Panel state shared across handlers.
#[derive(Clone)]
pub struct PanelState {
    pub db: PanelDb,
}

/// Query params for file listing.
#[derive(Deserialize)]
pub struct FileQuery {
    pub dir: Option<String>,
    pub path: Option<String>,
}

impl PanelState {
    pub fn new(db: PanelDb) -> Self {
        Self { db }
    }

    pub fn router(self) -> Router {
        Router::new()
            .route("/api/stats", get(stats_handler))
            .route("/api/websites", get(list_websites).post(create_website))
            .route(
                "/api/websites/{id}",
                get(get_website).put(update_website).delete(delete_website),
            )
            .route("/api/ssl", get(list_ssl))
            .route("/api/ssl/issue", post(issue_ssl))
            .route("/api/ssl/{id}", delete(delete_ssl_cert))
            .route("/api/ssl/{id}/renew", post(renew_ssl_cert))
            .route("/api/workers", get(list_workers))
            .route("/api/workers/{id}/restart", post(restart_worker))
            .route("/api/workers/{id}/stop", post(stop_worker))
            .route("/api/users", get(list_users).post(create_user))
            .route("/api/files", get(list_files))
            .route("/api/settings", get(list_settings).put(update_setting))
            .route("/api/auth/login", post(login))
            .fallback(spa_fallback)
            .with_state(self)
            .layer(
                CorsLayer::new()
                    .allow_origin(tower_http::cors::Any)
                    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                    .allow_headers([header::CONTENT_TYPE]),
            )
    }

    pub async fn run(self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let app = self.router();
        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!("LarvaPHP Panel listening on http://{}", addr);
        axum::serve(listener, app).await?;
        Ok(())
    }
}

// ============================================================================
// API Handlers
// ============================================================================

async fn stats_handler(
    State(state): State<PanelState>,
) -> Json<ApiResponse<DashboardStats>> {
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM websites", [], |r| r.get(0)).unwrap_or(0);
        let active: i64 = conn.query_row("SELECT COUNT(*) FROM websites WHERE is_active = 1", [], |r| r.get(0)).unwrap_or(0);
        let ssl_count: i64 = conn.query_row("SELECT COUNT(*) FROM ssl_certificates", [], |r| r.get(0)).unwrap_or(0);
        Ok(DashboardStats {
            total_websites: total as u64,
            active_websites: active as u64,
            total_workers: 0,
            running_workers: 0,
            ssl_certificates: ssl_count as u64,
            requests_today: 0,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            disk_usage: 0.0,
        })
    })();
    match result {
        Ok(s) => api_json(s),
        Err(e) => api_err(&format!("stats error: {}", e)),
    }
}

async fn list_websites(
    State(state): State<PanelState>,
) -> Json<ApiResponse<Vec<Website>>> {
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, domain, document_root, php_version, ssl_enabled, ssl_cert_path, ssl_key_path, acme_email, acme_domains, worker_count, is_active, created_at, updated_at FROM websites ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Website {
                id: r.get(0)?,
                name: r.get(1)?,
                domain: r.get(2)?,
                document_root: r.get(3)?,
                php_version: r.get(4)?,
                ssl_enabled: r.get::<_, i64>(5)? != 0,
                ssl_cert_path: r.get(6)?,
                ssl_key_path: r.get(7)?,
                acme_email: r.get(8)?,
                acme_domains: r.get(9)?,
                worker_count: r.get::<_, i64>(10)? as u32,
                is_active: r.get::<_, i64>(11)? != 0,
                created_at: r.get(12)?,
                updated_at: r.get(13)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        Ok(out)
    })();
    match result {
        Ok(w) => api_json(w),
        Err(e) => api_err(&format!("list websites: {}", e)),
    }
}

async fn create_website(
    State(state): State<PanelState>,
    Json(payload): Json<CreateWebsite>,
) -> Json<ApiResponse<Website>> {
    let id = short_id();
    let now = now_iso();
    let php_version = payload.php_version.clone().unwrap_or_else(|| "8.3".into());
    let worker_count = payload.worker_count.unwrap_or(1);
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        conn.execute(
            "INSERT INTO websites (id, name, domain, document_root, php_version, ssl_enabled, acme_email, acme_domains, worker_count, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                id,
                payload.name,
                payload.domain,
                payload.document_root,
                php_version,
                payload.ssl_enabled.unwrap_or(false) as i64,
                payload.acme_email,
                payload.acme_domains,
                worker_count,
                1,
                now,
                now,
            ],
        )?;
        Ok(Website {
            id,
            name: payload.name,
            domain: payload.domain,
            document_root: payload.document_root,
            php_version,
            ssl_enabled: payload.ssl_enabled.unwrap_or(false),
            ssl_cert_path: None,
            ssl_key_path: None,
            acme_email: payload.acme_email,
            acme_domains: payload.acme_domains,
            worker_count,
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        })
    })();
    match result {
        Ok(w) => api_json(w),
        Err(e) => api_err(&format!("create website: {}", e)),
    }
}

async fn get_website(
    State(state): State<PanelState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<Website>> {
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        conn.query_row(
            "SELECT id, name, domain, document_root, php_version, ssl_enabled, ssl_cert_path, ssl_key_path, acme_email, acme_domains, worker_count, is_active, created_at, updated_at FROM websites WHERE id = ?1",
            rusqlite::params![id],
            |r| Ok(Website {
                id: r.get(0)?,
                name: r.get(1)?,
                domain: r.get(2)?,
                document_root: r.get(3)?,
                php_version: r.get(4)?,
                ssl_enabled: r.get::<_, i64>(5)? != 0,
                ssl_cert_path: r.get(6)?,
                ssl_key_path: r.get(7)?,
                acme_email: r.get(8)?,
                acme_domains: r.get(9)?,
                worker_count: r.get::<_, i64>(10)? as u32,
                is_active: r.get::<_, i64>(11)? != 0,
                created_at: r.get(12)?,
                updated_at: r.get(13)?,
            }),
        )
    })();
    match result {
        Ok(w) => api_json(w),
        Err(e) => api_err(&format!("website not found: {}", e)),
    }
}

async fn update_website(
    State(state): State<PanelState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateWebsite>,
) -> Json<ApiResponse<Website>> {
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        let now = now_iso();
        let id_clone = id.clone();
        conn.execute(
            "UPDATE websites SET
                name = COALESCE(NULLIF(?1, ''), name),
                document_root = COALESCE(NULLIF(?2, ''), document_root),
                php_version = COALESCE(NULLIF(?3, ''), php_version),
                ssl_enabled = COALESCE(?4, ssl_enabled),
                ssl_cert_path = COALESCE(NULLIF(?5, ''), ssl_cert_path),
                ssl_key_path = COALESCE(NULLIF(?6, ''), ssl_key_path),
                acme_email = COALESCE(NULLIF(?7, ''), acme_email),
                acme_domains = COALESCE(NULLIF(?8, ''), acme_domains),
                worker_count = COALESCE(?9, worker_count),
                is_active = COALESCE(?10, is_active),
                updated_at = ?11
              WHERE id = ?12",
            rusqlite::params![
                payload.name.clone().unwrap_or_default(),
                payload.document_root.clone().unwrap_or_default(),
                payload.php_version.clone().unwrap_or_default(),
                payload.ssl_enabled.map(|v| v as i64),
                payload.ssl_cert_path.clone().unwrap_or_default(),
                payload.ssl_key_path.clone().unwrap_or_default(),
                payload.acme_email.clone().unwrap_or_default(),
                payload.acme_domains.clone().unwrap_or_default(),
                payload.worker_count.map(|v| v as i64),
                payload.is_active.map(|v| v as i64),
                now,
                id_clone,
            ],
        )?;

        conn.query_row(
            "SELECT id, name, domain, document_root, php_version, ssl_enabled, ssl_cert_path, ssl_key_path, acme_email, acme_domains, worker_count, is_active, created_at, updated_at FROM websites WHERE id = ?1",
            rusqlite::params![id],
            |r| Ok(Website {
                id: r.get(0)?,
                name: r.get(1)?,
                domain: r.get(2)?,
                document_root: r.get(3)?,
                php_version: r.get(4)?,
                ssl_enabled: r.get::<_, i64>(5)? != 0,
                ssl_cert_path: r.get(6)?,
                ssl_key_path: r.get(7)?,
                acme_email: r.get(8)?,
                acme_domains: r.get(9)?,
                worker_count: r.get::<_, i64>(10)? as u32,
                is_active: r.get::<_, i64>(11)? != 0,
                created_at: r.get(12)?,
                updated_at: r.get(13)?,
            }),
        )
    })();
    match result {
        Ok(w) => api_json(w),
        Err(e) => api_err(&format!("update website: {}", e)),
    }
}

async fn delete_website(
    State(state): State<PanelState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        conn.execute("DELETE FROM websites WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })();
    match result {
        Ok(()) => api_json(()),
        Err(e) => api_err(&format!("delete website: {}", e)),
    }
}

async fn list_ssl(
    State(state): State<PanelState>,
) -> Json<ApiResponse<Vec<SslCertificate>>> {
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, domain, cert_path, key_path, issuer, expires_at, auto_renew, created_at FROM ssl_certificates ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SslCertificate {
                id: r.get(0)?,
                domain: r.get(1)?,
                cert_path: r.get(2)?,
                key_path: r.get(3)?,
                issuer: r.get(4)?,
                expires_at: r.get(5)?,
                auto_renew: r.get::<_, i64>(6)? != 0,
                created_at: r.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        Ok(out)
    })();
    match result {
        Ok(c) => api_json(c),
        Err(e) => api_err(&format!("list ssl: {}", e)),
    }
}

async fn issue_ssl(
    State(state): State<PanelState>,
    Json(payload): Json<AcmeRequest>,
) -> Json<ApiResponse<SslCertificate>> {
    let id = short_id();
    let now = now_iso();
    let domain = payload.domains.first().cloned().unwrap_or_default();
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        conn.execute(
            "INSERT INTO ssl_certificates (id, domain, cert_path, key_path, issuer, expires_at, auto_renew, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                id.clone(),
                domain,
                format!("{}/cert.pem", id),
                format!("{}/key.pem", id),
                "letsencrypt-staging",
                Option::<String>::None,
                1,
                now,
            ],
        )?;
        let cert_path = format!("{}/cert.pem", id);
        let key_path = format!("{}/key.pem", id);
        Ok(SslCertificate {
            id,
            domain,
            cert_path,
            key_path,
            issuer: "letsencrypt-staging".into(),
            expires_at: None,
            auto_renew: true,
            created_at: now,
        })
    })();
    match result {
        Ok(c) => api_json(c),
        Err(e) => api_err(&format!("issue ssl: {}", e)),
    }
}

async fn delete_ssl_cert(
    State(state): State<PanelState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        conn.execute("DELETE FROM ssl_certificates WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })();
    match result {
        Ok(()) => api_json(()),
        Err(e) => api_err(&format!("delete ssl: {}", e)),
    }
}

async fn renew_ssl_cert(
    State(_state): State<PanelState>,
    Path(_id): Path<String>,
) -> Json<ApiResponse<()>> {
    api_json(())
}

async fn list_workers(
    State(_state): State<PanelState>,
) -> Json<ApiResponse<Vec<WorkerStatus>>> {
    api_json(vec![])
}

async fn restart_worker(
    State(_state): State<PanelState>,
    Path(_id): Path<String>,
) -> Json<ApiResponse<()>> {
    api_json(())
}

async fn stop_worker(
    State(_state): State<PanelState>,
    Path(_id): Path<String>,
) -> Json<ApiResponse<()>> {
    api_json(())
}

async fn list_users(
    State(state): State<PanelState>,
) -> Json<ApiResponse<Vec<User>>> {
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, username, role, email, is_active, created_at FROM users ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(User {
                id: r.get(0)?,
                username: r.get(1)?,
                role: r.get(2)?,
                email: r.get(3)?,
                is_active: r.get::<_, i64>(4)? != 0,
                created_at: r.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        Ok(out)
    })();
    match result {
        Ok(u) => api_json(u),
        Err(e) => api_err(&format!("list users: {}", e)),
    }
}

async fn create_user(
    State(state): State<PanelState>,
    Json(payload): Json<CreateUser>,
) -> Json<ApiResponse<User>> {
    let id = short_id();
    let now = now_iso();
    let role = payload.role.clone().unwrap_or_else(|| "viewer".into());
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        conn.execute(
            "INSERT INTO users (id, username, password, role, email, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                payload.username,
                payload.password,
                role,
                payload.email,
                1,
                now,
            ],
        )?;
        Ok(User {
            id,
            username: payload.username,
            role,
            email: payload.email,
            is_active: true,
            created_at: now,
        })
    })();
    match result {
        Ok(u) => api_json(u),
        Err(e) => api_err(&format!("create user: {}", e)),
    }
}

async fn list_files(
    Query(query): Query<FileQuery>,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let dir = query.dir.unwrap_or_else(|| "/".into());
    api_json(vec![
        serde_json::json!({"name": "index.php", "type": "file", "size": "1.2 KB", "modified": "2026-11-24", "path": format!("{}/index.php", dir)}),
        serde_json::json!({"name": ".htaccess", "type": "file", "size": "0.8 KB", "modified": "2026-11-24", "path": format!("{}/.htaccess", dir)}),
    ])
}

async fn list_settings(
    State(state): State<PanelState>,
) -> Json<ApiResponse<Vec<Setting>>> {
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
        let rows = stmt.query_map([], |r| Ok(Setting { key: r.get(0)?, value: r.get(1)? }))?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        Ok(out)
    })();
    match result {
        Ok(s) => api_json(s),
        Err(e) => api_err(&format!("list settings: {}", e)),
    }
}

async fn update_setting(
    State(_state): State<PanelState>,
    Json(_payload): Json<UpdateSetting>,
) -> Json<ApiResponse<()>> {
    api_json(())
}

async fn login(
    State(state): State<PanelState>,
    Json(payload): Json<LoginRequest>,
) -> Json<ApiResponse<LoginResponse>> {
    let result: Result<_, rusqlite::Error> = (move || {
        let conn = state.db.inner.lock().unwrap();
        conn.query_row(
            "SELECT id, username, role, email, is_active FROM users WHERE username = ?1 AND password = ?2",
            rusqlite::params![payload.username, payload.password],
            |r| Ok(User {
                id: r.get(0)?,
                username: r.get(1)?,
                role: r.get(2)?,
                email: r.get(3)?,
                is_active: r.get::<_, i64>(4)? != 0,
                created_at: String::new(),
            }),
        )
    })();
    match result {
        Ok(user) => {
            if !user.is_active {
                return api_err("Account disabled");
            }
            let token = format!("larva_{}", short_id());
            api_json(LoginResponse { token, user })
        }
        Err(_) => api_err("Invalid credentials"),
    }
}

// ============================================================================
// SPA Fallback
// ============================================================================

async fn spa_fallback() -> Html<String> {
    Html(crate::html::panel_html())
}
