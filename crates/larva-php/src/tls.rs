//! Direct TLS support and the HTTP-01 challenge/redirect listener.
//!
//! LarvaPHP can terminate TLS itself instead of requiring a reverse proxy.
//! Port 80 is kept for Let's Encrypt HTTP-01 validation and redirects every
//! other request to HTTPS.

use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto;
use tokio::net::TcpListener;

use crate::errors::{Error, Result};

/// Build a rustls server configuration from PEM certificate and key files.
pub fn build_server_config(cert_path: &str, key_path: &str) -> Result<rustls::ServerConfig> {
    let cert_file = File::open(cert_path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("open TLS certificate {}: {}", cert_path, e),
        ))
    })?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs = rustls_pemfile::certs(&mut cert_reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("parse TLS certificate {}: {}", cert_path, e),
            ))
        })?;
    if certs.is_empty() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("no certificates found in {}", cert_path),
        )));
    }

    let key_file = File::open(key_path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("open TLS key {}: {}", key_path, e),
        ))
    })?;
    let mut key_reader = BufReader::new(key_file);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("parse TLS key {}: {}", key_path, e),
            ))
        })?
        .ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("no private key found in {}", key_path),
            ))
        })?;

    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid TLS certificate/key: {}", e),
            ))
        })?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(config)
}

/// Start a plain HTTP listener that serves ACME HTTP-01 tokens and redirects
/// everything else to HTTPS. It runs on its own runtime thread so it is
/// available while the ACME client is obtaining/renewing a certificate.
pub fn spawn_http_challenge_server(addr: SocketAddr, challenge_dir: PathBuf) -> Result<()> {
    std::thread::Builder::new()
        .name("larva-http-01".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("failed to start HTTP-01 runtime: {}", e);
                    return;
                }
            };
            rt.block_on(async move {
                let listener = match TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("failed to bind HTTP-01 listener {}: {}", addr, e);
                        return;
                    }
                };
                tracing::info!("HTTP-01/redirect server listening on http://{}", addr);
                loop {
                    let (stream, _) = match listener.accept().await {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!("HTTP-01 accept error: {}", e);
                            continue;
                        }
                    };
                    let challenge_dir = challenge_dir.clone();
                    tokio::spawn(async move {
                        if let Err(err) =
                            auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                                .serve_connection(TokioIo::new(stream), service_fn(move |req| {
                                    let challenge_dir = challenge_dir.clone();
                                    async move { handle_http_request(req, challenge_dir).await }
                                }))
                                .await
                        {
                            tracing::debug!("HTTP-01 connection error: {}", err);
                        }
                    });
                }
            });
        })
        .map_err(Error::Io)?;
    Ok(())
}

async fn handle_http_request(
    req: Request<hyper::body::Incoming>,
    challenge_dir: PathBuf,
) -> std::result::Result<Response<Full<Bytes>>, hyper::Error> {
    let path = req.uri().path().to_string();
    const PREFIX: &str = "/.well-known/acme-challenge/";
    if path.starts_with(PREFIX) {
        let token = &path[PREFIX.len()..];
        let safe = !token.is_empty()
            && token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if safe {
            let file = challenge_dir.join(token);
            if let Ok(body) = tokio::fs::read(&file).await {
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/plain; charset=utf-8")
                    .header("content-length", body.len().to_string())
                    .header("cache-control", "no-store")
                    .header("server", "LarvaPHP")
                    .body(Full::new(Bytes::from(body)))
                    .unwrap());
            }
        }
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("content-type", "text/plain; charset=utf-8")
            .header("server", "LarvaPHP")
            .body(Full::new(Bytes::from_static(b"Not Found")))
            .unwrap());
    }

    let authority = req
        .headers()
        .get(hyper::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let uri = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or_else(|| req.uri().path())
        .to_string();
    let location = crate::server::canonical_redirect_location("https", authority, &uri)
        .unwrap_or_else(|| format!("https://{}{}", authority, uri));
    Ok(Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header("location", location)
        .header("cache-control", "no-store")
        .header("server", "LarvaPHP")
        .body(Full::new(Bytes::new()))
        .unwrap())
}

/// Ensure a path is inside the configured document root (defence in depth for
/// ACME token lookups).
#[allow(dead_code)]
fn is_safe_child(root: &Path, child: &Path) -> bool {
    child.starts_with(root)
}
