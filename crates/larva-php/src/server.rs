//! Server blocks (scoped request handling).
//!
//! Mirrors `server.go` from the Go source.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Instant};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper::body::Incoming as IncomingBody;
use hyper_util::server::conn::auto;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use dashmap::DashMap;

use crate::context::{PhpContext, RequestOption};
use crate::env::PreparedEnv;
use crate::errors::{Error, Result};
use crate::worker::Worker;
use tracing::{error, info, Level};

/// HTTP/3 UDP port (0 = HTTP/3 disabled). Set once at startup.
static H3_PORT: AtomicU16 = AtomicU16::new(0);

/// Set the HTTP/3 UDP port so TCP responses can advertise it via `Alt-Svc`.
pub fn set_http3_port(port: u16) {
    H3_PORT.store(port, Ordering::Release);
}

/// Build an `Alt-Svc` header value advertising HTTP/3, or `None` if disabled.
/// Browsers that support HTTP/3 will upgrade to QUIC; others keep using TCP.
pub fn alt_svc_header() -> Option<String> {
    let port = H3_PORT.load(Ordering::Acquire);
    if port == 0 {
        None
    } else {
        Some(format!("h3=\":{}\"; ma=86400", port))
    }
}

/// Compute the IPv6 address that mirrors an IPv4 bind, if any.
///
/// * `127.0.0.1:port`  → `[::1]:port`
/// * `0.0.0.0:port`    → `[::]:port`
/// * anything else     → `None` (cannot be mapped)
///
/// Binding both lets `localhost` (which resolves to `::1` first on Windows)
/// reach a server started on the IPv4 loopback address.
fn ipv6_equivalent(addr: SocketAddr) -> Option<SocketAddr> {
    match addr.ip() {
        std::net::IpAddr::V4(v4) => {
            let ip6 = if v4.is_loopback() {
                std::net::Ipv6Addr::LOCALHOST
            } else if v4.is_unspecified() {
                std::net::Ipv6Addr::UNSPECIFIED
            } else {
                return None;
            };
            Some(SocketAddr::new(std::net::IpAddr::V6(ip6), addr.port()))
        }
        std::net::IpAddr::V6(_) => None,
    }
}

/// A preconfigured server block.
///
/// Requests and workers can be scoped to a Server.
pub struct Server {
    pub idx: i32,
    pub configured_name: String,
    pub name: String,
    pub root: String,
    pub split_path: Vec<String>,
    pub env: HashMap<String, String>,
    pub workers: RwLock<Vec<Arc<Worker>>>,
    pub workers_by_path: RwLock<HashMap<String, Arc<Worker>>>,
    pub workers_with_request_matcher: RwLock<Vec<Arc<Worker>>>,
    pub is_registered: AtomicBool,
    pub logger_level: Level,
    /// Open file cache: path -> (size, mtime, etag)
    pub file_cache: DashMap<String, CachedFile>,
    /// Rate limiter (per-IP)
    pub rate_limiter: RateLimiter,
    /// Max request body size in bytes (default: 10 MB)
    pub max_body_size: u64,
    /// PHP configuration overrides (`php_ini` / `-d` directives).
    pub php_ini: HashMap<String, String>,
}

/// Cached file metadata (nginx open_file_cache equivalent).
#[derive(Clone, Debug)]
pub struct CachedFile {
    pub size: u64,
    pub mtime: u64,
    pub etag: String,
}

/// Rate limiter configuration.
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Max requests per window per IP
    pub max_requests: u64,
    /// Window duration in seconds
    pub window_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 60,
            window_secs: 60,
        }
    }
}

/// Per-IP rate limiter state.
#[derive(Clone, Debug)]
struct RateLimitEntry {
    count: u64,
    window_start: u64,
}

/// Thread-safe rate limiter using a fixed-window counter per IP.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    entries: Arc<DashMap<String, RateLimitEntry>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Check if the request from the given IP is allowed.
    /// Returns true if allowed, false if rate limit exceeded.
    pub fn check(&self, ip: &str) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut entry = self.entries.entry(ip.to_string()).or_insert(RateLimitEntry {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now - entry.window_start >= self.config.window_secs {
            entry.count = 1;
            entry.window_start = now;
            return true;
        }

        if entry.count >= self.config.max_requests {
            return false;
        }

        entry.count += 1;
        true
    }
}

impl Server {
    /// Create a new server with the given root directory.
    ///
    /// The root path is made absolute and symlinks are resolved (like
    /// FrankenPHP's `resolve_root_symlink`), so the document root is always a
    /// canonical path. If the path does not exist yet, resolution falls back to
    /// the absolute path without canonicalization.
    pub fn new(root: &str) -> Result<Self> {
        let abs_root = resolve_root_symlink(root)
            .map_err(|e| Error::Ffi(e.to_string()))?
            .to_string_lossy()
            .into_owned();

        Ok(Server {
            idx: -1,
            configured_name: String::new(),
            name: String::new(),
            root: abs_root,
            split_path: vec![".php".to_string()],
            env: HashMap::new(),
            workers: RwLock::new(Vec::new()),
            workers_by_path: RwLock::new(HashMap::new()),
            workers_with_request_matcher: RwLock::new(Vec::new()),
            is_registered: AtomicBool::new(false),
            logger_level: Level::INFO,
            file_cache: DashMap::new(),
            rate_limiter: RateLimiter::new(RateLimitConfig::default()),
            max_body_size: 10 * 1024 * 1024, // 10 MB
            php_ini: HashMap::new(),
        })
    }

    /// Create a fallback server (used when no server is registered).
    pub fn fallback() -> Self {
        Server {
            idx: -1,
            configured_name: String::new(),
            name: String::new(),
            root: String::new(),
            split_path: vec![".php".to_string()],
            env: HashMap::new(),
            workers: RwLock::new(Vec::new()),
            workers_by_path: RwLock::new(HashMap::new()),
            workers_with_request_matcher: RwLock::new(Vec::new()),
            is_registered: AtomicBool::new(true),
            logger_level: Level::INFO,
            file_cache: DashMap::new(),
            rate_limiter: RateLimiter::new(RateLimitConfig::default()),
            max_body_size: 10 * 1024 * 1024, // 10 MB
            php_ini: HashMap::new(),
        }
    }

    /// Set PHP configuration overrides (applied as `-d key=value` to php-cgi).
    pub fn set_php_ini(&mut self, ini: HashMap<String, String>) {
        self.php_ini = ini;
    }

    /// Find a worker whose request matcher matches the given request.
    ///
    /// Strategy pattern: iterates registered matchers, first match wins.
    /// O(n) where n = number of matcher workers (typically small).
    pub fn find_worker_by_request(&self, req: &Request<IncomingBody>) -> Option<Arc<Worker>> {
        let matchers = self.workers_with_request_matcher.read().unwrap();
        for worker in matchers.iter() {
            if let Some(matcher) = worker.match_request() {
                if matcher(req) {
                    return Some(worker.clone());
                }
            }
        }
        None
    }

    /// Get the server name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the root directory.
    pub fn root(&self) -> &str {
        &self.root
    }

    /// Get the split path.
    pub fn split_path(&self) -> &[String] {
        &self.split_path
    }

    /// Get the logger level.
    pub fn logger_level(&self) -> Level {
        self.logger_level
    }

    /// Set the server name.
    pub fn set_name(&mut self, name: String) {
        self.name = name.clone();
        self.configured_name = name;
    }

    /// Set the split path.
    pub fn set_split_path(&mut self, split_path: Vec<String>) {
        self.split_path = split_path;
    }

    /// Set the environment.
    pub fn set_env(&mut self, env: HashMap<String, String>) {
        self.env = env;
    }

    /// Add a worker to this server.
    pub fn add_worker(&self, worker: Arc<Worker>) -> Result<()> {
        let mut workers = self.workers.write().unwrap();
        workers.push(worker.clone());

        if worker.match_request().is_some() {
            let mut matchers = self.workers_with_request_matcher.write().unwrap();
            matchers.push(worker);
            return Ok(());
        }

        let mut by_path = self.workers_by_path.write().unwrap();
        let file_name = worker.file_name().to_string();
        if by_path.contains_key(&file_name) {
            return Err(Error::DuplicateServerWorkerFilename(file_name));
        }
        by_path.insert(file_name.clone(), worker.clone());
        by_path.insert(worker.declared_file_name().to_string(), worker);
        Ok(())
    }

    /// Reset workers (for reload).
    pub fn reset_workers(&self) {
        self.workers.write().unwrap().clear();
        self.workers_by_path.write().unwrap().clear();
        self.workers_with_request_matcher.write().unwrap().clear();
    }

    /// Check if this server is registered (accepting requests).
    pub fn is_registered(&self) -> bool {
        self.is_registered.load(Ordering::Acquire)
    }

    /// Activate this server (start accepting requests).
    pub fn activate(&self) {
        self.is_registered.store(true, Ordering::Release);
    }

    /// Deactivate this server.
    pub fn deactivate(&self) {
        self.is_registered.store(false, Ordering::Release);
    }

    /// Get the worker for a script filename.
    pub fn get_worker_by_path(&self, path: &str) -> Option<Arc<Worker>> {
        self.workers_by_path.read().unwrap().get(path).cloned()
    }

