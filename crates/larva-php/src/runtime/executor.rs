//! PHP executor abstraction for the worker process.
//!
//! A worker embeds a PHP runtime (FFI) and exposes it through the
//! [`PhpExecutor`] trait. The trait is transport-agnostic so the worker can be
//! tested with a [`MockExecutor`] without linking PHP.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::cgi;

/// A single PHP request: raw CGI-style fields (already UTF-8/bytes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub script_path: String,
    /// Virtual host (from the Host header), for SERVER_NAME.
    pub server_name: String,
    /// Listen port, for SERVER_PORT.
    pub server_port: String,
    /// The actual script name (e.g. /index.php), for SCRIPT_NAME/PHP_SELF.
    pub script_name: String,
}

/// The result of executing a PHP script.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Trait for a PHP executor (embedded FFI, FastCGI client, or mock).
pub trait PhpExecutor: Send + Sync {
    /// Human-readable executor name.
    fn name(&self) -> &str;
    /// Execute a request and return the response.
    fn execute(&self, req: &Request) -> Result<Response, String>;
}

/// A mock executor that echoes the request — used to test the full master↔worker
/// pipeline without a real PHP runtime.
pub struct MockExecutor;

impl PhpExecutor for MockExecutor {
    fn name(&self) -> &str {
        "mock"
    }

    fn execute(&self, req: &Request) -> Result<Response, String> {
        let body = format!(
            "MOCK {method} {uri}\nScript: {script}\nHeaders: {n}\nBody: {b} bytes",
            method = req.method,
            uri = req.uri,
            script = req.script_path,
            n = req.headers.len(),
            b = req.body.len(),
        );
        Ok(Response {
            status: 200,
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body: body.into_bytes(),
        })
    }
}

/// A CGI executor that spawns php-cgi.exe per request (reliable; loads php.ini).
///
/// This is the default: php-cgi reads the system php.ini, so extensions like
/// mysqli/pdo_mysql load correctly. It's slower than FFI (~80ms spawn) but
/// universally compatible.
pub struct CgiExecutor;

impl PhpExecutor for CgiExecutor {
    fn name(&self) -> &str {
        "cgi"
    }

    fn execute(&self, req: &Request) -> Result<Response, String> {
        let php_exe = crate::server::find_php_cgi()
            .ok_or_else(|| "php-cgi.exe not found".to_string())?;

        let mut cmd = std::process::Command::new(php_exe);
        cmd.env("REQUEST_METHOD", &req.method);
        cmd.env("REQUEST_URI", &req.uri);
        // Split query string from the URI (e.g. ?step=1&language=en_US).
        let (_path, query) = match req.uri.split_once('?') {
            Some((p, q)) => (p, q),
            None => (req.uri.as_str(), ""),
        };
        cmd.env("QUERY_STRING", query);
        cmd.env("SCRIPT_NAME", &req.script_name);
        cmd.env("PHP_SELF", &req.script_name);
        cmd.env("SERVER_NAME", &req.server_name);
        cmd.env("SERVER_PORT", &req.server_port);
        cmd.env("SCRIPT_FILENAME", &req.script_path);
        cmd.env("REDIRECT_STATUS", "200");
        cmd.env("GATEWAY_INTERFACE", "CGI/1.1");
        // Identify as FrankenPHP-compatible so WordPress' got_url_rewrite()
        // ($is_caddy) reports pretty permalinks / dynamic robots.txt as supported.
        cmd.env("SERVER_SOFTWARE", "LarvaPHP/1.13.5 (FrankenPHP-compatible)");
        // Always apply the built-in ini overrides (opcache with a persistent
        // file cache) so this per-request spawn reuses compiled bytecode
        // instead of recompiling it on every request.
        if let Some(dir) = crate::server::write_ini_overrides(&Default::default()) {
            cmd.env("PHP_INI_SCAN_DIR", &dir);
        }
        let mut has_content_length = false;
        let mut has_content_type = false;
        for (k, v) in &req.headers {
            // CGI SAPI only populates $_SERVER for variables prefixed with HTTP_.
            // Convert header names (e.g. "Cookie", "X-Real-IP") to "HTTP_COOKIE".
            let env_name = if k.eq_ignore_ascii_case("CONTENT_LENGTH")
                || k.eq_ignore_ascii_case("HTTP_CONTENT_LENGTH")
            {
                has_content_length = true;
                "CONTENT_LENGTH".to_string()
            } else if k.eq_ignore_ascii_case("CONTENT_TYPE")
                || k.eq_ignore_ascii_case("HTTP_CONTENT_TYPE")
            {
                has_content_type = true;
                "CONTENT_TYPE".to_string()
            } else if k.eq_ignore_ascii_case("Cookie") {
                "HTTP_COOKIE".to_string()
            } else if k.eq_ignore_ascii_case("HTTP_COOKIE") {
                "HTTP_COOKIE".to_string()
            } else if k.starts_with("HTTP_") {
                k.clone()
            } else {
                cgi::header_name(k)
            };
            cmd.env(&env_name, v);
        }
        // Fallback: derive from the body if the master didn't forward them.
        if !has_content_length && !req.body.is_empty() {
            cmd.env("CONTENT_LENGTH", req.body.len().to_string());
        }
        let _ = has_content_type;
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("spawn php-cgi: {}", e))?;
        if !req.body.is_empty() {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(&req.body);
            }
        }
        let output = child
            .wait_with_output()
            .map_err(|e| format!("wait php-cgi: {}", e))?;

        let text = String::from_utf8_lossy(&output.stdout).to_string();
        let (head, body) = if let Some(p) = text.find("\r\n\r\n") {
            (&text[..p], &text[p + 4..])
        } else if let Some(p) = text.find("\n\n") {
            (&text[..p], &text[p + 2..])
        } else {
            ("", text.as_str())
        };

        let mut status = 200u16;
        let mut headers = Vec::new();
        for line in head.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if k.eq_ignore_ascii_case("status") {
                    status = v
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(200);
                    continue;
                }
                headers.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
        Ok(Response {
            status,
            headers,
            body: body.as_bytes().to_vec(),
        })
    }
}

/// An embedded PHP executor (requires the `embedded` feature + PHP headers).
///
/// Fast (~8ms) but the embedded runtime must load extensions via php.ini.
pub struct FfiExecutor;

impl PhpExecutor for FfiExecutor {
    fn name(&self) -> &str {
        "ffi"
    }

    fn execute(&self, req: &Request) -> Result<Response, String> {
        #[cfg(feature = "embedded")]
        {
            crate::embed_php::execute_request(req).map_err(|e| e.to_string())
        }
        #[cfg(not(feature = "embedded"))]
        {
            Err("embedded PHP not enabled (build with --features embedded)".to_string())
        }
    }
}

/// Construct the default executor for a worker.
///
/// Defaults to CGI (reliable, loads php.ini + extensions). Set the
/// `LARVA_PHP_EXECUTOR=ffi` environment variable to use the embedded FFI
/// executor (faster, but extensions must be loaded via php.ini).
pub fn default_executor() -> Arc<dyn PhpExecutor> {
    if std::env::var("LARVA_PHP_EXECUTOR").as_deref() == Ok("ffi") {
        #[cfg(feature = "embedded")]
        {
            if crate::embed_php::is_available() {
                return Arc::new(FfiExecutor);
            }
        }
    }
    Arc::new(CgiExecutor)
}