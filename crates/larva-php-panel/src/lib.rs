//! LarvaPHP v2 Web Control Panel.
//! Provides a CyberPanel/OpenLiteSpeed-style GUI for managing the LarvaPHP server.

pub mod api;
pub mod db;
pub mod html;
pub mod installer;
pub mod modules;
pub mod server;

use std::sync::Arc;
use server::PanelState;
use db::PanelDb;

/// Callback the panel invokes after the website list changes so the live
/// server can rebuild its virtual-host routing table (hot-reload).
///
/// Defined here (in the panel crate) to avoid a circular dependency: the
/// `larva-php` crate depends on `larva-php-panel`, so the concrete
/// implementation lives in `larva-php`'s `main.rs` and is injected here.
pub trait VHostReloader: Send + Sync {
    /// Rebuild the live vhost set from the given (active) websites.
    fn reload(&self, websites: Vec<api::Website>);
}

/// Provides worker status to the panel.
pub trait WorkerProvider: Send + Sync {
    fn list_workers(&self) -> Vec<api::WorkerStatus>;
}

/// Provides database access to modules.
pub trait DbProvider: Send + Sync {
    fn query(&self, sql: &str) -> Result<Vec<std::collections::BTreeMap<String,String>>, String>;
    fn execute(&self, sql: &str) -> Result<(), String>;
}


/// Default DbProvider backed by a panel database connection.
pub struct PanelDbProvider {
    db: PanelDb,
}

impl PanelDbProvider {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self { db: PanelDb::open(&std::path::PathBuf::from("larva-panel.db"))? })
    }
}

impl DbProvider for PanelDbProvider {
    fn query(&self, sql: &str) -> Result<Vec<std::collections::BTreeMap<String,String>>, String> {
        let conn = self.db.inner.lock().unwrap();
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let cols: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let rows = stmt.query_map([], |r| {
            let mut m = std::collections::BTreeMap::new();
            for (i, c) in cols.iter().enumerate() {
                m.insert(c.clone(), r.get::<_, String>(i).unwrap_or_default());
            }
            Ok(m)
        }).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows { if let Ok(m) = row { out.push(m); } }
        Ok(out)
    }
    fn execute(&self, sql: &str) -> Result<(), String> {
        let conn = self.db.inner.lock().unwrap();
        conn.execute_batch(sql).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Start the panel server on the given address (no hot-reload callback).
pub async fn start(addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    start_with_reloader(addr, None).await
}

/// Start the panel server with an optional virtual-host hot-reload callback.
pub async fn start_with_reloader(
    addr: &str,
    reloader: Option<Arc<dyn VHostReloader>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    start_with_reloader_and_workers(addr, reloader, None).await
}

pub async fn start_with_reloader_and_workers(
    addr: &str,
    reloader: Option<Arc<dyn VHostReloader>>,
    worker_provider: Option<Arc<dyn WorkerProvider>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    start_with_reloader_workers_db(addr, reloader, worker_provider, None).await
}

pub async fn start_with_reloader_workers_db(
    addr: &str,
    reloader: Option<Arc<dyn VHostReloader>>,
    worker_provider: Option<Arc<dyn WorkerProvider>>,
    db_provider: Option<Arc<dyn DbProvider>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let socket_addr = addr.parse()?;
    let db = PanelDb::open(&std::path::PathBuf::from("larva-panel.db"))?;
    let state = PanelState::new(db, reloader, worker_provider, db_provider);
    state.run(socket_addr).await?;
    Ok(())
}