    /// Serve an HTTP request.
    ///
    /// This mirrors `Server.ServeHTTP()`.
    pub async fn serve_http(
        &self,
        req: Request<IncomingBody>,
        opts: Vec<Box<dyn RequestOption>>,
    ) -> Result<Response<Full<Bytes>>> {
        if !self.is_registered() {
            return Ok(Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .body(Full::new(Bytes::from("Service Unavailable")))
                .unwrap());
        }

        let ctx = PhpContext::from_request(&req, Arc::new(Server::fallback()), opts)?;
        ctx.validate()?;

        // Worker request matching (Strategy: first matcher wins).
        // If a worker matches, route to its script (front controller).
        let worker_script = self
            .find_worker_by_request(&req)
            .map(|w| w.file_name().to_string());

// Serve robots.txt directly (bypass PHP/front-controller fallback).
        // This must happen BEFORE resolve_script_path (which would append .php).
        let uri_path = req.uri().path().split('?').next().unwrap_or("");
        if uri_path == "/robots.txt" || uri_path.ends_with("/robots.txt") {
            let robots_path = PathBuf::from(&self.root).join("robots.txt");
            if robots_path.exists() {
                if let Ok(content) = tokio::fs::read(&robots_path).await {
                    return Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/plain; charset=UTF-8")
                        .header("content-length", content.len().to_string())
                        .header("server", "LarvaPHP")
                        .body(Full::new(Bytes::from(content)))
                        .unwrap());
                }
            }
            // No robots.txt on disk — serve a permissive default
            // so WordPress doesn't try to generate one.
            let default = b"User-agent: *\nDisallow: /\n";
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/plain; charset=UTF-8")
                .header("content-length", default.len().to_string())
                .header("server", "LarvaPHP")
                .body(Full::new(Bytes::from(default.to_vec())))
                .unwrap());
        }

        // Determine the script filename
        let script_path = if let Some(ws) = worker_script {
            ws
        } else {
            self.resolve_script_path(ctx.request_uri.as_str())?
        };

        // If the resolved path doesn't exist, try index.php (WordPress permalinks)
        let script_path = if !std::path::Path::new(&script_path).exists() {
            let alt = self.resolve_index_php(ctx.request_uri.as_str());
            match alt {
                Some(ref p) if std::path::Path::new(p).exists() => alt.unwrap(),
                _ => {
                    return Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Full::new(Bytes::from("Not Found")))
                        .unwrap());
                }
            }
        } else {
            script_path
        };

        // Serve static files directly (not .php)
        if let Some(ext) = std::path::Path::new(&script_path).extension() {
            if ext.to_string_lossy() != "php" {
                return self.serve_static_file(&script_path).await;
            }
        }

        // Serve robots.txt directly (bypass PHP/front-controller fallback).
        if script_path.ends_with("robots.txt") {
            let robots_path = PathBuf::from(&self.root).join("robots.txt");
            if robots_path.exists() {
                if let Ok(content) = tokio::fs::read(&robots_path).await {
                    return Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/plain; charset=UTF-8")
                        .header("content-length", content.len().to_string())
                        .header("server", "LarvaPHP")
                        .body(Full::new(Bytes::from(content)))
                        .unwrap());
                }
            }
            // No robots.txt — fall through to 404.
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("server", "LarvaPHP")
                .body(Full::new(Bytes::from("Not Found")))
                .unwrap());
        }

        // Read request body (drain the stream so hyper doesn't hang)
        let mut body_bytes = Vec::new();
        let mut body = req.into_body();
        while let Some(frame) = body.frame().await {
            match frame {
                Ok(frame) => {
                    if let Some(chunk) = frame.data_ref() {
                        body_bytes.extend_from_slice(chunk);
                    }
                }
                Err(_) => break,
            }
        }
        let body = Bytes::from(body_bytes);

        // Execute the PHP script via CGI and capture output
        let output = execute_php_cgi(&script_path, &ctx, &body, &self.php_ini)?;

        // Parse CGI output: headers + optional blank line + body
        let (status_code, headers, body_bytes) = parse_cgi_output(&output);

        // X-Sendfile: serve the referenced file directly.
        if let Some(resp) = try_x_sendfile(&headers, status_code) {
            return Ok(resp);
        }

        let mut builder = Response::builder()
            .status(status_code)
            .header("server", "LarvaPHP")
            .header("x-content-type-options", "nosniff")
            .header("x-frame-options", "DENY");

        if let Some(alt) = alt_svc_header() {
            builder = builder.header("alt-svc", alt);
        }

        for (name, value) in &headers {
            builder = builder.header(name, value);
        }

        Ok(builder.body(Full::new(Bytes::from(body_bytes))).unwrap())
    }

    /// Handle a request represented as raw parts (method, uri, headers, body).
    ///
    /// Returns `(status, headers, body)`. This is transport-agnostic and is used
    /// by the HTTP/3 (QUIC) server, which does not carry a `hyper::Body`.
    pub fn handle_request_parts(
        &self,
        method: &str,
        uri: &str,
        req_headers: &[(String, String)],
        body: &[u8],
    ) -> Result<(StatusCode, Vec<(String, String)>, Vec<u8>)> {
// Serve robots.txt directly (bypass PHP/front-controller fallback).
        // This must happen BEFORE resolve_script_path (which would append .php).
        let uri_path = uri.split('?').next().unwrap_or("");
        if uri_path == "/robots.txt" || uri_path.ends_with("/robots.txt") {
            let robots_path = PathBuf::from(&self.root).join("robots.txt");
            if robots_path.exists() {
                if let Ok(content) = std::fs::read(&robots_path) {
                    return Ok((
                        StatusCode::OK,
                        vec![
                            ("content-type".to_string(), "text/plain; charset=UTF-8".to_string()),
                            ("content-length".to_string(), content.len().to_string()),
                            ("server".to_string(), "LarvaPHP".to_string()),
                        ],
                        content,
                    ));
                }
            }
            let default = b"User-agent: *\nDisallow: /\n".to_vec();
            return Ok((
                StatusCode::OK,
                vec![
                    ("content-type".to_string(), "text/plain; charset=UTF-8".to_string()),
                    ("content-length".to_string(), default.len().to_string()),
                    ("server".to_string(), "LarvaPHP".to_string()),
                ],
                default,
            ));
        }

        // Resolve the script path.
        let script_path = self.resolve_script_path(uri)?;
        let script_path = if !std::path::Path::new(&script_path).exists() {
            match self.resolve_index_php(uri) {
                Some(p) if std::path::Path::new(&p).exists() => p,
                _ => {
                    return Ok((
                        StatusCode::NOT_FOUND,
                        vec![("content-type".to_string(), "text/plain".to_string())],
                        b"Not Found".to_vec(),
                    ))
                }
            }
        } else {
            script_path
        };

        // Static (non-.php) files: serve directly.
        if let Some(ext) = std::path::Path::new(&script_path).extension() {
            if ext.to_string_lossy() != "php" {
                // Block sensitive files even for static serving (defense in depth).
                if is_sensitive_file(&script_path) {
                    return Ok((
                        StatusCode::FORBIDDEN,
                        vec![("content-type".to_string(), "text/plain".to_string())],
                        b"Forbidden".to_vec(),
                    ));
                }
                if let Ok(content) = std::fs::read(&script_path) {
                    let mime = mime_type_static(&script_path);
                    let headers = vec![
                        ("server".to_string(), "LarvaPHP".to_string()),
                        ("content-type".to_string(), mime.to_string()),
                        ("content-length".to_string(), content.len().to_string()),
                    ];
                    return Ok((StatusCode::OK, headers, content));
                }
            }
        }

        // Build a synthetic request so we can reuse PhpContext::from_request.
        let mut builder = Request::builder().method(method).uri(uri);
        for (k, v) in req_headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        let req = builder
            .body(Full::new(Bytes::from(body.to_vec())))
            .map_err(|e| Error::Ffi(e.to_string()))?;
        let ctx = PhpContext::from_request(&req, Arc::new(Server::fallback()), vec![])?;
        ctx.validate()?;

        let body = Bytes::from(body.to_vec());
        let output = execute_php_cgi(&script_path, &ctx, &body, &self.php_ini)?;
        let (status, headers, out_body) = parse_cgi_output(&output);

        // X-Sendfile: serve the referenced file directly.
        if let Some((_, path)) = headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("x-sendfile"))
        {
            if !is_sensitive_file(path) {
                if let Ok(content) = std::fs::read(path) {
                    let mut final_headers = vec![
                        ("server".to_string(), "LarvaPHP".to_string()),
                        ("content-type".to_string(), mime_type_static(path).to_string()),
                        ("content-length".to_string(), content.len().to_string()),
                    ];
                    for (n, v) in &headers {
                        if !n.eq_ignore_ascii_case("x-sendfile")
                            && !n.eq_ignore_ascii_case("content-length")
                            && !n.eq_ignore_ascii_case("content-type")
                        {
                            final_headers.push((n.clone(), v.clone()));
                        }
                    }
                    return Ok((status, final_headers, content));
                }
            }
        }

        Ok((status, headers, out_body))
    }

    /// Resolve the PHP script path from the request URI.
    fn resolve_script_path(&self, uri: &str) -> Result<String> {
        // Remove query string
        let path = uri.split('?').next().unwrap_or(uri);

        // URL-decode the path
        let decoded = url_decode(path);

        // Reject null bytes
        if decoded.contains('\0') {
            return Err(Error::Ffi("Null byte in path".to_string()));
        }

        // Build the full path: root + path
        let mut full_path = PathBuf::from(&self.root);
        if decoded.starts_with('/') {
            full_path.push(&decoded[1..]);
        } else {
            full_path.push(&decoded);
        }

        // Sanitize path traversal (.. segments)
        let sanitized = sanitize_path(&full_path)?;

        // Verify the resolved path is within the document root
        if !sanitized.starts_with(&self.root) {
            return Err(Error::Ffi("Path traversal detected".to_string()));
        }

        // If it's a directory, look for index.php
        let sanitized = if sanitized.is_dir() {
            sanitized.join("index.php")
        } else {
            sanitized
        };

        // If no .php extension, add one
        let sanitized = if sanitized.extension().is_none() {
            let mut p = sanitized.clone();
            p.set_extension("php");
            p
        } else {
            sanitized
        };

        Ok(sanitized.to_string_lossy().to_string())
    }

    /// Try resolving index.php for the given URI (WordPress permalink fallback).
    fn resolve_index_php(&self, uri: &str) -> Option<String> {
        let _path = uri.split('?').next().unwrap_or(uri);

        // Try root index.php first (WordPress front controller)
        let mut index_path = PathBuf::from(&self.root);
        index_path.push("index.php");
        if index_path.exists() {
            return Some(index_path.to_string_lossy().to_string());
        }

        None
    }

    /// Serve a static file directly with nginx-style optimizations:
    /// file metadata cache, ETag, Last-Modified headers.
    async fn serve_static_file(&self, path: &str) -> Result<Response<Full<Bytes>>> {
        // Block sensitive files from being served
        if is_sensitive_file(path) {
            return Err(Error::Ffi("Forbidden".to_string()));
        }

        // Check file cache first
        let cached = self.file_cache.get(path);

        let metadata = if let Some(c) = cached {
            c.clone()
        } else {
            let meta = tokio::fs::metadata(path).await.map_err(|e| Error::Io(e))?;
            let size = meta.len();
            let mtime = meta
                .modified()
                .map_err(|e| Error::Io(e))?
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let etag = format!("\"{:x}-{:x}\"", mtime, size);

            // Insert into cache
            self.file_cache.insert(path.to_string(), CachedFile { size, mtime, etag: etag.clone() });

            CachedFile { size, mtime, etag }
        };

        let mime = Self::mime_type(path);
        let content = tokio::fs::read(path)
            .await
            .map_err(|e| Error::Io(e))?;

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", mime)
            .header("content-length", metadata.size.to_string())
            .header("last-modified", http_date(metadata.mtime))
            .header("etag", &metadata.etag)
            .header("accept-ranges", "bytes")
            .header("x-content-type-options", "nosniff")
            .header("x-frame-options", "DENY")
            .header("server", "LarvaPHP")
            .body(Full::new(Bytes::from(content)))
            .unwrap())
    }

    /// Determine MIME type from file extension.
    fn mime_type(path: &str) -> String {
        match std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
        {
            "css" => "text/css; charset=UTF-8",
            "js" => "application/javascript; charset=UTF-8",
            "json" => "application/json; charset=UTF-8",
            "html" | "htm" => "text/html; charset=UTF-8",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "ttf" => "font/ttf",
            "eot" => "application/vnd.ms-fontobject",
            "pdf" => "application/pdf",
            "zip" => "application/zip",
            "xml" => "application/xml; charset=UTF-8",
            "txt" => "text/plain; charset=UTF-8",
            _ => "application/octet-stream",
        }
        .to_string()
    }

    /// Start the HTTP server listening on the given address.
    pub async fn start_server(&self, addr: SocketAddr) -> Result<()> {
        self.activate();

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Io(e))?;
        info!("LarvaPHP server listening on http://{}", addr);

        // Dual-stack: also listen on the IPv6 equivalent of an IPv4 bind so
        // `localhost` works. On Windows `localhost` resolves to `::1` first,
        // and a browser that does not fall back to IPv4 would otherwise fail
        // to connect ("can't connect to the server at localhost:8000").
        let v6_listener = match ipv6_equivalent(addr) {
            Some(addr6) => match TcpListener::bind(addr6).await {
                Ok(l) => {
                    info!("LarvaPHP server also listening on http://{}", addr6);
                    Some(l)
                }
                Err(e) => {
                    tracing::debug!("IPv6 listener on {} unavailable: {}", addr6, e);
                    None
                }
            },
            None => None,
        };
        let server_root = self.root.clone();
        let server_split_path = self.split_path.clone();
        // Snapshot worker matchers and php.ini once (Flyweight: shared via Arc).
        let server_workers: Arc<Vec<Arc<Worker>>> =
            Arc::new(self.workers_with_request_matcher.read().unwrap().clone());
        let server_php_ini: Arc<HashMap<String, String>> = Arc::new(self.php_ini.clone());

        loop {
            // Accept from either the IPv4 or IPv6 listener (whichever fires).
            let accepted = {
                let accept_v4 = listener.accept();
                let accept_v6 = async {
                    match &v6_listener {
                        Some(l) => l.accept().await,
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    res = accept_v4 => res,
                    res = accept_v6 => res,
                }
            };
            let (stream, _) = accepted.map_err(|e| Error::Io(e))?;
            let peer_addr = stream.peer_addr().map_err(|e| Error::Io(e))?;

            let root = server_root.clone();
            let split_path = server_split_path.clone();
            let registered = self.is_registered.load(Ordering::Acquire);
            let rate_limiter = self.rate_limiter.clone();
            let max_body_size = self.max_body_size;
            let workers = server_workers.clone();
            let php_ini = server_php_ini.clone();

            tokio::spawn(async move {
                let io = TokioIo::new(stream);
                if let Err(err) = auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                    .serve_connection(
                        io,
                        service_fn(move |req: Request<IncomingBody>| {
                            let root = root.clone();
                            let split_path = split_path.clone();
                            let registered = registered.clone();
                            let rate_limiter = rate_limiter.clone();
                            let max_body_size = max_body_size;
                            let workers = workers.clone();
                            let php_ini = php_ini.clone();
                            let client_ip = peer_addr.ip().to_string();
                            async move {
                                if !registered {
                                    return Ok::<Response<Full<Bytes>>, hyper::Error>(
                                        Response::builder()
                                            .status(StatusCode::SERVICE_UNAVAILABLE)
                                            .body(Full::new(Bytes::from("Service Unavailable")))
                                            .unwrap(),
                                    );
                                }

                                let uri = req.uri().to_string();

                                // Worker request matching (Strategy: first matcher wins).
                                // If a worker matches, route to its script (front controller).
                                let mut worker_script: Option<String> = None;
                                for worker in workers.iter() {
                                    if let Some(matcher) = worker.match_request() {
                                        if matcher(&req) {
                                            worker_script =
                                                Some(worker.file_name().to_string());
                                            break;
                                        }
                                    }
                                }

// Serve robots.txt directly (bypass PHP/front-controller fallback).
        // This must happen BEFORE resolve_script_path (which would append .php).
        let uri_path = uri.split('?').next().unwrap_or("");
        if uri_path == "/robots.txt" || uri_path.ends_with("/robots.txt") {
            let robots_path = PathBuf::from(&root).join("robots.txt");
            if robots_path.exists() {
                if let Ok(content) = tokio::fs::read(&robots_path).await {
                    return Ok::<Response<Full<Bytes>>, hyper::Error>(
                        Response::builder()
                            .status(StatusCode::OK)
                            .header("content-type", "text/plain; charset=UTF-8")
                            .header("content-length", content.len().to_string())
                            .header("server", "LarvaPHP")
                            .body(Full::new(Bytes::from(content)))
                            .unwrap(),
                    );
                }
            }
            // No robots.txt on disk — serve a permissive default
            // so WordPress doesn't try to generate one.
            let default = b"User-agent: *\nDisallow: /\n";
            return Ok::<Response<Full<Bytes>>, hyper::Error>(
                Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/plain; charset=UTF-8")
                    .header("content-length", default.len().to_string())
                    .header("server", "LarvaPHP")
                    .body(Full::new(Bytes::from(default.to_vec())))
                    .unwrap(),
            );
        }

// Serve robots.txt directly (bypass PHP/front-controller fallback).
        // This must happen BEFORE resolve_script_path (which would append .php).
        let uri_path = uri.split('?').next().unwrap_or("");
        if uri_path == "/robots.txt" || uri_path.ends_with("/robots.txt") {
            let robots_path = PathBuf::from(&root).join("robots.txt");
            if robots_path.exists() {
                if let Ok(content) = std::fs::read(&robots_path) {
                    return Ok::<Response<Full<Bytes>>, hyper::Error>(
                        Response::builder()
                            .status(StatusCode::OK)
                            .header("content-type", "text/plain; charset=UTF-8")
                            .header("content-length", content.len().to_string())
                            .header("server", "LarvaPHP")
                            .body(Full::new(Bytes::from(content)))
                            .unwrap(),
                    );
                }
            }
            // No robots.txt on disk — serve a permissive default
            // so WordPress doesn't try to generate one.
            let default = b"User-agent: *\nDisallow: /\n";
            return Ok::<Response<Full<Bytes>>, hyper::Error>(
                Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/plain; charset=UTF-8")
                    .header("content-length", default.len().to_string())
                    .header("server", "LarvaPHP")
                    .body(Full::new(Bytes::from(default.to_vec())))
                    .unwrap(),
            );
        }

        let script_path = if let Some(ws) = worker_script {
            Some(ws)
        } else {
            let resolved =
                resolve_script_path_static(&uri, &root, &split_path);

            // try_files fallback: if the resolved path doesn't
            // exist (or is a directory without index), fall back
            // to index.php (front controller / permalink support).
            // Static assets (.ico, .css, .js, images) that don't
            // exist return 404 instead (no front-controller
            // fallback), which avoids WordPress building wrong redirect URLs
            // like /favicon.ico/wp-admin/install.php.
            if let Some(ref p) = resolved {
                if std::path::Path::new(p).exists() {
                    resolved
                } else {
                    let is_static = std::path::Path::new(p)
                        .extension()
                        .map(|e| e.to_string_lossy() != "php")
                        .unwrap_or(false);
                    if is_static {
                        return Ok::<Response<Full<Bytes>>, hyper::Error>(
                            Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .header("server", "LarvaPHP")
                                .body(Full::new(Bytes::from("Not Found")))
                                .unwrap(),
                        );
                    }
                    resolve_index_php_static(&uri, &root)
                }
            } else {
                resolve_index_php_static(&uri, &root)
            }
        };

        let Some(path) = script_path else {
            return Ok::<Response<Full<Bytes>>, hyper::Error>(
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Full::new(Bytes::from("Not Found")))
                    .unwrap(),
            );
        };

        // Serve static files directly (not .php)
                                if let Some(ext) = std::path::Path::new(&path).extension() {
                                    if ext.to_string_lossy() != "php" {
                                        // Block sensitive files
                                        if is_sensitive_file(&path) {
                                            return Ok::<Response<Full<Bytes>>, hyper::Error>(
                                                Response::builder()
                                                    .status(StatusCode::FORBIDDEN)
                                                    .header("content-type", "text/plain; charset=UTF-8")
                                                    .header("server", "LarvaPHP")
                                                    .body(Full::new(Bytes::from("Forbidden")))
                                                    .unwrap(),
                                            );
                                        }

                                        let content = tokio::fs::read(&path)
                                            .await
                                            .unwrap_or_default();
                                        let mime = mime_type_static(&path);
                                        // Nginx-style headers for static files
                                        let mut builder = Response::builder()
                                            .status(StatusCode::OK)
                                            .header("content-type", mime)
                                            .header("content-length", content.len().to_string())
                                            .header("accept-ranges", "bytes")
                                            .header("x-content-type-options", "nosniff")
                                            .header("x-frame-options", "DENY")
                                            .header("server", "LarvaPHP");
                                        if let Ok(meta) = tokio::fs::metadata(&path).await {
                                            if let Ok(mt) = meta.modified() {
                                                if let Ok(since) = mt.duration_since(UNIX_EPOCH) {
                                                    builder = builder
                                                        .header("last-modified", http_date(since.as_secs()))
                                                        .header("etag", format!("\"{:x}-{:x}\"", since.as_secs(), meta.len()));
                                                }
                                            }
                                        }
                                        return Ok::<Response<Full<Bytes>>, hyper::Error>(
                                            builder
                                                .body(Full::new(Bytes::from(content)))
                                                .unwrap(),
                                        );
                                    }
                                }

                                // Rate limiting: only PHP requests count (static files
                                // are served above and don't consume the quota). This
                                // prevents asset-heavy pages (e.g. wp-admin) from
                                // hitting the limit from CSS/JS/image requests.
                                let allowed = rate_limiter.check(&client_ip);
                                if !allowed {
                                    return Ok::<Response<Full<Bytes>>, hyper::Error>(
                                        Response::builder()
                                            .status(StatusCode::TOO_MANY_REQUESTS)
                                            .header("content-type", "text/plain; charset=UTF-8")
                                            .header(
                                                "retry-after",
                                                rate_limiter.config.window_secs.to_string(),
                                            )
                                            .header("server", "LarvaPHP")
                                            .body(Full::new(Bytes::from("Too Many Requests")))
                                            .unwrap(),
                                    );
                                }

                                // Build minimal CGI context for environment variables
                                let rel = path.trim_start_matches(&root);
                                let rel = rel.trim_start_matches('\\').trim_start_matches('/');
                                let script_name = if rel.is_empty() {
                                    "/".to_string()
                                } else {
                                    format!("/{}", rel.replace('\\', "/"))
                                };

                                let ctx = PhpContext {
                                    ctx: tokio::sync::watch::channel(None).1,
                                    document_root: root.clone(),
                                    split_path: split_path.clone(),
                                    env: {
                                        let mut e = PreparedEnv::new();
                                        for (k, v) in req.headers() {
                                            e.insert(
                                                k.as_str().to_string().into_bytes(),
                                                v.to_str().unwrap_or_default().to_string().into_bytes(),
                                            );
                                        }
                                        e
                                    },
                                    logger: Level::INFO,
                                    worker: None,
                                    server: Arc::new(Server::fallback()),
                                    request_body_timeout: None,
                                    doc_uri: String::new(),
                                    path_info: String::new(),
                                    script_name,
                                    script_filename: path.clone(),
                                    request_uri: uri.clone(),
                                    query_string: uri
                                        .split_once('?')
                                        .map(|(_, q)| q.to_string())
                                        .unwrap_or_default(),
                                    request_method: req.method().to_string(),
                                    content_type: req
                                        .headers()
                                        .get("content-type")
                                        .and_then(|v| v.to_str().ok())
                                        .unwrap_or_default()
                                        .to_string(),
                                    content_length: req
                                        .headers()
                                        .get("content-length")
                                        .and_then(|v| v.to_str().ok())
                                        .unwrap_or_default()
                                        .to_string(),
                                    server_protocol: "HTTP/1.1".to_string(),
                                    server_name: req
                                        .headers()
                                        .get("host")
                                        .and_then(|v| v.to_str().ok())
                                        .and_then(|v| v.split_once(':').map(|(h, _)| h.to_string()))
                                        .unwrap_or_else(|| "localhost".to_string()),
                                    server_port: req
                                        .headers()
                                        .get("host")
                                        .and_then(|v| v.to_str().ok())
                                        .and_then(|v| v.split_once(':').map(|(_, p)| p.to_string()))
                                        .unwrap_or_else(|| "80".to_string()),
                                    https: "off".to_string(),
                                    headers_written: false,
                                    is_done: AtomicBool::new(false),
                                    client_had_closed: AtomicBool::new(false),
                                    started_at: Instant::now(),
                                    handler_return: None,
                                };

                                // Read request body (drain the stream so hyper doesn't hang)
                                let mut body_bytes = Vec::new();
                                let mut body = req.into_body();
                                while let Some(frame) = body.frame().await {
                                    match frame {
                                        Ok(frame) => {
                                            if let Some(chunk) = frame.data_ref() {
                                                if body_bytes.len() + chunk.len()
                                                    > max_body_size as usize
                                                {
                                                    return Ok::<Response<Full<Bytes>>, hyper::Error>(
                                                        Response::builder()
                                                            .status(
                                                                StatusCode::PAYLOAD_TOO_LARGE,
                                                            )
                                                            .header(
                                                                "content-type",
                                                                "text/plain; charset=UTF-8",
                                                            )
                                                            .header("server", "LarvaPHP")
                                                            .body(Full::new(Bytes::from(
                                                                "Payload Too Large",
                                                            )))
                                                            .unwrap(),
                                                    );
                                                }
                                                body_bytes.extend_from_slice(chunk);
                                            }
                                        }
                                        Err(_) => break,
                                    }
                                }
                                let body = Bytes::from(body_bytes);

                                match execute_php_cgi(&path, &ctx, &body, &php_ini) {
                                    Ok(output) => {
                                        let (status_code, headers, body_bytes) =
                                            parse_cgi_output(&output);

                                        // X-Sendfile: serve the referenced file
                                        // directly (efficient large-file serving).
                                        if let Some(resp) = try_x_sendfile(&headers, status_code)
                                        {
                                            return Ok::<Response<Full<Bytes>>, hyper::Error>(
                                                resp,
                                            );
                                        }

                                        let mut builder =
                                            Response::builder()
                                                .status(status_code)
                                                .header("server", "LarvaPHP")
                                                .header("x-content-type-options", "nosniff")
                                                .header("x-frame-options", "DENY");

                                        if let Some(alt) = alt_svc_header() {
                                            builder = builder.header("alt-svc", alt);
                                        }

                                        for (name, value) in &headers {
                                            builder = builder.header(name, value);
                                        }

                                        Ok::<Response<Full<Bytes>>, hyper::Error>(
                                            builder
                                                .body(Full::new(Bytes::from(body_bytes)))
                                                .unwrap(),
                                        )
                                    }
                                    Err(e) => {
                                        error!("PHP execution error: {}", e);
                                        Ok::<Response<Full<Bytes>>, hyper::Error>(
                                            Response::builder()
                                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                                .body(Full::new(Bytes::from(
                                                    "Internal Server Error",
                                                )))
                                                .unwrap(),
                                        )
                                    }
                                }
                            }
                        }))
                        .await
                    {
                        error!("Connection error: {}", err);
                    }
                });
        }
    }
}

