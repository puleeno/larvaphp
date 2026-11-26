//! Per-request context.
//!
//! Mirrors `context.go` from the Go source. Holds the request context that
//! is passed to PHP threads for execution.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::errors::{Error, Result};
use crate::env::PreparedEnv;

/// Trait for options that configure a request.
pub trait RequestOption: Send + Sync {
    fn apply(&self, ctx: &mut PhpContext) -> Result<()>;
}

/// The context for a single PHP request.
///
/// This is the Rust equivalent of `frankenPHPContext` in Go.
pub struct PhpContext {
    /// The request context (cancellation, deadlines, etc.)
    pub ctx: tokio::sync::watch::Receiver<Option<()>>,
    /// Document root directory
    pub document_root: String,
    /// Split path patterns (e.g. [".php"])
    pub split_path: Vec<String>,
    /// Prepared environment variables
    pub env: PreparedEnv,
    /// Logger for this request
    pub logger: tracing::Level,
    /// The assigned worker (if any)
    pub worker: Option<Arc<crate::worker::Worker>>,
    /// The server block
    pub server: Arc<crate::server::Server>,
    /// Request body idle timeout
    pub request_body_timeout: Option<std::time::Duration>,
    /// Document URI (path minus PATH_INFO)
    pub doc_uri: String,
    /// PATH_INFO
    pub path_info: String,
    /// SCRIPT_NAME
    pub script_name: String,
    /// SCRIPT_FILENAME (absolute path)
    pub script_filename: String,
    /// REQUEST_URI
    pub request_uri: String,
    /// QUERY_STRING
    pub query_string: String,
    /// REQUEST_METHOD
    pub request_method: String,
    /// CONTENT_TYPE
    pub content_type: String,
    /// CONTENT_LENGTH
    pub content_length: String,
    /// SERVER_PROTOCOL
    pub server_protocol: String,
    /// SERVER_NAME
    pub server_name: String,
    /// SERVER_PORT
    pub server_port: String,
    /// HTTPS (on/off)
    pub https: String,
    /// Whether headers have been written
    pub headers_written: bool,
    /// Whether the request is done
    pub is_done: AtomicBool,
    /// Client connection state at the time is_done was set
    pub client_had_closed: std::sync::atomic::AtomicBool,
    /// When the request started
    pub started_at: Instant,
    /// Handler return value
    pub handler_return: Option<crate::types::PhpValue>,
}

/// Split an HTTP `Host` header into `(name, port)` without losing the name
/// when the port is omitted. IPv6 literals are bracketed in Host headers.
fn split_host_header(host: &str) -> (&str, &str) {
    let host = host.rsplit('@').next().unwrap_or(host); // strip userinfo if any
    if let Some(rest) = host.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            let name = &rest[..end];
            let tail = &rest[end + 1..];
            if let Some(port) = tail.strip_prefix(':') {
                return (name, port);
            }
            return (name, "80");
        }
    }
    match host.rsplit_once(':') {
        Some((name, port)) if !port.is_empty() => (name, port),
        _ => (host, "80"),
    }
}

/// Builder for creating a PhpContext.
#[allow(dead_code)]
pub struct PhpContextBuilder {
    ctx: PhpContext,
}

