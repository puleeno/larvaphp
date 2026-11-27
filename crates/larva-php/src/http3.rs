//! HTTP/3 (QUIC) server support via [cloudflare/quiche](https://github.com/cloudflare/quiche).
//!
//! This module is compiled only with the `http3` feature. It runs a QUIC/HTTP3
//! listener on a UDP socket, reusing the same request-handling core as the TCP
//! (HTTP/1.1 + HTTP/2) server via [`Server::handle_request_parts`].
//!
//! Design: the TCP server (hyper) and the UDP/QUIC server are two "transports"
//! that both delegate to the same core handler (a Strategy/Adapter split).
//! Protocol selection is automatic:
//!   - Browser with HTTP/3 support  -> QUIC/UDP (ALPN "h3")
//!   - Browser without HTTP/3        -> TCP HTTP/2 (ALPN "h2") or HTTP/1.1
//!
//! The TCP server advertises HTTP/3 via an `Alt-Svc` header so capable browsers
//! upgrade to QUIC while others transparently keep using TCP.

#[cfg(feature = "http3")]
pub mod imp {
    use std::collections::HashMap;
    use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use crossbeam::channel::{self, Receiver, Sender};
    use hyper::StatusCode;
    use quiche::h3;
    use quiche::h3::NameValue;

    use crate::error_page;
    use crate::errors::{Error, Result};
    use crate::server::Server;

    /// QUIC/UDP datagram buffer size.
    const MAX_DATAGRAM_SIZE: usize = 1350;

    /// How often QUIC timers (including the idle timeout) are driven.
    ///
    /// This must happen on a wall-clock cadence, independently of `recv_from()`:
    /// on a busy site datagrams arrive continuously, so waiting for a receive
    /// timeout would never clean up idle connections.
    const TIMER_TICK: Duration = Duration::from_millis(100);

    /// Hard cap on tracked QUIC connections. Also bounds the work done for
    /// every datagram in `send_pending()`.
    const MAX_CLIENTS: usize = 4096;

    /// Max requests waiting to be handed to the PHP/static-file workers.
    const REQUEST_QUEUE_CAP: usize = 256;

    /// Embedded PHP's compiler needs far more stack than Rust's default 2 MiB
    /// worker stack. With the default, PHP aborts every request with
    /// "Maximum call stack size of 2048000 bytes ... Infinite recursion?" even
    /// for a trivial script. Match the 64 MiB stack used by the embedded tests.
    pub(crate) const H3_WORKER_STACK_SIZE: usize = 64 * 1024 * 1024;

    /// Minimal headers for HTTP/3 errors produced before the shared request
    /// core runs. `content-length` is added by `send_h3_response`.
    fn direct_error_headers() -> Vec<(String, String)> {
        vec![
            ("content-type".to_string(), "text/html; charset=UTF-8".to_string()),
            ("cache-control".to_string(), "no-store".to_string()),
            ("server".to_string(), "LarvaPHP".to_string()),
            ("x-content-type-options".to_string(), "nosniff".to_string()),
            ("x-frame-options".to_string(), "DENY".to_string()),
            ("referrer-policy".to_string(), "no-referrer".to_string()),
        ]
    }

    /// A response that could not be fully written in one pass (flow control).
    struct PartialResponse {
        headers: Option<Vec<h3::Header>>,
        body: Vec<u8>,
        written: usize,
    }

    /// A parsed HTTP/3 request passed to a worker thread. `client_key` is the
    /// destination connection ID, so the response can be sent back on the
    /// same QUIC connection by the listener thread.
    struct PendingRequest {
        client_key: Vec<u8>,
        stream_id: u64,
        method: String,
        path: String,
        headers: Vec<(String, String)>,
    }

