//! LiteSpeed-style listeners: a Listener binds a port/protocol and serves a set
//! of virtual hosts. Design: Strategy (protocol) + Adapter (socket->HTTP) +
//! Registry (listener management).

use crate::server::{Server, VHostRouter};
use std::sync::{Arc, RwLock};

/// Protocol a listener speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Http1,
    Http2,
    Http3,
    Https,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http1 => "http1",
            Self::Http2 => "http2",
            Self::Http3 => "http3",
            Self::Https => "https",
        }
    }
}

/// A virtual host binding (alias for Server).
pub type VHost = Arc<Server>;

/// Adapter: turns a raw accepted connection into an HTTP request handler.
pub trait ProtocolAdapter: Send + Sync {
    fn protocol(&self) -> Protocol;
    fn name(&self) -> &'static str;
}

/// A listener binds an address and serves vhosts.
pub trait Listener: Send + Sync {
    fn id(&self) -> &str;
    fn address(&self) -> &str;
    fn protocol(&self) -> Protocol;
    fn vhosts(&self) -> Vec<VHost>;
    fn add_vhost(&self, vhost: VHost);
    fn remove_vhost(&self, domain: &str);
    /// Run this listener (binds the port and serves). Returns when stopped.
    fn run(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::errors::Result<()>> + Send + '_>>;
}

/// Registry of listeners.
pub trait ListenerRegistry: Send + Sync {
    fn register(&self, listener: Arc<dyn Listener>);
    fn unregister(&self, id: &str);
    fn get(&self, id: &str) -> Option<Arc<dyn Listener>>;
    fn list(&self) -> Vec<Arc<dyn Listener>>;
    /// Run all registered listeners concurrently.
    fn run_all(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::errors::Result<()>> + Send + '_>>;
}

// ============================================================================
// Concrete implementations
// ============================================================================

/// In-memory listener registry.
pub struct InMemoryListenerRegistry {
    listeners: RwLock<Vec<Arc<dyn Listener>>>,
}

impl InMemoryListenerRegistry {
    pub fn new() -> Self {
        Self { listeners: RwLock::new(Vec::new()) }
    }
}

impl Default for InMemoryListenerRegistry {
    fn default() -> Self { Self::new() }
}

impl ListenerRegistry for InMemoryListenerRegistry {
    fn register(&self, listener: Arc<dyn Listener>) {
        self.listeners.write().unwrap().push(listener);
    }
    fn unregister(&self, id: &str) {
        self.listeners.write().unwrap().retain(|l| l.id() != id);
    }
    fn get(&self, id: &str) -> Option<Arc<dyn Listener>> {
        self.listeners.read().unwrap().iter().find(|l| l.id() == id).cloned()
    }
    fn list(&self) -> Vec<Arc<dyn Listener>> {
        self.listeners.read().unwrap().clone()
    }
    fn run_all(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::errors::Result<()>> + Send + '_>> {
        let listeners = self.list();
        Box::pin(async move {
            let mut handles = Vec::new();
            for l in listeners {
                let l = l.clone();
                handles.push(tokio::spawn(async move { l.run().await }));
            }
            for h in handles {
                h.await.map_err(|e| crate::errors::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))??;
            }
            Ok(())
        })
    }
}

/// A concrete HTTP listener that delegates to the existing multi-vhost serve flow.
pub struct HttpListener {
    id: String,
    address: String,
    protocol: Protocol,
    vhosts: RwLock<Vec<VHost>>,
}

impl HttpListener {
    pub fn new(id: impl Into<String>, address: impl Into<String>, protocol: Protocol) -> Self {
        Self {
            id: id.into(),
            address: address.into(),
            protocol,
            vhosts: RwLock::new(Vec::new()),
        }
    }
    pub fn router(&self) -> Arc<VHostRouter> {
        let vhs = self.vhosts.read().unwrap().clone();
        Arc::new(VHostRouter::new(vhs))
    }
}

impl Listener for HttpListener {
    fn id(&self) -> &str { &self.id }
    fn address(&self) -> &str { &self.address }
    fn protocol(&self) -> Protocol { self.protocol }
    fn vhosts(&self) -> Vec<VHost> { self.vhosts.read().unwrap().clone() }
    fn add_vhost(&self, vhost: VHost) { self.vhosts.write().unwrap().push(vhost); }
    fn remove_vhost(&self, domain: &str) {
        self.vhosts.write().unwrap().retain(|v| v.domain() != domain);
    }
    fn run(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::errors::Result<()>> + Send + '_>> {
        let addr: std::net::SocketAddr = self.address.parse().unwrap();
        let router = self.router();
        Box::pin(async move { crate::server::Server::serve(addr, router).await })
    }
}
