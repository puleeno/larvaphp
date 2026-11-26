//! LarvaPHP v2 Web Control Panel.
//! Provides a CyberPanel/OpenLiteSpeed-style GUI for managing the LarvaPHP server.

pub mod api;
pub mod db;
pub mod html;
pub mod installer;
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

/// Start the panel server on the given address (no hot-reload callback).
pub async fn start(addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    start_with_reloader(addr, None).await
}

/// Start the panel server with an optional virtual-host hot-reload callback.
pub async fn start_with_reloader(
    addr: &str,
    reloader: Option<Arc<dyn VHostReloader>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let socket_addr = addr.parse()?;
    let db = PanelDb::open(&std::path::PathBuf::from("larva-panel.db"))?;
    let state = PanelState::new(db, reloader);
    state.run(socket_addr).await?;
    Ok(())
}