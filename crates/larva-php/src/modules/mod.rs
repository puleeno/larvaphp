//! LarvaPHP v2 module system.
//!
//! Two native module forms are supported:
//! - Lua modules (embedded LuaJIT via mlua): intervene at any layer via hooks.
//! - WASM modules (WebAssembly): sandboxed extensions.
//!
//! Modules can register handlers for request, response, vhost, worker, etc.

pub mod lua;
pub mod wasm;

pub trait Module: Send + Sync {
    fn name(&self) -> &str;
    fn init(&self) -> Result<(), String> { Ok(()) }
}
