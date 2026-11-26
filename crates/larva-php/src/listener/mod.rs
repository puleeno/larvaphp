//! LiteSpeed-style listeners: a Listener binds a port/protocol and serves a set
//! of virtual hosts. Design: Strategy (protocol) + Adapter (socket->HTTP) +
//! Registry (listener management).

use crate::server::Server;
use std::sync::Arc;

/// Protocol a listener speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Http1,
    Http2,
    Http3,
    Https,
}

/// A virtual host binding (alias for Server).
pub type VHost = Arc<Server>;

/// Adapter: turns a raw accepted connection into an HTTP request handler.
/// Implementations wrap hyper (http1/http2), quiche (http3), rustls (tls).
pub trait ProtocolAdapter: Send + Sync {
    fn protocol(&self) -> Protocol;
    fn name(&self) -> &'static str;
}

/// A listener binds an address and serves vhosts via an adapter.
pub trait Listener: Send + Sync {
    fn id(&self) -> &str;
    fn address(&self) -> &str;
    fn protocol(&self) -> Protocol;
    fn vhosts(&self) -> Vec<VHost>;
    fn add_vhost(&self, vhost: VHost);
    fn remove_vhost(&self, domain: &str);
}

/// Registry of listeners.
pub trait ListenerRegistry: Send + Sync {
    fn register(&self, listener: Arc<dyn Listener>);
    fn unregister(&self, id: &str);
    fn get(&self, id: &str) -> Option<Arc<dyn Listener>>;
    fn list(&self) -> Vec<Arc<dyn Listener>>;
}