impl PhpContext {
    /// Create a new context from an HTTP request.
    pub fn from_request<B>(
        request: &hyper::Request<B>,
        server: Arc<crate::server::Server>,
        opts: Vec<Box<dyn RequestOption>>,
    ) -> Result<Self> {
        let req_uri = request.uri().to_string();
        let query_string = req_uri
            .split_once('?')
            .map(|(_, q)| q.to_string())
            .unwrap_or_default();
        let method = request.method().to_string();
        let content_type = request
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let content_length = request
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        // Host may be `example.com`, `example.com:8080`, or `[::1]:8080`.
        // Never fall back to `localhost` just because the port is absent:
        // that would mix virtual hosts / cache keys between connections.
        let (server_name, server_port) = request
            .headers()
            .get("host")
            .and_then(|v| v.to_str().ok())
            .map(split_host_header)
            .unwrap_or(("localhost", "80"));
        let server_name = server_name.to_string();
        let server_port = server_port.to_string();
        let https = if request.uri().scheme().map_or(false, |s| s == "https") {
            "on".to_string()
        } else {
            "off".to_string()
        };

        let mut ctx = PhpContext {
            ctx: tokio::sync::watch::channel(None).1,
            document_root: server.root().to_string(),
            split_path: server.split_path().to_vec(),
            env: PreparedEnv::new(),
            logger: server.logger_level(),
            worker: None,
            server,
            request_body_timeout: None,
            doc_uri: String::new(),
            path_info: String::new(),
            script_name: String::new(),
            script_filename: String::new(),
            request_uri: req_uri,
            query_string,
            request_method: method,
            content_type,
            content_length,
            server_protocol: "HTTP/1.1".to_string(),
            server_name,
            server_port,
            https,
            headers_written: false,
            is_done: AtomicBool::new(false),
            client_had_closed: std::sync::atomic::AtomicBool::new(false),
            started_at: Instant::now(),
            handler_return: None,
        };

        // Forward all request headers as HTTP_* CGI variables so PHP ($_SERVER)
        // and WordPress (cookie/session checks) see them.
        for (k, v) in request.headers() {
            let key = k.as_str().to_string().into_bytes();
            let val = v.to_str().unwrap_or_default().to_string().into_bytes();
            ctx.env.insert(key, val);
        }

        for opt in &opts {
            opt.apply(&mut ctx)?;
        }

        Ok(ctx)
    }

    /// Create a dummy context for worker startup.
    pub fn worker_dummy(worker: &Arc<crate::worker::Worker>) -> Self {
        let server = worker.server().cloned().unwrap_or_else(|| {
            Arc::new(crate::server::Server::fallback())
        });

        PhpContext {
            ctx: tokio::sync::watch::channel(None).1,
            document_root: worker.file_name().to_string(),
            split_path: server.split_path().to_vec(),
            env: PreparedEnv::new(),
            logger: server.logger_level(),
            worker: Some(worker.clone()),
            server,
            request_body_timeout: None,
            doc_uri: String::new(),
            path_info: String::new(),
            script_name: String::new(),
            script_filename: worker.file_name().to_string(),
            request_uri: String::new(),
            query_string: String::new(),
            request_method: String::new(),
            content_type: String::new(),
            content_length: String::new(),
            server_protocol: "HTTP/1.1".to_string(),
            server_name: String::new(),
            server_port: String::new(),
            https: "off".to_string(),
            headers_written: false,
            is_done: AtomicBool::new(false),
            client_had_closed: std::sync::atomic::AtomicBool::new(false),
            started_at: Instant::now(),
            handler_return: None,
        }
    }

    /// Create a context from a message (for external workers).
pub fn from_message(
        message: crate::types::PhpValue,
        server: Arc<crate::server::Server>,
        worker: Arc<crate::worker::Worker>,
    ) -> Self {
        PhpContext {
            ctx: tokio::sync::watch::channel(None).1,
            document_root: server.root().to_string(),
            split_path: server.split_path().to_vec(),
            env: PreparedEnv::new(),
            logger: server.logger_level(),
            worker: Some(worker.clone()),
            server,
            request_body_timeout: None,
            doc_uri: String::new(),
            path_info: String::new(),
            script_name: String::new(),
            script_filename: worker.file_name().to_string(),
            request_uri: String::new(),
            query_string: String::new(),
            request_method: String::new(),
            content_type: String::new(),
            content_length: String::new(),
            server_protocol: "HTTP/1.1".to_string(),
            server_name: String::new(),
            server_port: String::new(),
            https: "off".to_string(),
            headers_written: false,
            is_done: AtomicBool::new(false),
            client_had_closed: std::sync::atomic::AtomicBool::new(false),
            started_at: Instant::now(),
            handler_return: Some(message),
        }
    }