    /// Convert HTTP/3 header blocks into the `(method, path, headers)` tuple
    /// consumed by `Server::handle_request_parts`.
    ///
    /// Browsers normally send `:authority` but no `host` field. The TCP path
    /// gets that mapping from Hyper for free; the QUIC path must do it here,
    /// otherwise PHP sees `SERVER_NAME=localhost` and WordPress can enter a
    /// redirect/bootstrap loop that surfaces as a 500.
    pub(crate) fn h3_request_parts(
        headers: &[h3::Header],
    ) -> (String, String, Vec<(String, String)>) {
        let mut method = String::from("GET");
        let mut path = String::from("/");
        let mut authority = String::new();
        let mut req_headers: Vec<(String, String)> = Vec::new();

        for h in headers {
            match h.name() {
                b":method" => method = String::from_utf8_lossy(h.value()).to_string(),
                b":path" => path = String::from_utf8_lossy(h.value()).to_string(),
                b":authority" => {
                    authority = String::from_utf8_lossy(h.value()).to_string();
                }
                b":scheme" | b":status" => {}
                name => {
                    req_headers.push((
                        String::from_utf8_lossy(name).to_string(),
                        String::from_utf8_lossy(h.value()).to_string(),
                    ));
                }
            }
        }

        // Explicit `host` wins; otherwise synthesise it from `:authority`.
        if !authority.is_empty()
            && !req_headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("host"))
        {
            req_headers.push(("host".to_string(), authority));
        }

