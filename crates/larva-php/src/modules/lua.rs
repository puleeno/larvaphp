//! Lua module loader (LuaJIT via mlua).
//!
//! Lua modules can register HTTP routes and intervene at layers.
//! Example lua:
//!   larva.route("GET", "/api/hello", function(req)
//!       return { status = 200, body = "Hello from Lua" }
//!   end)

#[cfg(feature = "lua")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "lua")]
use mlua::prelude::*;

#[cfg(feature = "lua")]
#[derive(Clone)]
pub struct LuaRoute {
    pub method: String,
    pub path: String,
    pub handler: Arc<Mutex<LuaFunction>>,
    pub lua: Arc<Mutex<Lua>>,
}

#[cfg(feature = "lua")]
static ROUTES: once_cell::sync::Lazy<Mutex<Vec<LuaRoute>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(Vec::new()));

#[cfg(feature = "lua")]
#[derive(Clone)]
pub enum HookKind { Request, Response }

#[cfg(feature = "lua")]
#[derive(Clone)]
pub struct LuaHook {
    pub kind: HookKind,
    pub handler: Arc<Mutex<LuaFunction>>,
    pub lua: Arc<Mutex<Lua>>,
}

#[cfg(feature = "lua")]
static HOOKS: once_cell::sync::Lazy<Mutex<Vec<LuaHook>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(Vec::new()));

#[cfg(feature = "lua")]
pub fn hooks() -> Vec<LuaHook> { HOOKS.lock().unwrap().clone() }

#[cfg(feature = "lua")]
pub fn routes() -> Vec<LuaRoute> {
    ROUTES.lock().unwrap().clone()
}

#[cfg(feature = "lua")]
pub fn load_lua_module(path: &str) -> Result<(), String> {
    let lua = Lua::new();
    let larva = lua.create_table().map_err(|e| e.to_string())?;
    let register = lua
        .create_function(|lua, (method, path, func): (String, String, LuaFunction)| {
            let route = LuaRoute {
                method: method.to_uppercase(),
                path,
                handler: Arc::new(Mutex::new(func)),
                lua: Arc::new(Mutex::new(lua.clone())),
            };
            ROUTES.lock().unwrap().push(route);
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    larva.set("route", register).map_err(|e| e.to_string())?;
    let on_request = lua.create_function(|lua, func: LuaFunction| {
        HOOKS.lock().unwrap().push(LuaHook { kind: HookKind::Request, handler: Arc::new(Mutex::new(func)), lua: Arc::new(Mutex::new(lua.clone())) });
        Ok(())
    }).map_err(|e| e.to_string())?;
    let on_response = lua.create_function(|lua, func: LuaFunction| {
        HOOKS.lock().unwrap().push(LuaHook { kind: HookKind::Response, handler: Arc::new(Mutex::new(func)), lua: Arc::new(Mutex::new(lua.clone())) });
        Ok(())
    }).map_err(|e| e.to_string())?;
    larva.set("on_request", on_request).map_err(|e| e.to_string())?;
    larva.set("on_response", on_response).map_err(|e| e.to_string())?;
    // Panel integration: modules can register panel pages/routes
    let panel_page = lua.create_function(|_lua, (slug, title): (String, String)| {
        // Store in a separate registry (placeholder)
        let _ = (slug, title);
        Ok(())
    }).map_err(|e| e.to_string())?;
    larva.set("panel_page", panel_page).map_err(|e| e.to_string())?;
    lua.globals().set("larva", larva).map_err(|e| e.to_string())?;
    lua.load(std::fs::read_to_string(path).map_err(|e| e.to_string())?)
        .exec()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(not(feature = "lua"))]
pub fn load_lua_module(_path: &str) -> Result<(), String> {
    Err("lua feature not enabled".into())
}
