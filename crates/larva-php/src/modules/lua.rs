//! Lua module loader (LuaJIT via mlua).

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
static ROUTES: once_cell::sync::Lazy<Mutex<Vec<LuaRoute>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(Vec::new()));

#[cfg(feature = "lua")]
static HOOKS: once_cell::sync::Lazy<Mutex<Vec<LuaHook>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(Vec::new()));

#[cfg(feature = "lua")]
pub fn routes() -> Vec<LuaRoute> { ROUTES.lock().unwrap().clone() }

#[cfg(feature = "lua")]
pub fn hooks() -> Vec<LuaHook> { HOOKS.lock().unwrap().clone() }

#[cfg(feature = "lua")]
pub fn load_lua_module(path: &str) -> Result<(), String> {
    let lua = Lua::new();
    let larva = lua.create_table().map_err(|e| e.to_string())?;

    let register = lua.create_function(|lua, (method, path, func): (String, String, LuaFunction)| {
        ROUTES.lock().unwrap().push(LuaRoute {
            method: method.to_uppercase(),
            path,
            handler: Arc::new(Mutex::new(func)),
            lua: Arc::new(Mutex::new(lua.clone())),
        });
        Ok(())
    }).map_err(|e| e.to_string())?;
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

    // panel integration
    let panel_page = lua.create_function(|_lua, (_slug, _title): (String, String)| Ok(())).map_err(|e| e.to_string())?;
    larva.set("panel_page", panel_page).map_err(|e| e.to_string())?;

    // crypto
    let crypto = lua.create_table().map_err(|e| e.to_string())?;
    let b64e = lua.create_function(|_, s: String| Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, s))).map_err(|e| e.to_string())?;
    let b64d = lua.create_function(|_, s: String| {
        let b = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s).map_err(mlua::Error::external)?;
        Ok(String::from_utf8_lossy(&b).to_string())
    }).map_err(|e| e.to_string())?;
    crypto.set("b64encode", b64e).map_err(|e| e.to_string())?;
    crypto.set("b64decode", b64d).map_err(|e| e.to_string())?;
    larva.set("crypto", crypto).map_err(|e| e.to_string())?;

    // template
    let tpl = lua.create_function(|_, (tmpl, vars): (String, LuaTable)| {
        let mut out = tmpl.clone();
        for pair in vars.pairs::<String, String>() {
            if let Ok((k, v)) = pair { out = out.replace(&format!("{{{{{}}}}}", k), &v); }
        }
        Ok(out)
    }).map_err(|e| e.to_string())?;
    let template = lua.create_table().map_err(|e| e.to_string())?;
    template.set("render", tpl).map_err(|e| e.to_string())?;
    larva.set("template", template).map_err(|e| e.to_string())?;

    lua.globals().set("larva", larva).map_err(|e| e.to_string())?;
    // Support loading a directory of .lua files
    let p = std::path::Path::new(path);
    if p.is_dir() {
        // set package.path
        let dir = p.to_string_lossy();
        let _ = lua.load(format!("package.path = package.path .. ';{}/?.lua'", dir)).exec();
        let mut entries: Vec<_> = std::fs::read_dir(p).map_err(|e| e.to_string())?.filter_map(|e| e.ok()).collect();
        // sort: init.lua first, then alphabetical
        entries.sort_by_key(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            if n == "init.lua" { "0_init.lua".to_string() } else { n }
        });
        let dir_fwd = p.to_string_lossy().replace('\\', "/");
        let _ = lua.load(format!("package.path = package.path .. ';{}/?.lua'", dir_fwd)).exec();
        for e in entries {
            let fp = e.path();
            if fp.extension().and_then(|x| x.to_str()) == Some("lua") {
                let src = std::fs::read_to_string(&fp).map_err(|e| e.to_string())?;
                lua.load(src).set_name(fp.to_string_lossy().as_ref()).exec().map_err(|e| e.to_string())?;
            }
        }
    } else {
        let src = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        lua.load(src).exec().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(not(feature = "lua"))]
pub fn load_lua_module(_path: &str) -> Result<(), String> {
    Err("lua feature not enabled".into())
}