        (method, path, req_headers)
    }

    /// A finished request waiting to be written by the listener thread.
    struct CompletedRequest {
        client_key: Vec<u8>,
        stream_id: u64,
        result: std::result::Result<(StatusCode, Vec<(String, String)>, Vec<u8>), String>,
    }

    /// Per-connection state.
    struct Client {
        conn: quiche::Connection,
        http3_conn: Option<h3::Connection>,
        partial_responses: HashMap<u64, PartialResponse>,
    }

    /// HTTP/3 (QUIC) server.
    ///
    /// Holds the shared [`Server`] and delegates every HTTP/3 request to
    /// [`Server::handle_request_parts`], so behavior is identical to HTTP/1.1
    /// and HTTP/2.
    pub struct Http3Server {
        addr: String,
        cert_path: String,
        key_path: String,
        server: Arc<Server>,
    }

    impl Http3Server {
        pub fn new(
            addr: impl Into<String>,
            cert_path: impl Into<String>,
            key_path: impl Into<String>,
            server: Arc<Server>,
        ) -> Self {
            Self {
                addr: addr.into(),
                cert_path: cert_path.into(),
                key_path: key_path.into(),
                server,
            }
        }

        /// Run the HTTP/3 server (blocking). Intended to run on a dedicated
        /// thread. Returns only on a fatal socket error.
        pub fn run(&self) -> Result<()> {
            let local: SocketAddr = self
                .addr
                .to_socket_addrs()?
                .next()
                .ok_or_else(|| Error::Ffi("invalid http3 address".to_string()))?;

            let socket = UdpSocket::bind(local)?;
            socket.set_read_timeout(Some(Duration::from_millis(100)))?;

            let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)
                .map_err(|e| Error::Ffi(e.to_string()))?;
            config
                .load_cert_chain_from_pem_file(&self.cert_path)
                .map_err(|e| Error::Ffi(format!("load cert chain: {}", e)))?;
            config
                .load_priv_key_from_pem_file(&self.key_path)
                .map_err(|e| Error::Ffi(format!("load private key: {}", e)))?;
            config
                .set_application_protos(h3::APPLICATION_PROTOCOL)
                .map_err(|e| Error::Ffi(e.to_string()))?;
            config.set_max_idle_timeout(30_000);
            config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
            config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
            config.set_initial_max_data(10_000_000);
            config.set_initial_max_stream_data_bidi_local(1_000_000);
            config.set_initial_max_stream_data_bidi_remote(1_000_000);
            config.set_initial_max_stream_data_uni(1_000_000);
            config.set_initial_max_streams_bidi(100);
            config.set_initial_max_streams_uni(100);
            config.set_disable_active_migration(true);

            let h3_config =
                h3::Config::new().map_err(|e| Error::Ffi(format!("h3 config: {}", e)))?;

            // PHP/static-file work must not block the QUIC event loop: a single
            // slow request would otherwise stall every other connection and
            // delay QUIC timers. Workers hand finished responses back through
            // `result_rx`; the listener thread remains the only thread that
            // touches quiche connections.
            let (job_tx, job_rx) = channel::bounded::<PendingRequest>(REQUEST_QUEUE_CAP);
            let (result_tx, result_rx) = channel::unbounded::<CompletedRequest>();
            spawn_request_workers(
                self.server.clone(),
                job_rx,
                result_tx,
                num_cpus::get().clamp(2, 8),
            );

            let mut clients: HashMap<Vec<u8>, Client> = HashMap::new();
            let mut buf = [0u8; 65535];
            let mut out = [0u8; MAX_DATAGRAM_SIZE];
            let mut last_timer_tick = Instant::now();

            loop {
                // Drive QUIC timers on a wall-clock cadence. Doing this only
                // when `recv_from` times out means a busy site never expires
                // idle connections: the `clients` map grows for hours and
                // `send_pending()` gets slower with every request.
                if last_timer_tick.elapsed() >= TIMER_TICK {
                    for c in clients.values_mut() {
                        c.conn.on_timeout();
                    }
                    last_timer_tick = Instant::now();
                }

                // A worker may finish while we were blocked in recv_from() on a
                // previous iteration. Move the result into the QUIC stream and
                // flush it BEFORE waiting again: the client is waiting for the
                // reply and will not send another datagram on its own.
                drain_completed_results(&mut clients, &result_rx);
                flush_writable_streams(&mut clients);
                send_pending(&socket, &mut clients, &mut out);

                let (len, from) = match socket.recv_from(&mut buf) {
                    Ok(v) => v,
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        // Timeout is NOT an idle moment: finished jobs and
                        // flow-control-blocked responses must still be written.
                        // Returning to recv_from() without draining here was
                        // the deadlock that made browsers wait minutes.
                        drain_completed_results(&mut clients, &result_rx);
                        flush_writable_streams(&mut clients);
                        send_pending(&socket, &mut clients, &mut out);
                        continue;
                    }
                    Err(e) => return Err(Error::Io(e)),
                };

                // For long headers (Initial/Handshake) the DCID length is in
                // the packet, so MAX_CONN_ID_LEN is safe. For short headers
                // (1-RTT requests) quiche needs the *local* connection-ID
                // length up front. We accept with the client's Initial DCID
                // as our SCID, which is often 8 or 16 bytes; parsing those
                // packets with MAX_CONN_ID_LEN silently failed and dropped
                // every HTTP/3 request after the handshake. Try each legal
                // length and keep the DCID that belongs to a known client.
                let hdr = if buf[0] & 0x80 == 0 {
                    let mut parsed = None;
                    for dcid_len in 0..=quiche::MAX_CONN_ID_LEN {
                        if let Ok(h) = quiche::Header::from_slice(&mut buf[..len], dcid_len) {
                            if clients.contains_key(&h.dcid.to_vec()) {
                                parsed = Some(h);
                                break;
                            }
                        }
                    }
                    match parsed {
                        Some(h) => h,
                        None => continue,
                    }
                } else {
                    match quiche::Header::from_slice(&mut buf[..len], quiche::MAX_CONN_ID_LEN) {
                        Ok(h) => h,
                        Err(_) => continue,
                    }
                };

                let key = hdr.dcid.to_vec();

                // Find or create the connection. Version negotiation is only
                // valid for a NEW Initial connection. Short-header (1-RTT)
                // packets deliberately report `version: 0` in quiche's parsed
                // header; treating those as "unsupported version" would drop
                // every HTTP/3 request after the handshake.
                if !clients.contains_key(&key) {
                    if hdr.ty != quiche::Type::Initial {
                        continue;
                    }
                    if !quiche::version_is_supported(hdr.version) {
                        if let Ok(n) = quiche::negotiate_version(&hdr.scid, &hdr.dcid, &mut out) {
                            let _ = socket.send_to(&out[..n], from);
                        }
                        continue;
                    }
                    if clients.len() >= MAX_CLIENTS {
                        tracing::warn!(
                            "HTTP/3 connection limit ({}) reached, dropping new connection",
                            MAX_CLIENTS
                        );
                        continue;
                    }
                    let scid = hdr.dcid.clone();
                    let conn = quiche::accept(&scid, None, local, from, &mut config)
                        .map_err(|e| Error::Ffi(format!("quiche accept: {}", e)))?;
                    clients.insert(
                        scid.to_vec(),
                        Client {
                            conn,
                            http3_conn: None,
                            partial_responses: HashMap::new(),
                        },
                    );
                }

                let client = clients.get_mut(&key).unwrap();

                let recv_info = quiche::RecvInfo { to: local, from };
                if client.conn.recv(&mut buf[..len], recv_info).is_err() {
                    continue;
                }

                // Establish the HTTP/3 layer once QUIC is up.
                if (client.conn.is_established() || client.conn.is_in_early_data())
                    && client.http3_conn.is_none()
                {
                    if let Ok(h3c) = h3::Connection::with_transport(&mut client.conn, &h3_config) {
                        client.http3_conn = Some(h3c);
                    }
                }

                // Flush any flow-control-blocked responses.
                if client.http3_conn.is_some() {
                    for stream_id in client.conn.writable() {
                        handle_writable(client, stream_id);
                    }
                }

                // Process HTTP/3 events.
                if let Some(h3c) = client.http3_conn.as_mut() {
                    loop {
                        match h3c.poll(&mut client.conn) {
                            Ok((stream_id, h3::Event::Headers { list, .. })) => {
                                handle_request(
                                    &job_tx,
                                    &mut client.conn,
                                    h3c,
                                    &key,
                                    stream_id,
                                    &list,
                                    &mut client.partial_responses,
                                );
                            }
                            Ok((_, h3::Event::Data))
                            | Ok((_, h3::Event::Finished))
                            | Ok((_, h3::Event::Reset { .. }))
                            | Ok((_, h3::Event::PriorityUpdate))
                            | Ok((_, h3::Event::GoAway)) => {}
                            Err(h3::Error::Done) => break,
                            Err(_) => break,
                        }
                    }
                }

                // Write responses produced by the worker threads. quiche
                // connections are only touched by this listener thread, so
                // `send_response` stays race-free.
                drain_completed_results(&mut clients, &result_rx);

                send_pending(&socket, &mut clients, &mut out);
            }
        }
    }

    /// Flush pending QUIC packets for all connections and garbage-collect closed ones.
    fn send_pending(
        socket: &UdpSocket,
        clients: &mut HashMap<Vec<u8>, Client>,
        out: &mut [u8],
    ) {
        for client in clients.values_mut() {
            loop {
                match client.conn.send(out) {
                    Ok((write, send_info)) => {
                        if socket.send_to(&out[..write], send_info.to).is_err() {
                            break;
                        }
                    }
                    Err(quiche::Error::Done) => break,
                    Err(_) => {
                        client.conn.close(false, 0x1, b"fail").ok();
                        break;
                    }
                }
            }
        }
        clients.retain(|_, c| !c.conn.is_closed());
    }

    /// Move every finished worker response into its QUIC stream.
    ///
    /// This runs on EVERY iteration of the listener loop, including the
    /// socket-timeout branch. Waiting for an inbound datagram before draining
    /// deadlocks: the client is waiting for the reply and sends nothing.
    fn drain_completed_results(
        clients: &mut HashMap<Vec<u8>, Client>,
        result_rx: &Receiver<CompletedRequest>,
    ) {
        while let Ok(done) = result_rx.try_recv() {
            let Some(client) = clients.get_mut(&done.client_key) else {
                tracing::warn!(
                    "HTTP/3 response dropped: connection {:?} is no longer tracked",
                    done.client_key
                );
                continue;
            };
            let Some(h3c) = client.http3_conn.as_mut() else {
                tracing::warn!("HTTP/3 response dropped: h3 layer is not ready");
                continue;
            };
            match done.result {
                Ok((status, headers, body)) => send_h3_response(
                    &mut client.conn,
                    h3c,
                    done.stream_id,
                    status,
                    &headers,
                    &body,
                    &mut client.partial_responses,
                ),
                Err(e) => {
                    tracing::warn!("HTTP/3 request failed: {}", e);
                    let body = error_page::html(StatusCode::INTERNAL_SERVER_ERROR);
                    send_h3_response(
                        &mut client.conn,
                        h3c,
                        done.stream_id,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &direct_error_headers(),
                        &body,
                        &mut client.partial_responses,
                    );
                }
            }
        }
    }

    /// Resume responses blocked by QUIC flow control on every loop iteration,
    /// not only after a datagram arrives.
    fn flush_writable_streams(clients: &mut HashMap<Vec<u8>, Client>) {
        for client in clients.values_mut() {
            for stream_id in client.conn.writable() {
                handle_writable(client, stream_id);
            }
        }
    }

    /// Spawn the threads that run the shared request core (PHP + static files).
    fn spawn_request_workers(
        server: Arc<Server>,
        job_rx: Receiver<PendingRequest>,
        result_tx: Sender<CompletedRequest>,
        count: usize,
    ) {
        for id in 0..count {
            let server = Arc::clone(&server);
            let rx = job_rx.clone();
            let tx = result_tx.clone();
            std::thread::Builder::new()
                .name(format!("larva-h3-worker-{id}"))
                .stack_size(H3_WORKER_STACK_SIZE)
                .spawn(move || {
                    while let Ok(job) = rx.recv() {
                        let result = server
                            .handle_request_parts(&job.method, &job.path, &job.headers, &[])
                            .map_err(|e| e.to_string());
                        let _ = tx.send(CompletedRequest {
                            client_key: job.client_key,
                            stream_id: job.stream_id,
                            result,
                        });
                    }
                })
                .expect("spawn HTTP/3 worker thread");
        }
    }

    /// Parse an incoming request and hand it to a worker thread. Keeping the
    /// listener free is essential: executing PHP inline here serialised every
    /// HTTP/3 request (and stalled QUIC timers) for the whole process.
    fn handle_request(
        job_tx: &Sender<PendingRequest>,
        conn: &mut quiche::Connection,
        http3_conn: &mut h3::Connection,
        client_key: &[u8],
        stream_id: u64,
        headers: &[h3::Header],
        partial_responses: &mut HashMap<u64, PartialResponse>,
    ) {
        // Stop reading the request stream; we decide the response from headers.
        let _ = conn.stream_shutdown(stream_id, quiche::Shutdown::Read, 0);

        let (method, path, req_headers) = h3_request_parts(headers);

        let job = PendingRequest {
            client_key: client_key.to_vec(),
            stream_id,
            method,
            path,
            headers: req_headers,
        };
        if job_tx.try_send(job).is_err() {
            tracing::warn!("HTTP/3 request queue full, returning 503");
            let body = error_page::html(StatusCode::SERVICE_UNAVAILABLE);
            send_h3_response(
                conn,
                http3_conn,
                stream_id,
                StatusCode::SERVICE_UNAVAILABLE,
                &direct_error_headers(),
                &body,
                partial_responses,
            );
        }
    }

    /// Write a finished response on the listener thread, handling flow control.
    fn send_h3_response(
        conn: &mut quiche::Connection,
        http3_conn: &mut h3::Connection,
        stream_id: u64,
        status: StatusCode,
        resp_headers: &[(String, String)],
        body: &[u8],
        partial_responses: &mut HashMap<u64, PartialResponse>,
    ) {
        // Build HTTP/3 response headers. Preserve an explicit content-length
        // (notably the error page's length on HEAD requests); only synthesize
        // one when the shared core did not provide it.
        let mut out_headers = vec![h3::Header::new(
            b":status",
            status.as_u16().to_string().as_bytes(),
        )];
        let mut has_content_length = false;
        for (name, value) in resp_headers {
            if name.eq_ignore_ascii_case("content-length") {
                has_content_length = true;
                out_headers.push(h3::Header::new(name.as_bytes(), value.as_bytes()));
                continue;
            }
            if name.eq_ignore_ascii_case("connection")
                || name.eq_ignore_ascii_case("transfer-encoding")
            {
                continue;
            }
            out_headers.push(h3::Header::new(name.as_bytes(), value.as_bytes()));
        }
        let content_length = body.len().to_string();
        if !has_content_length {
            out_headers.push(h3::Header::new(b"content-length", content_length.as_bytes()));
        }

        // Send headers, handling flow control.
        match http3_conn.send_response(conn, stream_id, &out_headers, false) {
            Ok(_) => {}
            Err(h3::Error::StreamBlocked) => {
                partial_responses.insert(
                    stream_id,
                    PartialResponse {
                        headers: Some(out_headers),
                        body: body.to_vec(),
                        written: 0,
                    },
                );
                return;
            }
            Err(_) => return,
        }

        let written = match http3_conn.send_body(conn, stream_id, body, true) {
            Ok(v) => v,
            Err(h3::Error::Done) => 0,
            Err(_) => return,
        };

        if written < body.len() {
            partial_responses.insert(
                stream_id,
                PartialResponse {
                    headers: None,
                    body: body.to_vec(),
                    written,
                },
            );
        }
    }

    /// Continue writing a flow-control-blocked response on a writable stream.
    fn handle_writable(client: &mut Client, stream_id: u64) {
        if !client.partial_responses.contains_key(&stream_id) {
            return;
        }

        let conn = &mut client.conn;
        let http3_conn = client.http3_conn.as_mut().unwrap();
        let resp = client.partial_responses.get_mut(&stream_id).unwrap();

        if let Some(headers) = resp.headers.take() {
            if http3_conn.send_response(conn, stream_id, &headers, false).is_err() {
                resp.headers = Some(headers);
                return;
            }
        }

        let body = &resp.body[resp.written..];
        match http3_conn.send_body(conn, stream_id, body, true) {
            Ok(written) => resp.written += written,
            Err(h3::Error::Done) => {}
            Err(_) => {
                client.partial_responses.remove(&stream_id);
                return;
            }
        }

        if resp.written == resp.body.len() {
            client.partial_responses.remove(&stream_id);
        }
    }

    /// Generate a self-signed certificate/key pair if the given paths do not exist.
    /// Returns `(cert_path, key_path)` ready for quiche.
    pub fn ensure_cert(cert_path: &str, key_path: &str) -> Result<(String, String)> {
        use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};

        if std::path::Path::new(cert_path).exists() && std::path::Path::new(key_path).exists() {
            return Ok((cert_path.to_string(), key_path.to_string()));
        }

        let mut params = CertificateParams::new(vec!["localhost".to_string()])
            .map_err(|e| Error::Ffi(format!("cert params: {}", e)))?;
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, "localhost");

        let key_pair = KeyPair::generate().map_err(|e| Error::Ffi(format!("keygen: {}", e)))?;
        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| Error::Ffi(format!("self_signed: {}", e)))?;

        std::fs::write(cert_path, cert.pem())
            .map_err(|e| Error::Ffi(format!("write cert: {}", e)))?;
        std::fs::write(key_path, key_pair.serialize_pem())
            .map_err(|e| Error::Ffi(format!("write key: {}", e)))?;

        Ok((cert_path.to_string(), key_path.to_string()))
    }
}

