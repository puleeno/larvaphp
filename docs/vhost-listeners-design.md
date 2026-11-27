# VHost Listeners — LiteSpeed-style Design

## Recommendation
Yes, follow LiteSpeed/OpenLiteSpeed: a **Listener** binds a port/protocol and maps to a set of **VirtualHosts**. Multiple listeners (80, 443, 8443) can share vhosts or have dedicated ones. This gives per-vhost port binding, protocol selection, and SSL cert per listener.

## Design Pattern: Strategy + Adapter + Registry

### Core abstractions (traits)
- `Listener` (trait): `bind()`, `accept()`, `protocol()`, `vhosts()`. Strategy pattern — each protocol (HTTP/1.1, HTTP/2, HTTP/3, HTTPS) is a strategy.
- `VHost` (struct): domain, document_root, php_version, ssl, aliases. Already exists as `Server`.
- `ListenerRegistry` (trait): register/unregister/list listeners. Registry pattern.
- `ProtocolAdapter` (trait): adapts a raw TCP/UDP socket into an HTTP service. Adapter pattern — wraps hyper/quiche into a uniform `HttpService`.

### Architecture
```
ListenerRegistry
  ├── HttpListener (port 80)  -> VHostRouter (all vhosts)
  ├── HttpsListener (port 443) -> VHostRouter (ssl vhosts)
  └── Http3Listener (port 443/udp) -> VHostRouter (http3-enabled vhosts)
```

Each Listener holds an `Arc<VHostRouter>` and a `ProtocolAdapter`. On accept, adapter upgrades socket → HTTP request → VHostRouter resolves vhost → handler.

### Adapter implementations
- `Http1Adapter` (hyper http1)
- `Http2Adapter` (hyper http2)
- `Http3Adapter` (quiche)
- `TlsAdapter` (rustls) wraps any of the above

### Config (DB + CLI)
- Table `listeners` (id, name, address, port, protocol, ssl_cert_id, is_active)
- Table `listener_vhosts` (listener_id, website_id) — many-to-many
- CLI: `--listener PORT=PROTO` (e.g. `--listener 80=http`, `--listener 443=https`)

### Migration
- Default listener on `--addr` (backward compat).
- Panel UI: Listeners page (add/edit/delete, bind vhosts).

## Implementation Phases
1. Trait scaffolding (`listener` module in larva-php) — Listener, ListenerRegistry, ProtocolAdapter.
2. Refactor `Server::serve` to accept a listener config.
3. DB tables + panel API + UI.
4. CLI flags.
5. Tests.

## Enterprise features
- Per-listener SSL cert (SNI)
- Per-listener rate limits / WAF
- Per-listener access logs