    /// Close the context, signaling that the request is done.
    pub fn close_context(&self) {
        if self.is_done.load(Ordering::Acquire) {
            return;
        }
        self.client_had_closed.store(self.client_has_closed(), Ordering::Release);
        self.is_done.store(true, Ordering::Release);
    }

    /// Check if the client has closed the connection.
    pub fn client_has_closed(&self) -> bool {
        self.ctx.borrow().is_some()
    }

    /// Reject the request with an error.
    pub fn reject(&self, err: Error) {
        if self.is_done.load(Ordering::Acquire) {
            return;
        }
        if let Error::Rejected(_re) = err {
            self.close_context();
        }
    }

    /// Validate the request.
    pub fn validate(&self) -> Result<()> {
        if self.request_uri.contains('\0') {
            return Err(Error::InvalidRequestPath);
        }
        Ok(())
    }

    /// Check if the context is done.
    pub fn is_done(&self) -> bool {
        self.is_done.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::server::Server;

    fn test_server() -> Arc<Server> {
        Arc::new(Server::new("C:\\www").unwrap())
    }

    #[test]
    fn validate_rejects_null_byte() {
        let server = test_server();
        let ctx = PhpContext {
            ctx: tokio::sync::watch::channel(None).1,
            document_root: server.root().to_string(),
            split_path: server.split_path().to_vec(),
            env: PreparedEnv::new(),
            logger: server.logger_level(),
            worker: None,
            server: server.clone(),
            request_body_timeout: None,
            doc_uri: String::new(),
            path_info: String::new(),
            script_name: String::new(),
            script_filename: String::new(),
            request_uri: "/test\0.php".to_string(),
            query_string: String::new(),
            request_method: "GET".to_string(),
            content_type: String::new(),
            content_length: String::new(),
            server_protocol: "HTTP/1.1".to_string(),
            server_name: "localhost".to_string(),
            server_port: "80".to_string(),
            https: "off".to_string(),
            headers_written: false,
            is_done: AtomicBool::new(false),
            client_had_closed: AtomicBool::new(false),
            started_at: Instant::now(),
            handler_return: None,
        };
        assert!(ctx.validate().is_err());
    }

    #[test]
    fn validate_allows_normal_uri() {
        let server = test_server();
        let req = hyper::Request::builder()
            .uri("/index.php")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        assert!(ctx.validate().is_ok());
    }

    #[test]
    fn from_request_extracts_query_string() {
        let server = test_server();
        let req = hyper::Request::builder()
            .uri("/index.php?foo=bar&baz=qux")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        assert_eq!(ctx.query_string, "foo=bar&baz=qux");
        assert_eq!(ctx.request_uri, "/index.php?foo=bar&baz=qux");
    }

    #[test]
    fn from_request_extracts_method() {
        let server = test_server();
        let req = hyper::Request::builder()
            .method("POST")
            .uri("/index.php")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        assert_eq!(ctx.request_method, "POST");
    }

    #[test]
    fn from_request_extracts_content_type() {
        let server = test_server();
        let req = hyper::Request::builder()
            .header("content-type", "application/json")
            .uri("/index.php")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        assert_eq!(ctx.content_type, "application/json");
    }

    #[test]
    fn from_request_extracts_host() {
        let server = test_server();
        let req = hyper::Request::builder()
            .header("host", "example.com:8080")
            .uri("/index.php")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        assert_eq!(ctx.server_name, "example.com");
        assert_eq!(ctx.server_port, "8080");
    }

    #[test]
    fn from_request_defaults_host() {
        let server = test_server();
        let req = hyper::Request::builder()
            .uri("/index.php")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        assert_eq!(ctx.server_name, "localhost");
        assert_eq!(ctx.server_port, "80");
    }

    #[test]
    fn host_without_port_keeps_the_real_hostname() {
        let server = test_server();
        let req = hyper::Request::builder()
            .header("host", "nghesachxua.vn")
            .uri("/index.php")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        assert_eq!(ctx.server_name, "nghesachxua.vn");
        assert_eq!(ctx.server_port, "80");
    }

    #[test]
    fn host_with_ipv6_and_port_is_split_correctly() {
        assert_eq!(split_host_header("[::1]:8443"), ("::1", "8443"));
        assert_eq!(split_host_header("[::1]"), ("::1", "80"));
        assert_eq!(split_host_header("example.com:8080"), ("example.com", "8080"));
        assert_eq!(split_host_header("example.com"), ("example.com", "80"));
    }

    #[test]
    fn from_request_defaults_https_off() {
        let server = test_server();
        let req = hyper::Request::builder()
            .uri("http://example.com/index.php")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        assert_eq!(ctx.https, "off");
    }

    #[test]
    fn from_request_defaults_server_protocol() {
        let server = test_server();
        let req = hyper::Request::builder()
            .uri("/index.php")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        assert_eq!(ctx.server_protocol, "HTTP/1.1");
    }

    #[test]
    fn close_context_marks_done() {
        let server = test_server();
        let req = hyper::Request::builder()
            .uri("/index.php")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        assert!(!ctx.is_done());
        ctx.close_context();
        assert!(ctx.is_done());
    }

    #[test]
    fn close_context_idempotent() {
        let server = test_server();
        let req = hyper::Request::builder()
            .uri("/index.php")
            .body(())
            .unwrap();
        let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();
        ctx.close_context();
        ctx.close_context();
        assert!(ctx.is_done());
    }

    #[test]
    fn concurrent_contexts_keep_isolated_request_state() {
        const N: usize = 16;
        let server = test_server();

        std::thread::scope(|scope| {
            for id in 0..N {
                let server = server.clone();
                scope.spawn(move || {
                    let method = if id % 2 == 0 { "GET" } else { "POST" };
                    let req = hyper::Request::builder()
                        .method(method)
                        .uri(format!("/index.php?req={}", id))
                        .header("host", format!("tenant-{}.example", id))
                        .header("x-request-id", format!("req-{}", id))
                        .header("cookie", format!("sid={}", id))
                        .body(())
                        .unwrap();
                    let ctx = PhpContext::from_request(&req, server, vec![]).unwrap();

                    assert_eq!(ctx.request_method, method);
                    assert_eq!(ctx.request_uri, format!("/index.php?req={}", id));
                    assert_eq!(ctx.query_string, format!("req={}", id));
                    assert_eq!(ctx.server_name, format!("tenant-{}.example", id));
                    assert_eq!(
                        ctx.env.get(&b"x-request-id".to_vec()).unwrap(),
                        format!("req-{}", id).as_bytes()
                    );
                    assert_eq!(
                        ctx.env.get(&b"cookie".to_vec()).unwrap(),
                        format!("sid={}", id).as_bytes()
                    );

                    // No value from a neighbouring request may leak in.
                    let neighbour = format!("req-{}", (id + 1) % N);
                    assert!(
                        !ctx.env.values().any(|value| value == neighbour.as_bytes()),
                        "context {} leaked state from {}",
                        id,
                        (id + 1) % N
                    );
                });
            }
        });
    }

    #[test]
    fn contexts_do_not_share_env_or_done_state() {
        let server = test_server();

        let first_req = hyper::Request::builder()
            .uri("/index.php?who=first")
            .header("x-request-id", "first")
            .body(())
            .unwrap();
        let second_req = hyper::Request::builder()
            .uri("/index.php?who=second")
            .header("x-request-id", "second")
            .body(())
            .unwrap();

        let mut first = PhpContext::from_request(&first_req, server.clone(), vec![]).unwrap();
        let second = PhpContext::from_request(&second_req, server, vec![]).unwrap();

        assert_eq!(first.env.get(&b"x-request-id".to_vec()).unwrap(), b"first");
        assert_eq!(second.env.get(&b"x-request-id".to_vec()).unwrap(), b"second");

        first.env.insert(b"only-first\0".to_vec(), b"yes".to_vec());
        assert!(!second.env.contains_key(&b"only-first\0".to_vec()));

        first.close_context();
        assert!(first.is_done());
        assert!(!second.is_done(), "closing one request must not close another");
    }
}