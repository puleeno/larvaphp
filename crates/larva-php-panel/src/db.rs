//! SQLite database for the control panel.
//! Stores websites, SSL certificates, users, settings, and audit logs.

use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{info};

/// Wrapper around a SQLite connection for safe shared access.
#[derive(Clone)]
pub struct PanelDb {
    pub(crate) inner: Arc<Mutex<Connection>>,
}

impl PanelDb {
    /// Open (or create) the panel database at the given path.
    pub fn open(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
        }

        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        ).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

        // Enable WAL mode for better concurrent read performance.
        conn.pragma_update(None, "journal_mode", &"WAL")
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
        conn.pragma_update(None, "synchronous", &"NORMAL")
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
        conn.pragma_update(None, "foreign_keys", &"ON")
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

        Self::migrate(&conn)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e })?;

        info!("Panel database initialized at {:?}", path);

        Ok(Self {
            inner: Arc::new(Mutex::new(conn)),
        })
    }

    fn migrate(conn: &Connection) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        conn.execute_batch(
            // --- Websites ---
            r#"
            CREATE TABLE IF NOT EXISTS websites (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                domain          TEXT NOT NULL UNIQUE,
                document_root   TEXT NOT NULL,
                php_version     TEXT NOT NULL DEFAULT '8.3',
                ssl_enabled     INTEGER NOT NULL DEFAULT 0,
                ssl_cert_path   TEXT,
                ssl_key_path    TEXT,
                acme_email      TEXT,
                acme_domains    TEXT,
                worker_count    INTEGER NOT NULL DEFAULT 1,
                is_active       INTEGER NOT NULL DEFAULT 1,
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );
            "#,
        ).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

        let _ = conn.execute("ALTER TABLE websites ADD COLUMN listen_port INTEGER", []);

        conn.execute_batch(
            // --- SSL certificates ---
            r#"
            CREATE TABLE IF NOT EXISTS ssl_certificates (
                id           TEXT PRIMARY KEY,
                domain       TEXT NOT NULL,
                cert_path    TEXT NOT NULL,
                key_path     TEXT NOT NULL,
                issuer       TEXT NOT NULL DEFAULT 'letsencrypt',
                expires_at   TEXT,
                auto_renew   INTEGER NOT NULL DEFAULT 1,
                created_at   TEXT NOT NULL DEFAULT (datetime('now'))
            );
            "#,
        ).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

        conn.execute_batch(
            // --- Users ---
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id         TEXT PRIMARY KEY,
                username   TEXT NOT NULL UNIQUE,
                password   TEXT NOT NULL,
                role       TEXT NOT NULL DEFAULT 'admin',
                email      TEXT,
                is_active  INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            "#,
        ).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

        conn.execute_batch(
            // --- Settings ---
            r#"
            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        ).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

        conn.execute_batch(
            // --- Sessions (RBAC) ---
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                token      TEXT PRIMARY KEY,
                user_id    TEXT NOT NULL,
                username   TEXT NOT NULL,
                role       TEXT NOT NULL DEFAULT 'user',
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            ",
        ).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

        conn.execute_batch(
            // --- Audit log ---
            r#"
            CREATE TABLE IF NOT EXISTS audit_log (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id   TEXT,
                action    TEXT NOT NULL,
                target    TEXT,
                details   TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            "#,
        ).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

        // Seed default admin user if none exists.
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users",
            [],
            |row| row.get(0),
        ).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
        if count == 0 {
            conn.execute(
                "INSERT INTO users (id, username, password, role, email) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    "admin",
                    "$2y$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewdBPj4J/HS.iK8O", // "admin" bcrypt
                    "admin",
                    "admin@localhost",
                ],
            ).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
            info!("Seeded default admin user (username: admin, password: admin)");
        }

        Ok(())
    }

    /// Execute a closure with exclusive access to the connection.
    pub fn with_conn<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Connection) -> R,
    {
        let guard = self.inner.lock().expect("panel db lock poisoned");
        f(&guard)
    }
}

impl Default for PanelDb {
    fn default() -> Self {
        Self::open(&PathBuf::from("larva-panel.db")).expect("failed to open panel db")
    }
}