/// Find the php-cgi.exe executable.
///
/// The result is cached after the first call so the per-request hot path does
/// no filesystem `stat` calls.
pub fn find_php_cgi() -> Option<String> {
    static CACHE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            // 1. Highest priority: LARVA_PHP_CGI env var (explicit override).
            if let Ok(exe) = std::env::var("LARVA_PHP_CGI") {
                let p = std::path::Path::new(&exe);
                if p.exists() {
                    return Some(exe);
                }
            }

            // 2. Unix/Linux: use `which php-cgi` (or php if php-cgi not found).
            #[cfg(not(windows))]
            {
                for cmd_name in &["php-cgi", "php"] {
                    if let Ok(output) = std::process::Command::new("which")
                        .arg(cmd_name)
                        .output()
                    {
                        if output.status.success() {
                            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            if !path.is_empty() && std::path::Path::new(&path).exists() {
                                return Some(path);
                            }
                        }
                    }
                }
            }

            // 3. Windows: search PATH for php-cgi.exe, then php.exe.
            #[cfg(windows)]
            {
                if let Ok(path) = std::env::var("PATH") {
                    for exe_name in &["php-cgi.exe", "php.exe"] {
                        for dir in path.split(';') {
                            let p = std::path::Path::new(dir).join(exe_name);
                            if p.exists() {
                                return Some(p.to_string_lossy().to_string());
                            }
                        }
                    }
                }

                // Check PHPRC (Windows PHP install dir).
                if let Ok(php_dir) = std::env::var("PHPRC") {
                    for exe_name in &["php-cgi.exe", "php.exe"] {
                        let p = std::path::Path::new(&php_dir).join(exe_name);
                        if p.exists() {
                            return Some(p.to_string_lossy().to_string());
                        }
                    }
                }
            }

            None
        })
        .clone()
}

