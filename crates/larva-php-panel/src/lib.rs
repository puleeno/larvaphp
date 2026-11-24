//! LarvaPHP v2 Web Control Panel.
//! Provides a CyberPanel/OpenLiteSpeed-style GUI for managing the LarvaPHP server.

pub mod api;
pub mod db;
pub mod html;
pub mod server;

use server::PanelState;
use db::PanelDb;

/// Start the panel server on the given address.
pub async fn start(addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let socket_addr = addr.parse()?;
    let db = PanelDb::open(&std::path::PathBuf::from("larva-panel.db"))?;
    let state = PanelState::new(db);
    state.run(socket_addr).await?;
    Ok(())
}
