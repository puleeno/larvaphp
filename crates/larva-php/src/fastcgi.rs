//! FastCGI client for persistent PHP execution.
//!
//! The CGI model spawns a fresh `php-cgi.exe` process for every request
//! (~80 ms of process-creation overhead, no opcache reuse). FastCGI keeps a
//! long-lived `php-cgi -b` process that handles many requests, eliminating the
//! per-request spawn and enabling opcache reuse.
//!
//! This module implements the minimal FastCGI "responder" client:
//! BEGIN_REQUEST -> PARAMS -> STDIN -> read STDOUT/STDERR/END_REQUEST.

#[cfg(feature = "fcgi")]
pub mod imp {
    use std::io::{self, Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::process::{Child, Command};
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::Duration;

    use crate::errors::{Error, Result};

    // FastCGI record types.
    const FCG_BEGIN_REQUEST: u8 = 1;
    const FCG_END_REQUEST: u8 = 3;
    const FCG_PARAMS: u8 = 4;
    const FCG_STDIN: u8 = 5;
    const FCG_STDOUT: u8 = 6;
    const FCG_STDERR: u8 = 7;

    const FCG_VERSION: u8 = 1;
    const FCG_RESPONDER: u16 = 1;
    const MAX_CONTENT: usize = 60_000;

    struct Record {
        type_: u8,
        content: Vec<u8>,
    }

    fn write_record(stream: &mut TcpStream, type_: u8, req_id: u16, content: &[u8]) -> io::Result<()> {
        let mut header = [0u8; 8];
        header[0] = FCG_VERSION;
        header[1] = type_;
        header[2..4].copy_from_slice(&req_id.to_be_bytes());
        header[4..6].copy_from_slice(&(content.len() as u16).to_be_bytes());
        // header[6] = padding length (0), header[7] = reserved (0)
        stream.write_all(&header)?;
        stream.write_all(content)?;
        Ok(())
    }

    fn read_record(stream: &mut TcpStream) -> io::Result<Record> {
        let mut header = [0u8; 8];
        stream.read_exact(&mut header)?;
        let type_ = header[1];
        let content_len = u16::from_be_bytes([header[4], header[5]]) as usize;
        let padding_len = header[6] as usize;
        let mut content = vec![0u8; content_len];
        stream.read_exact(&mut content)?;
        if padding_len > 0 {
            let mut padding = vec![0u8; padding_len];
            stream.read_exact(&mut padding)?;
        }
        Ok(Record { type_, content })
    }

    fn encode_len(out: &mut Vec<u8>, len: usize) {
        if len < 128 {
            out.push(len as u8);
        } else {
            out.extend_from_slice(&((len as u32 | 0x8000_0000)).to_be_bytes());
        }
    }

    fn encode_name_value(out: &mut Vec<u8>, name: &str, value: &str) {
        encode_len(out, name.len());
        encode_len(out, value.len());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(value.as_bytes());
    }

    /// A FastCGI response.
    pub struct FastCgiResponse {
        pub stdout: Vec<u8>,
        pub stderr: Vec<u8>,
    }

    /// Send one FastCGI request on an existing connection and collect the response.
    fn fastcgi_request_on(
        stream: &mut TcpStream,
        req_id: u16,
        params: &[(String, String)],
        body: &[u8],
    ) -> io::Result<FastCgiResponse> {
        // BEGIN_REQUEST (role = RESPONDER, flags = 0)
        let mut begin = Vec::with_capacity(8);
        begin.extend_from_slice(&FCG_RESPONDER.to_be_bytes());
        begin.push(0);
        begin.extend_from_slice(&[0u8; 6]);
        write_record(stream, FCG_BEGIN_REQUEST, req_id, &begin)?;

        // PARAMS
        let mut params_data = Vec::new();
        for (k, v) in params {
            encode_name_value(&mut params_data, k, v);
        }
        for chunk in params_data.chunks(MAX_CONTENT) {
            write_record(stream, FCG_PARAMS, req_id, chunk)?;
        }
        write_record(stream, FCG_PARAMS, req_id, &[])?; // end of PARAMS

        // STDIN
        for chunk in body.chunks(MAX_CONTENT) {
            write_record(stream, FCG_STDIN, req_id, chunk)?;
        }
        write_record(stream, FCG_STDIN, req_id, &[])?; // end of STDIN

        // Read the response.
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        loop {
            let rec = read_record(stream)?;
            match rec.type_ {
                FCG_STDOUT => stdout.extend_from_slice(&rec.content),
                FCG_STDERR => stderr.extend_from_slice(&rec.content),
                FCG_END_REQUEST => break,
                _ => {}
            }
        }

        Ok(FastCgiResponse { stdout, stderr })
    }

    /// Send one FastCGI request to `addr` and collect the response (new connection).
    pub fn fastcgi_request(
        addr: &SocketAddr,
        params: &[(String, String)],
        body: &[u8],
    ) -> io::Result<FastCgiResponse> {
        let mut stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(120)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        stream.set_nodelay(true)?;
        fastcgi_request_on(&mut stream, 1, params, body)
    }

    /// A persistent FastCGI backend (`php-cgi -b`) shared across requests.
    pub struct FastCgiServer {
        child: Mutex<Option<Child>>,
        addr: SocketAddr,
        pool: Mutex<Vec<TcpStream>>,
        req_counter: std::sync::atomic::AtomicU16,
    }

    impl FastCgiServer {
        /// Spawn `php-cgi -b <addr>` and wait until it is accepting connections.
        pub fn start(port: u16) -> Result<Self> {
            let php_exe = crate::server::find_php_cgi()
                .ok_or_else(|| Error::Ffi("php-cgi.exe not found".to_string()))?;
            let addr: SocketAddr = format!("127.0.0.1:{}", port)
                .parse()
                .map_err(|e| Error::Ffi(format!("invalid fastcgi addr: {}", e)))?;

            let mut cmd = Command::new(php_exe);
            cmd.arg("-b")
                .arg(format!("{}:{}", addr.ip(), addr.port()))
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            // Load opcache (with a persistent file cache) into the long-lived
            // backend. Because this process outlives requests, opcache's shared
            // memory stays warm across requests — the closest we get to
            // FrankenPHP's in-process PHP without the embedded FFI path.
            if let Some(dir) = crate::server::write_ini_overrides(&Default::default()) {
                cmd.env("PHP_INI_SCAN_DIR", &dir);
            }
            let child = cmd
                .spawn()
                .map_err(|e| Error::Ffi(format!("failed to spawn php-cgi -b: {}", e)))?;

            // Wait until the port is listening (up to ~5s).
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                if TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok() {
                    break;
                }
                if std::time::Instant::now() > deadline {
                    return Err(Error::Ffi(
                        "php-cgi FastCGI did not start listening in time".to_string(),
                    ));
                }
                std::thread::sleep(Duration::from_millis(100));
            }

            tracing::info!("FastCGI backend listening on {}", addr);

            Ok(Self {
                child: Mutex::new(Some(child)),
                addr,
                pool: Mutex::new(Vec::new()),
                req_counter: std::sync::atomic::AtomicU16::new(1),
            })
        }

        pub fn addr(&self) -> SocketAddr {
            self.addr
        }

        /// Execute a request against the persistent backend, reusing a pooled
        /// connection when possible (avoids a TCP handshake per request).
        pub fn request(
            &self,
            params: &[(String, String)],
            body: &[u8],
        ) -> io::Result<FastCgiResponse> {
            // Reuse an idle connection or establish a new one.
            let mut stream = self.pool.lock().unwrap().pop();
            let mut stream = match stream.take() {
                Some(s) => s,
                None => TcpStream::connect(&self.addr)?,
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
            let _ = stream.set_nodelay(true);

            // Unique request id (wraps at u16; 0 is reserved).
            let mut req_id = self.req_counter.fetch_add(1, Ordering::Relaxed);
            if req_id == 0 {
                req_id = 1;
            }

            match fastcgi_request_on(&mut stream, req_id, params, body) {
                Ok(resp) => {
                    // Connection still healthy: return it to the pool.
                    self.pool.lock().unwrap().push(stream);
                    Ok(resp)
                }
                Err(e) => Err(e), // broken connection is dropped
            }
        }
    }

    impl Drop for FastCgiServer {
        fn drop(&mut self) {
            if let Ok(mut guard) = self.child.lock() {
                if let Some(mut child) = guard.take() {
                    let _ = child.kill();
                }
            }
        }
    }

    /// Lazily-started global FastCGI backend (default port 19000).
    /// Returns `None` if it could not be started (caller falls back to CGI).
    pub fn backend() -> Option<Arc<FastCgiServer>> {
        static BACKEND: OnceLock<Arc<FastCgiServer>> = OnceLock::new();
        if let Some(b) = BACKEND.get() {
            return Some(Arc::clone(b));
        }
        match FastCgiServer::start(19000) {
            Ok(server) => {
                let arc = Arc::new(server);
                let _ = BACKEND.set(Arc::clone(&arc));
                Some(arc)
            }
            Err(e) => {
                tracing::debug!("FastCGI backend unavailable: {}", e);
                None
            }
        }
    }
}

#[cfg(feature = "fcgi")]
pub use imp::*;

#[cfg(not(feature = "fcgi"))]
pub mod imp {
    use std::net::SocketAddr;
    use std::sync::Arc;

    pub struct FastCgiResponse {
        pub stdout: Vec<u8>,
        pub stderr: Vec<u8>,
    }

    pub struct FastCgiServer;

    impl FastCgiServer {
        pub fn start(_port: u16) -> crate::errors::Result<Self> {
            Err(crate::errors::Error::Ffi(
                "FastCGI support is not enabled (build with --features fcgi)".to_string(),
            ))
        }
        pub fn addr(&self) -> SocketAddr {
            "127.0.0.1:0".parse().unwrap()
        }
        pub fn request(
            &self,
            _params: &[(String, String)],
            _body: &[u8],
        ) -> std::io::Result<FastCgiResponse> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "fcgi not enabled",
            ))
        }
    }

    pub fn backend() -> Option<Arc<FastCgiServer>> {
        None
    }
}

#[cfg(not(feature = "fcgi"))]
pub use imp::*;