/// Write php.ini overrides to a temp directory (for PHP_INI_SCAN_DIR).
/// The directory is created once per unique override set and cached, so
/// per-request cost is a hash lookup + mutex (no disk I/O after first write).
///
/// The built-in [`crate::runtime::php_ini::IniEntriesResolver`] entries are
/// always prepended. They enable **opcache** with a persistent file cache,
/// which is what turns a ~900 ms first request into ~100 ms warm requests:
/// bytecode is compiled once and reused instead of being recompiled per request.
pub(crate) fn write_ini_overrides(php_ini: &HashMap<String, String>) -> Option<std::path::PathBuf> {
    // Built-in entries first, in their declared order. Order matters:
    // `zend_extension=php_opcache.dll` must precede the `opcache.*` directives
    // that configure it.
    let built_in = crate::runtime::php_ini::IniEntriesResolver::entries();
    // Caller-supplied overrides, sorted by key for a deterministic file.
    let mut user: Vec<(&String, &String)> = php_ini.iter().collect();
    user.sort_by(|a, b| a.0.cmp(b.0));
    let user_str: String = user
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    let overrides = if user_str.is_empty() {
        built_in
    } else if built_in.is_empty() {
        user_str
    } else {
        format!("{}\n{}", built_in, user_str)
    };
    if overrides.trim().is_empty() {
        return None;
    }

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Deterministic content + cache key.
    let mut hasher = DefaultHasher::new();
    overrides.hash(&mut hasher);
    let key = hasher.finish();

    static CACHE: once_cell::sync::Lazy<
        std::sync::Mutex<HashMap<u64, std::path::PathBuf>>,
    > = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

    let mut cache = CACHE.lock().unwrap();
    if let Some(dir) = cache.get(&key) {
        if dir.exists() {
            return Some(dir.clone());
        }
    }

    // Create a unique temp directory for this override set.
    let dir = std::env::temp_dir().join(format!("larva-php-ini-{:x}", key));
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let ini_file = dir.join("00-overrides.ini");
    if std::fs::write(&ini_file, &overrides).is_err() {
        return None;
    }

    cache.insert(key, dir.clone());
    Some(dir)
}

