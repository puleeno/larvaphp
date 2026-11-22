//! Embedded PHP runtime via the C shim (`larva_embed.c`).
//!
//! The shim loads `php8.dll` at runtime and runs PHP in-process, like
//! `mod_php` / FrankenPHP. This eliminates the per-request `php-cgi.exe`
//! spawn.
//!
//! Why a C shim instead of direct Rust FFI? Calling the PHP C API directly
//! from Rust crashed inside `sapi_startup()` on Windows (an MSVC/Rust ABI/CRT
//! interaction). The C shim, compiled with the same MSVC toolchain as
//! `php8.dll`, is the same approach FrankenPHP uses (`frankenphp.c`) and works
//! reliably.
//!
//! The NTS `php8.dll` is not thread-safe, so callers must serialise PHP
//! access (see `EmbeddedExecutor`).

#[cfg(feature = "embedded")]
pub mod imp {
    use std::ffi::CString;
    use std::sync::{Mutex, OnceLock};

    use crate::errors::{Error, Result};

    /// The NTS php8.dll is not thread-safe, so all embedded PHP access must
    /// be serialised through this mutex.
    static PHP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn php_lock() -> &'static Mutex<()> {
        PHP_LOCK.get_or_init(|| Mutex::new(()))
    }

    /// Initialise the embedded runtime once. Returns Ok(()) on success.
    fn ensure_init() -> Result<()> {
        use std::sync::OnceLock;
        static INIT: OnceLock<std::result::Result<(), String>> = OnceLock::new();
        let r = INIT.get_or_init(|| {
            // Resolve php.ini: prefer an explicit override, else let the shim
            // use the default lookup (PHPRC / php.ini in the working dir).
            let ini_cfg = crate::runtime::php_ini::resolve();

            let php_ini_path = ini_cfg
                .path
                .as_ref()
                .and_then(|p| CString::new(p.to_string_lossy().to_string()).ok());
            let entries = ini_cfg
                .entries
                .as_ref()
                .and_then(|e| CString::new(e.clone()).ok());

            let path_ptr = php_ini_path
                .map(|c| CString::into_raw(c) as *const std::os::raw::c_char)
                .unwrap_or(std::ptr::null());
            let entries_ptr = entries
                .map(|c| CString::into_raw(c) as *const std::os::raw::c_char)
                .unwrap_or(std::ptr::null());

            let rc = unsafe { larva_php_sys::larva_embed_init(path_ptr, entries_ptr) };
            if rc != 0 {
                Err(format!("larva_embed_init failed: {}", rc))
            } else {
                tracing::info!("embedded PHP runtime initialised");
                Ok(())
            }
        });
        match r {
            Ok(()) => Ok(()),
            Err(e) => Err(Error::Ffi(e.clone())),
        }
    }

    /// Execute a PHP script with the embedded runtime. Returns the script's
    /// stdout as bytes.
    pub fn execute_script(script_path: &str) -> Result<Vec<u8>> {
        ensure_init()?;
        let _guard = php_lock().lock().unwrap_or_else(|e| e.into_inner());

        let cpath = CString::new(script_path)
            .map_err(|_| Error::Ffi("invalid script path".to_string()))?;

        // Grow the output buffer until it fits (the shim NUL-terminates when it
        // fits and reports the true length via out_len).
        let mut cap: usize = 64 * 1024;
        loop {
            let mut buf = vec![0u8; cap];
            let mut out_len: std::os::raw::c_int = 0;
            let rc = unsafe {
                larva_php_sys::larva_embed_execute(
                    cpath.as_ptr(),
                    buf.as_mut_ptr() as *mut std::os::raw::c_char,
                    cap as std::os::raw::c_int,
                    &mut out_len,
                )
            };
            if rc != 0 {
                return Err(Error::Ffi(format!("larva_embed_execute failed: {}", rc)));
            }
            let n = out_len as usize;
            if n < cap {
                buf.truncate(n);
                return Ok(buf);
            }
            // Output was truncated because it exactly filled the buffer; grow
            // and retry.
            cap = cap.saturating_mul(2);
            if cap > 256 * 1024 * 1024 {
                buf.truncate(n.min(buf.len()));
                return Ok(buf);
            }
        }
    }

    /// Execute a PHP script with full request context ($_SERVER, $_GET,
    /// $_POST, $_COOKIE, php://input). Returns the response (status, headers,
    /// body).
    pub fn execute_request(req: &crate::runtime::executor::Request) -> Result<crate::runtime::executor::Response> {
        ensure_init()?;
        let _guard = php_lock().lock().unwrap_or_else(|e| e.into_inner());

        let script_path = CString::new(req.script_path.as_str())
            .map_err(|_| Error::Ffi("invalid script path".to_string()))?;
        let method = CString::new(req.method.as_str()).unwrap_or_default();
        let request_uri = CString::new(req.uri.as_str()).unwrap_or_default();

        let query_string = req.uri.split_once('?').map(|(_, q)| q);
        let query_cstr = query_string.and_then(|q| CString::new(q).ok());
        let query_ptr = query_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());

        // Cookie data from the Cookie header.
        let cookie = req.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("Cookie"));
        let cookie_cstr = cookie.and_then(|(_, v)| CString::new(v.as_str()).ok());
        let cookie_ptr = cookie_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());

        // Content-Type and Content-Length from headers.
        let content_type = req.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("Content-Type"));
        let content_type_cstr = content_type.and_then(|(_, v)| CString::new(v.as_str()).ok());
        let content_type_ptr = content_type_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());

        let content_length = req.body.len() as std::os::raw::c_long;
        let body_ptr = if req.body.is_empty() {
            std::ptr::null()
        } else {
            req.body.as_ptr() as *const std::os::raw::c_char
        };

        // Build $_SERVER entries from request headers (HTTP_* variables).
        let mut server_vars: Vec<larva_php_sys::LarvaEmbedServerVar> = Vec::new();
        let mut server_keys: Vec<CString> = Vec::new();
        let mut server_vals: Vec<CString> = Vec::new();
        for (k, v) in &req.headers {
            let key = if k.starts_with("HTTP_") {
                k.clone()
            } else if k.eq_ignore_ascii_case("Content-Type")
                || k.eq_ignore_ascii_case("Content-Length")
            {
                continue; // handled via SG(request_info)
            } else {
                format!("HTTP_{}", k.to_ascii_uppercase().replace('-', "_"))
            };
            if let (Ok(ck), Ok(cv)) = (CString::new(key), CString::new(v.as_str())) {
                server_vars.push(larva_php_sys::LarvaEmbedServerVar {
                    key: ck.as_ptr(),
                    value: cv.as_ptr(),
                });
                server_keys.push(ck);
                server_vals.push(cv);
            }
        }

        // Always include some standard server vars.
        let extra: Vec<(&str, String)> = vec![
            ("SERVER_NAME", req.server_name.clone()),
            ("SERVER_PORT", req.server_port.clone()),
            ("SERVER_PROTOCOL", "HTTP/1.1".to_string()),
            ("SERVER_SOFTWARE", "LarvaPHP/1.13.5 (FrankenPHP-compatible)".to_string()),
            ("GATEWAY_INTERFACE", "CGI/1.1".to_string()),
            ("SCRIPT_NAME", req.script_name.clone()),
            ("PHP_SELF", req.script_name.clone()),
            ("DOCUMENT_ROOT", req.script_path.rsplit_once(std::path::MAIN_SEPARATOR).map(|(d, _)| d.to_string()).unwrap_or_default()),
            ("REDIRECT_STATUS", "200".to_string()),
        ];
        for (k, v) in &extra {
            if let (Ok(ck), Ok(cv)) = (CString::new(k.to_string()), CString::new(v.as_str())) {
                server_vars.push(larva_php_sys::LarvaEmbedServerVar {
                    key: ck.as_ptr(),
                    value: cv.as_ptr(),
                });
                server_keys.push(ck);
                server_vals.push(cv);
            }
        }

        let larva_req = larva_php_sys::LarvaEmbedRequest {
            method: method.as_ptr(),
            request_uri: request_uri.as_ptr(),
            query_string: query_ptr,
            content_type: content_type_ptr,
            content_length,
            body: body_ptr,
            body_len: req.body.len() as std::os::raw::c_long,
            cookie_data: cookie_ptr,
            server_vars: server_vars.as_ptr(),
            server_vars_count: server_vars.len() as std::os::raw::c_int,
        };

        let mut larva_resp = larva_php_sys::LarvaEmbedResponse {
            status: 0,
            headers: std::ptr::null_mut(),
            headers_len: 0,
            body: std::ptr::null_mut(),
            body_len: 0,
        };

        let rc = unsafe {
            larva_php_sys::larva_embed_execute_request(
                script_path.as_ptr(),
                &larva_req,
                &mut larva_resp,
            )
        };
        if rc != 0 {
            return Err(Error::Ffi(format!("larva_embed_execute_request failed: {}", rc)));
        }

        // Copy headers and body out, then free the C buffers.
        let headers_str = unsafe {
            if larva_resp.headers.is_null() || larva_resp.headers_len <= 0 {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(larva_resp.headers)
                    .to_string_lossy()
                    .into_owned()
            }
        };
        let body = unsafe {
            if larva_resp.body.is_null() || larva_resp.body_len <= 0 {
                Vec::new()
            } else {
                let slice = std::slice::from_raw_parts(
                    larva_resp.body as *const u8,
                    larva_resp.body_len as usize,
                );
                slice.to_vec()
            }
        };
        let status = larva_resp.status as u16;

        unsafe { larva_php_sys::larva_embed_response_free(&mut larva_resp) };

        // Parse the header lines into (key, value) pairs.
        let mut headers = Vec::new();
        for line in headers_str.lines() {
            // Skip the status line (e.g. "HTTP/1.0 200 OK").
            if line.starts_with("HTTP/") {
                continue;
            }
            if let Some((k, v)) = line.split_once(':') {
                headers.push((k.trim().to_string(), v.trim().to_string()));
            }
        }

        Ok(crate::runtime::executor::Response {
            status,
            headers,
            body,
        })
    }

    /// Whether the embedded runtime is available.
    pub fn is_available() -> bool {
        ensure_init().is_ok()
    }

    /// Tear the runtime down.
    pub fn shutdown() {
        unsafe { larva_php_sys::larva_embed_shutdown() };
    }
}

#[cfg(feature = "embedded")]
pub use imp::*;

#[cfg(not(feature = "embedded"))]
pub mod imp {
    use crate::errors::{Error, Result};

    pub fn execute_script(_script_path: &str) -> Result<Vec<u8>> {
        Err(Error::Ffi(
            "embedded PHP support is not enabled (build with --features embedded)".to_string(),
        ))
    }

    pub fn execute_request(
        _req: &crate::runtime::executor::Request,
    ) -> Result<crate::runtime::executor::Response> {
        Err(Error::Ffi(
            "embedded PHP support is not enabled (build with --features embedded)".to_string(),
        ))
    }

    pub fn is_available() -> bool {
        false
    }

    pub fn shutdown() {}
}

#[cfg(not(feature = "embedded"))]
pub use imp::*;