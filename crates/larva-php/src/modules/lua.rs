//! Lua module loader (LuaJIT via mlua).
#[cfg(feature = "lua")]
pub fn load_lua_module(_path: &str) -> Result<(), String> {
    Ok(())
}