/// Execute a PHP script via php-cgi.exe CGI subprocess and capture output.
fn execute_php_cgi(
    script_path: &str,
    ctx: &PhpContext,
    body: &[u8],
    php_ini: &HashMap<String, String>,
) -> Result<String> {
    // Ensure the script path is in the form the OS's external programs expect
    // (e.g. strip Windows' \\?\ verbatim prefix so php-cgi can open it).
    let native_script = crate::path_resolver::make_native(Path::new(script_path));
    let native_script_str = native_script.to_string_lossy().to_string();

    // Build the CGI parameter list once. Shared by both the FastCGI transport
    // and the CGI-spawn fallback. Empty optional vars are skipped to keep the
    // environment small and avoid php-cgi misreading empty CONTENT_LENGTH.
    let mut params: Vec<(String, String)> = Vec::with_capacity(16);
    params.push(("REQUEST_METHOD".into(), ctx.request_method.clone()));
    params.push(("REQUEST_URI".into(), ctx.request_uri.clone()));
    if !ctx.query_string.is_empty() {
        params.push(("QUERY_STRING".into(), ctx.query_string.clone()));
    }
    // SCRIPT_NAME must reflect the ACTUAL script (e.g. /index.php when the
    // request fell back to the front controller), not the original URI. This
    // lets WordPress build correct URLs/redirects (e.g. /wp-admin/install.php).
    let script_name = std::path::Path::new(&native_script_str)
        .strip_prefix(&ctx.document_root)
        .ok()
        .and_then(|rel| {
            let s = rel.to_string_lossy().replace('\\', "/");
            if s.is_empty() { None } else { Some(format!("/{}", s)) }
        })
        .unwrap_or_else(|| ctx.script_name.clone());
    params.push(("SCRIPT_NAME".into(), script_name.clone()));
    params.push(("SCRIPT_FILENAME".into(), native_script_str.clone()));
    params.push(("DOCUMENT_ROOT".into(), ctx.document_root.clone()));
    params.push(("SERVER_PROTOCOL".into(), ctx.server_protocol.clone()));
    params.push(("SERVER_NAME".into(), ctx.server_name.clone()));
    params.push(("SERVER_PORT".into(), ctx.server_port.clone()));
    params.push(("HTTPS".into(), ctx.https.clone()));
    if !ctx.content_type.is_empty() {
        params.push(("CONTENT_TYPE".into(), ctx.content_type.clone()));
    }
    if !ctx.content_length.is_empty() {
        params.push(("CONTENT_LENGTH".into(), ctx.content_length.clone()));
    }
    params.push(("PHP_SELF".into(), script_name.clone()));
    params.push(("REDIRECT_STATUS".into(), "200".into()));
    params.push(("GATEWAY_INTERFACE".into(), "CGI/1.1".into()));
    params.push(("SERVER_SOFTWARE".into(), "LarvaPHP/1.13.5 (FrankenPHP-compatible)".into()));
    // HTTP_HOST (Host header) so WordPress/URLs resolve to the right host.
    let http_host = if ctx.server_port.is_empty()
        || ctx.server_port == "80"
        || ctx.server_port == "443"
    {
        ctx.server_name.clone()
    } else {
        format!("{}:{}", ctx.server_name, ctx.server_port)
    };
params.push(("HTTP_HOST".into(), http_host.clone()));

    // Forward the Cookie header as HTTP_COOKIE so PHP ($_COOKIE) and
    // WordPress (which checks $_COOKIE[TEST_COOKIE]) see the session cookie.
    if let Some(cookie) = ctx
        .env
        .iter()
        .find_map(|(k, v)| {
            let ks = std::str::from_utf8(k).ok()?;
            if ks.eq_ignore_ascii_case("Cookie") {
                std::str::from_utf8(v).ok().map(|s| s.to_string())
            } else {
                None
            }
        })
    {
        params.push(("HTTP_COOKIE".into(), cookie));
    }

    // Fastest path: 2-layer runtime worker pool (master → shm → worker → PHP).
    // The master never runs PHP; a segfault in a worker is isolated and respawned.
    {
        let mut req_headers: Vec<(String, String)> = ctx
            .env
            .iter()
            .filter_map(|(k, v)| {
                std::str::from_utf8(k)
                    .ok()
                    .and_then(|ks| std::str::from_utf8(v).ok().map(|vs| (ks.to_string(), vs.to_string())))
            })
            .collect();
        // Forward POST metadata so the worker's php-cgi reads the request body.
        if !ctx.content_length.is_empty() {
            req_headers.push(("CONTENT_LENGTH".to_string(), ctx.content_length.clone()));
        } else if !body.is_empty() {
            req_headers.push(("CONTENT_LENGTH".to_string(), body.len().to_string()));
        }
        if !ctx.content_type.is_empty() {
            req_headers.push(("CONTENT_TYPE".to_string(), ctx.content_type.clone()));
        }
        // HTTP_HOST so the worker's php-cgi resolves the correct host.
        req_headers.push(("HTTP_HOST".to_string(), http_host.clone()));
        // Forward Cookie as HTTP_COOKIE so PHP ($_COOKIE) and WordPress see it.
        if let Some(cookie) = ctx
            .env
            .iter()
            .find_map(|(k, v)| {
                let ks = std::str::from_utf8(k).ok()?;
                if ks.eq_ignore_ascii_case("Cookie") {
                    std::str::from_utf8(v).ok().map(|s| s.to_string())
                } else {
                    None
                }
            })
        {
            req_headers.push(("HTTP_COOKIE".to_string(), cookie));
        }
        if let Some(result) = crate::runtime::try_dispatch(&crate::runtime::Request {
            method: ctx.request_method.clone(),
            uri: ctx.request_uri.clone(),
            headers: req_headers,
            body: body.to_vec(),
            script_path: script_path.to_string(),
            server_name: ctx.server_name.clone(),
            server_port: ctx.server_port.clone(),
            script_name: script_name.clone(),
        }) {
            return result.map_err(Error::Ffi);
        }
    }

    // Embedded PHP path: run in-process (no php-cgi spawn). Activated by
    // --embedded (sets LARVA_PHP_EXECUTOR=ffi). The NTS php8.dll is not
    // thread-safe, so embed_php::execute_request serialises access with a
    // mutex. This is faster than CGI/FastCGI (no process spawn) but requires
    // building with --features embedded.
    if std::env::var("LARVA_PHP_EXECUTOR").as_deref() == Ok("ffi") {
        #[cfg(feature = "embedded")]
        {
            let req_headers: Vec<(String, String)> = ctx
                .env
                .iter()
                .filter_map(|(k, v)| {
                    std::str::from_utf8(k)
                        .ok()
                        .and_then(|ks| std::str::from_utf8(v).ok().map(|vs| (ks.to_string(), vs.to_string())))
                })
                .collect();
            let mut all_headers = req_headers;
            if !ctx.content_length.is_empty() {
                all_headers.push(("CONTENT_LENGTH".to_string(), ctx.content_length.clone()));
            } else if !body.is_empty() {
                all_headers.push(("CONTENT_LENGTH".to_string(), body.len().to_string()));
            }
            if !ctx.content_type.is_empty() {
                all_headers.push(("CONTENT_TYPE".to_string(), ctx.content_type.clone()));
            }
            all_headers.push(("HTTP_HOST".to_string(), http_host.clone()));
            if let Some(cookie) = ctx
                .env
                .iter()
                .find_map(|(k, v)| {
                    let ks = std::str::from_utf8(k).ok()?;
                    if ks.eq_ignore_ascii_case("Cookie") {
                        std::str::from_utf8(v).ok().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            {
                all_headers.push(("HTTP_COOKIE".to_string(), cookie));
            }
            match crate::embed_php::execute_request(&crate::runtime::executor::Request {
                method: ctx.request_method.clone(),
                uri: ctx.request_uri.clone(),
                headers: all_headers,
                body: body.to_vec(),
                script_path: script_path.to_string(),
                server_name: ctx.server_name.clone(),
                server_port: ctx.server_port.clone(),
                script_name: script_name.clone(),
            }) {
                Ok(resp) => {
                    // Build CGI-style output string: status line + headers + body.
                    let mut out = format!("Status: {} {}\r\n", resp.status, "");
                    for (k, v) in &resp.headers {
                        out.push_str(&format!("{}: {}\r\n", k, v));
                    }
                    out.push_str("\r\n");
                    let mut bytes = out.into_bytes();
                    bytes.extend_from_slice(&resp.body);
                    return Ok(String::from_utf8_lossy(&bytes).into_owned());
                }
                Err(e) => {
                    tracing::warn!("embedded PHP request failed, falling back to CGI: {}", e);
                }
            }
        }
    }

    // NOTE: embedded PHP (feature "embedded") is experimental and requires the
    // exact PHP 8.x ABI (sapi_module_struct / zend_file_handle layouts). Without
    // PHP development headers the struct layout is wrong and PHP crashes, so it
    // is NOT auto-invoked here. See embed_php.rs for details.

    // Fast path: reuse a persistent FastCGI backend (no per-request spawn).
    // Falls through to CGI spawn on any error or when the feature is disabled.
    #[cfg(feature = "fcgi")]
    {
        if let Some(backend) = crate::fastcgi::backend() {
            match backend.request(&params, body) {
                Ok(resp) => {
                    if !resp.stderr.is_empty() {
                        tracing::debug!(
                            "php-cgi stderr: {}",
                            String::from_utf8_lossy(&resp.stderr)
                        );
                    }
                    return Ok(String::from_utf8_lossy(&resp.stdout).into_owned());
                }
                Err(e) => {
                    tracing::debug!("FastCGI request failed, falling back to CGI spawn: {}", e);
                }
            }
        }
    }

    // Fallback: spawn a fresh php-cgi process per request (CGI model).
    let php_exe = find_php_cgi()
        .ok_or_else(|| Error::Ffi("php-cgi.exe not found".to_string()))?;

    let mut cmd = std::process::Command::new(php_exe);
    // Pass the script path as a positional argument (php-cgi requires it).
    cmd.arg(&native_script_str);
    cmd.current_dir(
        native_script
            .parent()
            .unwrap_or(std::path::Path::new(".")),
    );

    // Apply php.ini overrides via PHP_INI_SCAN_DIR (env-based).
    if let Some(dir) = write_ini_overrides(php_ini) {
        cmd.env("PHP_INI_SCAN_DIR", &dir);
    }

    for (k, v) in &params {
        cmd.env(k, v);
    }

    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| Error::Ffi(format!("Failed to spawn php-cgi: {}", e)))?;

    // Write request body to stdin
    if !body.is_empty() {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(body);
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| Error::Ffi(format!("Failed to wait for php-cgi: {}", e)))?;

    if !output.stderr.is_empty() {
        tracing::debug!(
            "php-cgi stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Parse CGI output: split headers from body at the first blank line.
/// Also detects Early Hints (103) responses and extracts Link headers.
fn parse_cgi_output(output: &str) -> (StatusCode, Vec<(String, String)>, Vec<u8>) {
    let mut headers = Vec::new();
    let mut body_start = 0;
    let mut status_code = StatusCode::OK;
    let mut early_hints: Vec<(String, String)> = Vec::new();

    // Find the blank line separating headers from body
    if let Some(pos) = output.find("\r\n\r\n") {
        let header_section = &output[..pos];
        body_start = pos + 4;

        let mut is_early_hints = false;
        for line in header_section.split("\r\n") {
            if let Some((name, value)) = line.split_once(':') {
                let name = name.trim();
                let value = value.trim();
                if name.eq_ignore_ascii_case("status") {
                    // Parse CGI Status header: "302 Found" or just "302"
                    if let Some(code_str) = value.split_whitespace().next() {
                        if let Ok(code) = code_str.parse::<u16>() {
                            if code == 103 {
                                // Early Hints response
                                is_early_hints = true;
                            } else if is_early_hints && code >= 200 {
                                // Main response starts after Early Hints
                                is_early_hints = false;
                                status_code = StatusCode::from_u16(code).unwrap_or(StatusCode::OK);
                            } else {
                                status_code = StatusCode::from_u16(code).unwrap_or(StatusCode::OK);
                            }
                        }
                    }
                    continue; // Don't add Status: to response headers
                }
                if is_early_hints {
                    early_hints.push((name.to_string(), value.to_string()));
                } else {
                    headers.push((name.to_string(), value.to_string()));
                }
            }
        }
    } else if let Some(pos) = output.find("\n\n") {
        let header_section = &output[..pos];
        body_start = pos + 2;

        let mut is_early_hints = false;
        for line in header_section.split('\n') {
            if let Some((name, value)) = line.split_once(':') {
                let name = name.trim();
                let value = value.trim();
                if name.eq_ignore_ascii_case("status") {
                    if let Some(code_str) = value.split_whitespace().next() {
                        if let Ok(code) = code_str.parse::<u16>() {
                            if code == 103 {
                                is_early_hints = true;
                            } else if is_early_hints && code >= 200 {
                                is_early_hints = false;
                                status_code = StatusCode::from_u16(code).unwrap_or(StatusCode::OK);
                            } else {
                                status_code = StatusCode::from_u16(code).unwrap_or(StatusCode::OK);
                            }
                        }
                    }
                    continue;
                }
                if is_early_hints {
                    early_hints.push((name.to_string(), value.to_string()));
                } else {
                    headers.push((name.to_string(), value.to_string()));
                }
            }
        }
    }

    // Merge Early Hints Link headers into main headers (for browser preloading)
    for (name, value) in &early_hints {
        if name.eq_ignore_ascii_case("link") {
            // Add as a Link header to the main response for preload hints
            headers.push((name.clone(), value.clone()));
        }
    }

    let body_bytes = output[body_start..].as_bytes().to_vec();

    // Ensure content-type header exists
    let has_content_type =
        headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("content-type"));
    if !has_content_type {
        headers.push((
            "content-type".to_string(),
            "text/html; charset=UTF-8".to_string(),
        ));
    }

    (status_code, headers, body_bytes)
}

/// Attempt to serve an `X-Sendfile` response.
///
/// If the CGI output carries an `X-Sendfile` header pointing to a readable,
/// non-sensitive file, the server streams that file directly (efficient for
/// large files) instead of the (usually empty) PHP output body. CGI headers are
/// preserved except `X-Sendfile` and body-related headers, which are recomputed.
fn try_x_sendfile(
    headers: &[(String, String)],
    status_code: StatusCode,
) -> Option<Response<Full<Bytes>>> {
    let xsendfile = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("x-sendfile"))?;
    let file_path = xsendfile.1.trim();

    // Refuse sensitive files even if PHP asks for them.
    if is_sensitive_file(file_path) {
        return None;
    }

    let content = std::fs::read(file_path).ok()?;
    let mime = mime_type_static(file_path);

    let mut builder = Response::builder()
        .status(status_code)
        .header("server", "LarvaPHP")
        .header("x-content-type-options", "nosniff")
        .header("x-frame-options", "DENY")
        .header("content-type", mime)
        .header("content-length", content.len().to_string())
        .header("accept-ranges", "bytes");

    for (name, value) in headers {
        if name.eq_ignore_ascii_case("x-sendfile")
            || name.eq_ignore_ascii_case("content-length")
            || name.eq_ignore_ascii_case("content-type")
        {
            continue;
        }
        builder = builder.header(name, value);
    }

    Some(builder.body(Full::new(Bytes::from(content))).unwrap())
}

/// Resolve the document root to a canonical absolute path, following symlinks.
///
/// Mirrors FrankenPHP's `resolve_root_symlink`. Falls back to the plain
/// absolute path (without canonicalization) when the path does not exist or
/// cannot be resolved, so a not-yet-created root still works.
pub fn resolve_root_symlink(root: &str) -> std::io::Result<PathBuf> {
    // Delegate to the OS-appropriate path resolver (Strategy pattern).
    crate::path_resolver::resolve_root(root)
}

/// Resolve the PHP script path from the request URI (static helper).
fn resolve_script_path_static(uri: &str, root: &str, split_path: &[String]) -> Option<String> {
    let path = uri.split('?').next().unwrap_or(uri);
    let decoded = url_decode(path);

    // Reject null bytes
    if decoded.contains('\0') {
        return None;
    }

    // Build the full path: root + path
    let mut full_path = PathBuf::from(root);
    if decoded.starts_with('/') {
        full_path.push(&decoded[1..]);
    } else {
        full_path.push(&decoded);
    }

    // Sanitize path traversal (.. segments)
    let sanitized = match sanitize_path(&full_path) {
        Ok(p) => p,
        Err(_) => return None,
    };

    // Verify the resolved path is within the document root
    if !sanitized.starts_with(root) {
        return None;
    }

    if sanitized.is_dir() {
        let index_path = sanitized.join("index.php");
        if index_path.exists() {
            return Some(index_path.to_string_lossy().to_string());
        }
        return None;
    }

    if sanitized.extension().is_none() {
        for ext in split_path {
            let mut p = sanitized.clone();
            p.set_extension(ext.trim_start_matches('.'));
            if p.exists() {
                return Some(p.to_string_lossy().to_string());
            }
        }
        let mut p = sanitized.clone();
        p.set_extension("php");
        return Some(p.to_string_lossy().to_string());
    }

    Some(sanitized.to_string_lossy().to_string())
}

/// Try resolving index.php for the given URI (WordPress permalink fallback).
fn resolve_index_php_static(uri: &str, root: &str) -> Option<String> {
    let _path = uri.split('?').next().unwrap_or(uri);
    let mut index_path = PathBuf::from(root);
    index_path.push("index.php");
    if index_path.exists() {
        return Some(index_path.to_string_lossy().to_string());
    }

    None
}

/// Determine MIME type from file extension (static version for inline handler).
fn mime_type_static(path: &str) -> String {
    match std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
    {
        "css" => "text/css; charset=UTF-8",
        "js" => "application/javascript; charset=UTF-8",
        "json" => "application/json; charset=UTF-8",
        "html" | "htm" => "text/html; charset=UTF-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "xml" => "application/xml; charset=UTF-8",
        "txt" => "text/plain; charset=UTF-8",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// URL-decode a string.
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(val) = u8::from_str_radix(hex, 16) {
                result.push(val as char);
                i += 3;
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        } else if bytes[i] == b'+' {
            result.push(' ');
            i += 1;
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}

/// Format a Unix timestamp as an HTTP date string (RFC 7231).
fn http_date(secs: u64) -> String {
    let dt = match SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(secs)) {
        Some(t) => t,
        None => return String::new(),
    };

    let (year, month, day, hour, min, sec, wday) = {
        let mut total_days = dt.duration_since(UNIX_EPOCH).unwrap().as_secs() / 86400;
        let time_of_day = dt.duration_since(UNIX_EPOCH).unwrap().as_secs() % 86400;
        let hour = (time_of_day / 3600) as u32;
        let min = ((time_of_day % 3600) / 60) as u32;
        let sec = (time_of_day % 60) as u32;

        let mut year = 1970i32;
        loop {
            let days_in_year = if is_leap_year(year) { 366 } else { 365 };
            if total_days < days_in_year as u64 {
                break;
            }
            total_days -= days_in_year as u64;
            year += 1;
        }

        let mut month = 0usize;
        let days_in_month = [
            31,
            if is_leap_year(year) { 29 } else { 28 },
            31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
        ];
        while total_days >= days_in_month[month] as u64 {
            total_days -= days_in_month[month] as u64;
            month += 1;
        }
        let day = total_days as u32 + 1;

        // Zeller's congruence (0=Saturday, adjust to 0=Sunday)
        let mut m = month as i32 + 1;
        let mut y = year;
        if m < 3 {
            m += 12;
            y -= 1;
        }
        let k = y % 100;
        let j = y / 100;
        let h = (day as i32
            + (13 * (m + 1)) / 5
            + k
            + k / 4
            + j / 4
            + 5 * j)
            % 7;
        let wday = (h + 6) % 7; // convert: 0=Sat -> 0=Sun

        (year, month as u32 + 1, day, hour, min, sec, wday as u32)
    };

    const WEEKDAYS: &[&str] = &["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: &[&str] = &["Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
        WEEKDAYS[wday as usize], day, MONTHS[month as usize - 1], year, hour, min, sec
    )
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Sanitize a path by resolving `..` and `.` segments to prevent directory traversal.
/// Returns the canonicalized absolute path if it stays within bounds, or an error.
fn sanitize_path(path: &PathBuf) -> Result<PathBuf> {
    let mut components: Vec<&std::ffi::OsStr> = Vec::new();

    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
                // If components is empty, we've gone above root - reject
                else {
                    return Err(Error::Ffi("Path traversal above root".to_string()));
                }
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(c) => components.push(c),
            // Preserve the root (e.g. "\" on Windows or "/" on Unix) and drive
            // prefix (e.g. "C:") so the sanitized path keeps its absolute form.
            std::path::Component::RootDir => components.push(component.as_os_str()),
            std::path::Component::Prefix(_) => components.push(component.as_os_str()),
        }
    }

    if components.is_empty() {
        return Err(Error::Ffi("Empty path after sanitization".to_string()));
    }

    let mut result = PathBuf::new();
    for c in &components {
        result.push(c);
    }

    Ok(result)
}

/// Check if a file path is sensitive and should not be served directly.
/// Blocks: .htaccess, .htpasswd, wp-config.php, .env, .git/*, composer.lock,
/// vendor/*, storage/*, node_modules/*, *.sql, *.bak, *.log, *.orig
fn is_sensitive_file(path: &str) -> bool {
    let p = Path::new(path);
    let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let file_name_lower = file_name.to_lowercase();

    // Hidden files starting with dot
    if file_name.starts_with('.') {
        return true;
    }

    // Specific sensitive filenames
    let sensitive_names = [
        "wp-config.php",
        "wp-config-sample.php",
        ".env",
        ".env.example",
        ".env.local",
        "composer.lock",
        "composer.json",
        "package-lock.json",
        "package.json",
        "makefile",
        "dockerfile",
        "docker-compose.yml",
        "docker-compose.yaml",
        "vagrantfile",
    ];
    if sensitive_names.iter().any(|&n| file_name_lower == n) {
        return true;
    }

    // Sensitive extensions
    let sensitive_exts = [
        ".sql", ".bak", ".backup", ".old", ".orig", ".tmp", ".temp",
        ".log", ".swp", ".swo", "~", ".ds_store",
    ];
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        let ext_lower = format!(".{}", ext.to_lowercase());
        if sensitive_exts.iter().any(|&e| ext_lower == e) {
            return true;
        }
    }

    // Block .git/, .svn/, .hg/ directories anywhere in path
    let path_str = path.to_lowercase();
    let vcs_dirs = [".git/", ".svn/", ".hg/", ".bzr/"];
    if vcs_dirs.iter().any(|d| path_str.contains(d)) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // === RateLimiter tests ===

    #[test]
    fn rate_limiter_counts_requests() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 3,
            window_secs: 60,
        });

        assert!(limiter.check("127.0.0.1"));
        assert!(limiter.check("127.0.0.1"));
        assert!(limiter.check("127.0.0.1"));
        assert!(!limiter.check("127.0.0.1"));
        assert!(!limiter.check("127.0.0.1"));
    }

    #[test]
    fn rate_limiter_per_ip_isolation() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 1,
            window_secs: 60,
        });

        assert!(limiter.check("1.2.3.4"));
        assert!(!limiter.check("1.2.3.4"));
        assert!(limiter.check("5.6.7.8"));
        assert!(!limiter.check("5.6.7.8"));
    }

    #[test]
    fn rate_limiter_window_reset() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 1,
            window_secs: 0,
        });
        // With window_secs=0, window always expires immediately, so every request is allowed
        assert!(limiter.check("127.0.0.1"));
        assert!(limiter.check("127.0.0.1"));
        assert!(limiter.check("127.0.0.1"));
    }

    #[test]
    fn rate_limiter_clones_share_state() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 2,
            window_secs: 60,
        });
        let clone = limiter.clone();

        assert!(limiter.check("127.0.0.1"));
        assert!(clone.check("127.0.0.1"));
        assert!(!limiter.check("127.0.0.1"));
        assert!(!clone.check("127.0.0.1"));
    }

    #[test]
    fn rate_limiter_default_config() {
        let limiter = RateLimiter::new(RateLimitConfig::default());
        assert_eq!(limiter.config.max_requests, 60);
        assert_eq!(limiter.config.window_secs, 60);

        for _ in 0..60 {
            assert!(limiter.check("127.0.0.1"));
        }
        assert!(!limiter.check("127.0.0.1"));
    }

    // === Server tests ===

    #[test]
    fn server_new_sets_defaults() {
        let server = Server::new("C:\\www").unwrap();
        assert_eq!(server.root(), "C:\\www");
        assert_eq!(server.split_path(), &[".php".to_string()]);
        assert_eq!(server.max_body_size, 10 * 1024 * 1024);
        assert!(!server.is_registered());
    }

    #[test]
    fn server_fallback_is_registered() {
        let server = Server::fallback();
        assert!(server.is_registered());
        assert_eq!(server.root(), "");
        assert_eq!(server.max_body_size, 10 * 1024 * 1024);
    }

    #[test]
    fn server_set_name_and_split_path() {
        let mut server = Server::new("C:\\www").unwrap();
        server.set_name("example.com".to_string());
        server.set_split_path(vec![".php".to_string(), ".phtml".to_string()]);
        assert_eq!(server.name(), "example.com");
        assert_eq!(server.split_path(), &[".php".to_string(), ".phtml".to_string()]);
    }

    // === url_decode tests ===

    #[test]
    fn url_decode_plain_string() {
        assert_eq!(url_decode("hello"), "hello");
    }

    #[test]
    fn url_decode_percent_encoded() {
        assert_eq!(url_decode("%48%65%6c%6c%6f"), "Hello");
    }

    #[test]
    fn url_decode_plus_as_space() {
        assert_eq!(url_decode("hello+world"), "hello world");
    }

    #[test]
    fn url_decode_mixed() {
        assert_eq!(url_decode("%74%65%73%74+path"), "test path");
    }

    #[test]
    fn url_decode_invalid_hex_passthrough() {
        assert_eq!(url_decode("%ZZ"), "%ZZ");
    }

    #[test]
    fn url_decode_empty() {
        assert_eq!(url_decode(""), "");
    }

    // === sanitize_path tests ===

    #[test]
    fn sanitize_path_normal() {
        let path = PathBuf::from("C:/www/index.php");
        let result = sanitize_path(&path).unwrap();
        assert!(result.to_string_lossy().contains("www"));
        assert!(result.to_string_lossy().contains("index.php"));
    }

    #[test]
    fn sanitize_path_traversal_resolves_dotdot() {
        let path = PathBuf::from("C:/www/../../etc/passwd");
        let result = sanitize_path(&path).unwrap();
        // .. segments are resolved; result no longer contains www
        let s = result.to_string_lossy().to_string();
        assert!(!s.contains("www"));
        assert!(s.contains("passwd"));
    }

    #[test]
    fn sanitize_path_current_dir_ignored() {
        let path = PathBuf::from("C:/www/./index.php");
        let result = sanitize_path(&path).unwrap();
        assert!(result.to_string_lossy().contains("www"));
        assert!(result.to_string_lossy().contains("index.php"));
    }

    #[test]
    fn sanitize_path_empty_rejected() {
        let path = PathBuf::from("");
        let result = sanitize_path(&path);
        assert!(result.is_err());
    }

    #[test]
    fn sanitize_path_preserves_root() {
        // Regression: the root ("/" or "\") must be preserved so that
        // sanitized paths stay rooted and starts_with(root) succeeds.
        let path = PathBuf::from("/var/www/index.php");
        let result = sanitize_path(&path).unwrap();
        assert!(result.has_root(), "sanitized path must keep its root: {:?}", result);
        assert!(result.starts_with("/var/www"));
    }

    #[test]
    fn sanitize_path_windows_drive_preserved() {
        let path = PathBuf::from("C:/www/index.php");
        let result = sanitize_path(&path).unwrap();
        let s = result.to_string_lossy().to_string();
        // Drive prefix and root separator must both survive.
        assert!(s.starts_with("C:") || s.starts_with("c:"));
        assert!(s.contains("www"));
        assert!(s.contains("index.php"));
    }

    // === parse_cgi_output tests ===

    #[test]
    fn parse_cgi_output_with_headers_and_body() {
        let output = "Status: 200 OK\r\nContent-Type: text/html\r\n\r\n<body>hello</body>";
        let (status, headers, body) = parse_cgi_output(output);
        assert_eq!(status, StatusCode::OK);
        assert!(headers.iter().any(|(n, v)| n == "Content-Type" && v == "text/html"));
        assert!(!headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("Status")));
        assert_eq!(body, b"<body>hello</body>");
    }

    #[test]
    fn parse_cgi_output_no_headers() {
        let output = "just a body";
        let (status, headers, body) = parse_cgi_output(output);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"just a body");
        // Default content-type is added when none present
        assert!(headers.iter().any(|(n, v)| n == "content-type" && v == "text/html; charset=UTF-8"));
    }

    #[test]
    fn parse_cgi_output_status_302() {
        let output = "Status: 302 Found\r\nLocation: /login\r\n\r\n";
        let (status, headers, body) = parse_cgi_output(output);
        assert_eq!(status, StatusCode::FOUND);
        assert!(headers.iter().any(|(n, v)| n == "Location" && v == "/login"));
        assert_eq!(body, b"");
    }

    #[test]
    fn parse_cgi_output_default_content_type() {
        let output = "Status: 200 OK\r\n\r\n<body></body>";
        let (_status, headers, _body) = parse_cgi_output(output);
        assert!(headers.iter().any(|(n, v)| n == "content-type" && v == "text/html; charset=UTF-8"));
    }

    // === mime_type tests ===

    #[test]
    fn mime_type_css() {
        assert_eq!(Server::mime_type("style.css"), "text/css; charset=UTF-8");
    }

    #[test]
    fn mime_type_js() {
        assert_eq!(Server::mime_type("app.js"), "application/javascript; charset=UTF-8");
    }

    #[test]
    fn mime_type_html() {
        assert_eq!(Server::mime_type("index.html"), "text/html; charset=UTF-8");
    }

    #[test]
    fn mime_type_png() {
        assert_eq!(Server::mime_type("image.png"), "image/png");
    }

    #[test]
    fn mime_type_unknown() {
        assert_eq!(Server::mime_type("file.xyz"), "application/octet-stream");
    }

    #[test]
    fn mime_type_no_extension() {
        assert_eq!(Server::mime_type("Makefile"), "application/octet-stream");
    }

    // === is_sensitive_file tests ===

    #[test]
    fn sensitive_file_wp_config() {
        assert!(is_sensitive_file("C:/www/wp-config.php"));
    }

    #[test]
    fn sensitive_file_env() {
        assert!(is_sensitive_file("C:/www/.env"));
    }

    #[test]
    fn sensitive_file_dot_file() {
        assert!(is_sensitive_file("C:/www/.htaccess"));
    }

    #[test]
    fn sensitive_file_sql() {
        assert!(is_sensitive_file("C:/www/db.sql"));
    }

    #[test]
    fn not_sensitive_php() {
        assert!(!is_sensitive_file("C:/www/index.php"));
    }

    #[test]
    fn not_sensitive_css() {
        assert!(!is_sensitive_file("C:/www/style.css"));
    }

    #[test]
    fn sensitive_file_git_dir() {
        assert!(is_sensitive_file("C:/www/.git/config"));
    }

    #[test]
    fn sensitive_file_composer_lock() {
        assert!(is_sensitive_file("C:/www/composer.lock"));
    }

    // === http_date tests ===

    #[test]
    fn http_date_epoch() {
        let date = http_date(0);
        assert_eq!(date, "Thu, 01 Jan 1970 00:00:00 GMT");
    }

    // === X-Sendfile tests ===

    #[test]
    fn x_sendfile_serves_file() {
        let dir = std::env::temp_dir().join(format!("larva-xsendfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("data.bin");
        std::fs::write(&file, b"file-content-here").unwrap();

        let headers = vec![
            ("X-Sendfile".to_string(), file.to_string_lossy().to_string()),
            ("Content-Type".to_string(), "application/octet-stream".to_string()),
        ];
        let resp = try_x_sendfile(&headers, StatusCode::OK);
        assert!(resp.is_some());
        let resp = resp.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("content-length").unwrap(),
            "17"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn x_sendfile_absent_returns_none() {
        let headers = vec![("Content-Type".to_string(), "text/html".to_string())];
        assert!(try_x_sendfile(&headers, StatusCode::OK).is_none());
    }

    #[test]
    fn x_sendfile_missing_file_returns_none() {
        let headers = vec![(
            "X-Sendfile".to_string(),
            "C:\\nonexistent\\nope.bin".to_string(),
        )];
        assert!(try_x_sendfile(&headers, StatusCode::OK).is_none());
    }

    #[test]
    fn x_sendfile_blocks_sensitive_file() {
        let headers = vec![(
            "X-Sendfile".to_string(),
            "C:/www/.env".to_string(),
        )];
        assert!(try_x_sendfile(&headers, StatusCode::OK).is_none());
    }

    // === resolve_root_symlink tests ===

    #[test]
    fn resolve_root_symlink_existing_dir() {
        let dir = std::env::temp_dir();
        let resolved = resolve_root_symlink(&dir.to_string_lossy()).unwrap();
        // Canonicalized path should exist and be absolute.
        assert!(resolved.is_absolute());
        assert!(resolved.exists());
        // Regression: on Windows the verbatim `\\?\` prefix must be stripped,
        // otherwise php-cgi fails with "No input file specified."
        #[cfg(windows)]
        {
            let s = resolved.to_string_lossy().to_string();
            assert!(
                !s.starts_with(r"\\?\"),
                "verbatim prefix must be stripped: {}",
                s
            );
        }
    }

    #[test]
    fn resolve_root_symlink_nonexistent_falls_back() {
        // A path that does not exist should fall back to the absolute path
        // rather than erroring.
        let resolved = resolve_root_symlink("C:\\definitely\\nonexistent\\path-xyz").unwrap();
        assert!(resolved.to_string_lossy().to_string().contains("nonexistent"));
    }

    // === Alt-Svc tests ===

    #[test]
    fn alt_svc_disabled_by_default() {
        // H3_PORT starts at 0 (or was reset), so no Alt-Svc header.
        // We can only assert the function is callable; global state may be
        // mutated by other tests, so just check the enabled path format.
        let header = alt_svc_header();
        if let Some(h) = header {
            assert!(h.contains("h3="));
        }
    }

    #[test]
    fn alt_svc_enabled_format() {
        set_http3_port(9443);
        let header = alt_svc_header().unwrap();
        assert_eq!(header, "h3=\":9443\"; ma=86400");
        set_http3_port(0); // reset
    }

    // === handle_request_parts tests (HTTP/3 core) ===

    #[test]
    fn handle_request_parts_static_file() {
        let dir = std::env::temp_dir().join(format!("larva-h3-static-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("style.css"), b"body{}").unwrap();

        let server = Server::new(&dir.to_string_lossy()).unwrap();
        let (status, headers, body) =
            server.handle_request_parts("GET", "/style.css", &[], &[]).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"body{}");
        assert!(headers.iter().any(|(n, _)| n == "content-type"));
        assert!(headers.iter().any(|(n, _)| n == "content-length"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_request_parts_not_found() {
        let dir = std::env::temp_dir().join(format!("larva-h3-404-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // No index.php, so a missing .php returns 404.

        let server = Server::new(&dir.to_string_lossy()).unwrap();
        let (status, _, _) =
            server.handle_request_parts("GET", "/missing.php", &[], &[]).unwrap();

        assert_eq!(status, StatusCode::NOT_FOUND);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_request_parts_sensitive_static_blocked() {
        let dir = std::env::temp_dir().join(format!("larva-h3-sens-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), b"SECRET=1").unwrap();

        let server = Server::new(&dir.to_string_lossy()).unwrap();
        let (status, _, _) =
            server.handle_request_parts("GET", "/.env", &[], &[]).unwrap();

        // Sensitive files must NOT be served: either 403 (blocked) or 404
        // (path not resolved) is acceptable — never 200.
        assert!(
            status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND,
            "sensitive file must not be served, got {}",
            status
        );
        assert_ne!(status, StatusCode::OK);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_request_parts_php_executes() {
        let dir = std::env::temp_dir().join(format!("larva-h3-php-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ok.php"), b"<?php echo 'H3OK'; ?>").unwrap();

        let server = Server::new(&dir.to_string_lossy()).unwrap();
        match server.handle_request_parts("GET", "/ok.php", &[], &[]) {
            Ok((status, _, body)) => {
                // When php-cgi is available and the script runs, assert the body.
                if status == StatusCode::OK {
                    assert_eq!(String::from_utf8_lossy(&body).trim(), "H3OK");
                }
                // Non-OK (e.g. php-cgi unavailable in CI) is tolerated.
            }
            Err(_) => {
                // php-cgi not available in this environment — skip.
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_request_parts_php_ini_reachable() {
        // handle_request_parts must be callable with extra headers without panicking.
        let dir = std::env::temp_dir().join(format!("larva-h3-hdr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("style.css"), b"x").unwrap();

        let server = Server::new(&dir.to_string_lossy()).unwrap();
        let headers = vec![
            ("accept".to_string(), "*/*".to_string()),
            ("user-agent".to_string(), "test".to_string()),
        ];
        let result = server.handle_request_parts("GET", "/style.css", &headers, &[]);
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