#[cfg(feature = "http3")]
pub use imp::*;

#[cfg(not(feature = "http3"))]
pub mod imp {
    use std::sync::Arc;
    use crate::server::Server;

    /// Stub HTTP/3 server used when the `http3` feature is disabled.
    pub struct Http3Server;

    impl Http3Server {
        pub fn new(
            _addr: impl Into<String>,
            _cert_path: impl Into<String>,
            _key_path: impl Into<String>,
            _server: Arc<Server>,
        ) -> Self {
            Http3Server
        }

        pub fn run(&self) -> crate::errors::Result<()> {
            Err(crate::errors::Error::Ffi(
                "HTTP/3 support is not enabled (build with --features http3)".to_string(),
            ))
        }
    }

    /// Stub cert helper (no-op when disabled).
    pub fn ensure_cert(cert_path: &str, key_path: &str) -> crate::errors::Result<(String, String)> {
        Ok((cert_path.to_string(), key_path.to_string()))
    }
}

#[cfg(not(feature = "http3"))]
pub use imp::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "http3")]
    use super::imp::{h3_request_parts, H3_WORKER_STACK_SIZE};
    use crate::server::Server;
    #[cfg(feature = "http3")]
    use quiche::h3;
    use std::sync::Arc;

    fn test_server() -> Arc<Server> {
        Arc::new(Server::new(".").unwrap())
    }

    #[cfg(not(feature = "http3"))]
    #[test]
    fn stub_new_does_not_panic() {
        let h3 = Http3Server::new("127.0.0.1:0", "cert.pem", "key.pem", test_server());
        // Constructing the stub is safe.
        let _ = h3;
    }

    #[cfg(not(feature = "http3"))]
    #[test]
    fn stub_run_returns_error() {
        let h3 = Http3Server::new("127.0.0.1:0", "cert.pem", "key.pem", test_server());
        let err = h3.run().unwrap_err();
        assert!(err.to_string().contains("not enabled"));
    }

    #[cfg(not(feature = "http3"))]
    #[test]
    fn stub_ensure_cert_passthrough() {
        let (c, k) = ensure_cert("/tmp/c.pem", "/tmp/k.pem").unwrap();
        assert_eq!(c, "/tmp/c.pem");
        assert_eq!(k, "/tmp/k.pem");
    }

    #[cfg(feature = "http3")]
    #[test]
    fn ensure_cert_generates_files() {
        let dir = std::env::temp_dir().join(format!("larva-cert-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cert = dir.join("c.pem").to_string_lossy().to_string();
        let key = dir.join("k.pem").to_string_lossy().to_string();
        let _ = std::fs::remove_file(&cert);
        let _ = std::fs::remove_file(&key);

        let (c, k) = ensure_cert(&cert, &key).unwrap();
        assert!(std::path::Path::new(&c).exists(), "cert file should exist");
        assert!(std::path::Path::new(&k).exists(), "key file should exist");
        assert!(std::fs::metadata(&c).unwrap().len() > 0, "cert non-empty");
        assert!(std::fs::metadata(&k).unwrap().len() > 0, "key non-empty");

        // Idempotent: second call reuses existing files.
        let (c2, _) = ensure_cert(&cert, &key).unwrap();
        assert_eq!(c, c2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "http3")]
    #[test]
    fn run_with_missing_cert_errors() {
        // run() must return Err (not panic) when the cert/key files are absent.
        let h3 = Http3Server::new(
            "127.0.0.1:0",
            "nonexistent-cert.pem",
            "nonexistent-key.pem",
            test_server(),
        );
        assert!(h3.run().is_err());
    }

    #[cfg(feature = "http3")]
    #[test]
    fn h3_request_parts_maps_authority_to_host() {
        let headers = vec![
            h3::Header::new(b":method", b"GET"),
            h3::Header::new(b":scheme", b"https"),
            h3::Header::new(b":authority", b"nghesachxua.vn"),
            h3::Header::new(b":path", b"/"),
            h3::Header::new(b"user-agent", b"Waterfox"),
        ];

        let (method, path, headers) = h3_request_parts(&headers);
        assert_eq!(method, "GET");
        assert_eq!(path, "/");
        assert!(headers
            .iter()
            .any(|(name, value)| name == "host" && value == "nghesachxua.vn"));
        assert!(headers
            .iter()
            .any(|(name, value)| name == "user-agent" && value == "Waterfox"));
    }

    #[cfg(feature = "http3")]
    #[test]
    fn h3_request_parts_keeps_explicit_host() {
        let headers = vec![
            h3::Header::new(b":method", b"GET"),
            h3::Header::new(b":authority", b"ignored.example"),
            h3::Header::new(b":path", b"/"),
            h3::Header::new(b"host", b"nghesachxua.vn"),
        ];

        let (_, _, headers) = h3_request_parts(&headers);
        let hosts: Vec<&String> = headers
            .iter()
            .filter_map(|(name, value)| name.eq_ignore_ascii_case("host").then_some(value))
            .collect();
        assert_eq!(hosts, vec![&"nghesachxua.vn".to_string()]);
    }

    #[cfg(feature = "http3")]
    #[test]
    fn h3_worker_stack_is_large_enough_for_php() {
        assert!(H3_WORKER_STACK_SIZE >= 16 * 1024 * 1024);

        // Exercise a real thread with the production stack size, not just the
        // constant, so a platform that ignores/clamps it is caught here.
        let parts = std::thread::Builder::new()
            .stack_size(H3_WORKER_STACK_SIZE)
            .spawn(|| {
                let headers = [
                    h3::Header::new(b":method", b"GET"),
                    h3::Header::new(b":authority", b"nghesachxua.vn"),
                    h3::Header::new(b":path", b"/probe.php"),
                ];
                h3_request_parts(&headers)
            })
            .unwrap()
            .join()
            .unwrap();

        assert_eq!(parts.0, "GET");
        assert_eq!(parts.1, "/probe.php");
    }

    #[cfg(feature = "http3")]
    #[test]
    fn h3_request_returns_its_own_static_body() {
        use quiche::h3;
        use quiche::h3::NameValue;
        use std::net::UdpSocket;
        use std::time::{Duration, Instant};

        let dir = std::env::temp_dir().join(format!("larva-h3-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hello.txt"), b"hello-h3").unwrap();

        let cert = dir.join("c.pem").to_string_lossy().to_string();
        let key = dir.join("k.pem").to_string_lossy().to_string();
        let (cert, key) = ensure_cert(&cert, &key).unwrap();

        // Reserve a port, then let the server bind it.
        let probe = UdpSocket::bind("127.0.0.1:0").unwrap();
        let server_addr = probe.local_addr().unwrap();
        drop(probe);

        let server = Arc::new(Server::new(&dir.to_string_lossy()).unwrap());
        let h3 = Http3Server::new(server_addr.to_string(), cert, key, server);
        let server_thread = std::thread::spawn(move || h3.run());

        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        let local = socket.local_addr().unwrap();

        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
        config
            .set_application_protos(h3::APPLICATION_PROTOCOL)
            .unwrap();
        config.verify_peer(false); // self-signed test certificate
        config.set_max_idle_timeout(5_000);
        config.set_initial_max_data(10_000_000);
        config.set_initial_max_stream_data_bidi_local(1_000_000);
        config.set_initial_max_stream_data_bidi_remote(1_000_000);
        config.set_initial_max_stream_data_uni(1_000_000);
        config.set_initial_max_streams_bidi(100);
        config.set_initial_max_streams_uni(100);

        let mut conn = quiche::connect(
            Some("localhost"),
            &quiche::ConnectionId::from_ref(&[0xba; 16]),
            local,
            server_addr,
            &mut config,
        )
        .unwrap();
        let h3_config = h3::Config::new().unwrap();
        let mut h3_conn: Option<h3::Connection> = None;

        let mut out = [0u8; 65535];
        let mut buf = [0u8; 65535];
        let mut request_sent = false;
        let mut status = None;
        let mut body = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);

        while Instant::now() < deadline {
            loop {
                match conn.send(&mut out) {
                    Ok((n, info)) => {
                        socket.send_to(&out[..n], info.to).unwrap();
                    }
                    Err(quiche::Error::Done) => break,
                    Err(e) => panic!("quiche send failed: {}", e),
                }
            }

            match socket.recv_from(&mut buf) {
                Ok((n, from)) => {
                    conn.recv(&mut buf[..n], quiche::RecvInfo { to: local, from })
                        .unwrap();
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => panic!("udp recv failed: {}", e),
            }

            if (conn.is_established() || conn.is_in_early_data()) && h3_conn.is_none() {
                h3_conn = Some(h3::Connection::with_transport(&mut conn, &h3_config).unwrap());
            }

            if let Some(h3c) = h3_conn.as_mut() {
                if !request_sent {
                    let headers = vec![
                        h3::Header::new(b":method", b"GET"),
                        h3::Header::new(b":scheme", b"https"),
                        h3::Header::new(b":authority", b"localhost"),
                        h3::Header::new(b":path", b"/hello.txt"),
                        h3::Header::new(b"host", b"localhost"),
                    ];
                    let stream = h3c.send_request(&mut conn, &headers, true).unwrap();
                    assert_eq!(stream, 0);
                    request_sent = true;
                }

                loop {
                    match h3c.poll(&mut conn) {
                        Ok((_, h3::Event::Headers { list, .. })) => {
                            for header in list {
                                if header.name() == b":status" {
                                    status = Some(
                                        String::from_utf8_lossy(header.value())
                                            .parse::<u16>()
                                            .unwrap(),
                                    );
                                }
                            }
                        }
                        Ok((stream, h3::Event::Data)) => loop {
                            let mut chunk = [0u8; 4096];
                            match h3c.recv_body(&mut conn, stream, &mut chunk) {
                                Ok(n) => body.extend_from_slice(&chunk[..n]),
                                Err(h3::Error::Done) => break,
                                Err(e) => panic!("h3 recv_body failed: {}", e),
                            }
                        },
                        Ok((_, h3::Event::Finished))
                        | Ok((_, h3::Event::Reset { .. }))
                        | Ok((_, h3::Event::PriorityUpdate))
                        | Ok((_, h3::Event::GoAway)) => {}
                        Err(h3::Error::Done) => break,
                        Err(e) => panic!("h3 poll failed: {}", e),
                    }
                }
            }

            if status == Some(200) && body == b"hello-h3" {
                break;
            }
        }

        assert_eq!(status, Some(200), "HTTP/3 request never got a response");
        assert_eq!(body, b"hello-h3", "HTTP/3 response body was wrong");
        // The listener loop intentionally has no shutdown signal; leave the
        // thread detached so the test harness can finish.
        drop(server_thread);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
