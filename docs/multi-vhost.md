# Multi-Vhost Architecture & Feature System

LarvaPHP v2 is a **multi-tenant** PHP application server: a single listening
port serves many isolated websites, each selected by the request `Host` header
— the same model as Nginx `server` blocks, Apache `<VirtualHost>`, LiteSpeed
virtual hosts, and Caddy site blocks.

On top of that, every virtual host declares **which features it wants**, and
features are **lazily initialized** so a site that does not use HTTP/3, ACME,
or the embedded runtime pays **zero** initialization cost.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Design Patterns](#2-design-patterns)
3. [VHostRouter — Multi-Tenant Routing](#3-vhostrouter--multi-tenant-routing)
4. [Per-Vhost Feature System](#4-per-vhost-feature-system)
5. [CLI Usage (Full Reference)](#5-cli-usage-full-reference)
6. [Supported Configuration Formats](#6-supported-configuration-formats)
7. [Control Panel Integration](#7-control-panel-integration)
8. [Security & Isolation](#8-security--isolation)
9. [Examples](#9-examples)
10. [Rust API](#10-rust-api)

---

## 1. Architecture Overview

```
                          ┌───────────────────────────┐
                          │       HTTP Client          │
                          │  (Host: a.com / b.com)     │
                          └─────────────┬─────────────┘
                                        │
                          ┌─────────────▼─────────────┐
                          │   Single TCP Listener      │
                          │   (one shared port, e.g.   │
                          │   0.0.0.0:8000)            │
                          └─────────────┬─────────────┘
                                        │  per-connection
                          ┌─────────────▼─────────────┐
                          │   VHostRouter              │
                          │   match Host header →      │
                          │   select isolated Server   │
                          └──────┬──────────────┬─────┘
                                 │              │
                    ┌────────────▼───┐   ┌──────▼────────────┐
                    │  VHost A       │   │  VHost B          │
                    │  (Server)      │   │  (Server)         │
                    │  root: /a      │   │  root: /b         │
                    │  workers: [..] │   │  workers: [..]    │
                    │  php.ini: {..} │   │  php.ini: {..}    │
                    │  rate limiter  │   │  rate limiter     │
                    │  features: {..}│   │  features: {..}   │
                    └────────┬───────┘   └────────┬──────────┘
                             │                    │
                    ┌────────▼─────┐      ┌───────▼─────────┐
                    │ PHP exec A   │      │ PHP exec B      │
                    │ (isolated)   │      │ (isolated)      │
                    └──────────────┘      └─────────────────┘
```

**Key properties:**

| Property | Description |
|----------|-------------|
| **One port, many sites** | A single `TcpListener` accepts all connections; routing is per-request by `Host` header (HTTP/1.1 and HTTP/2 both carry it). |
| **Full isolation** | Each vhost has its own document root, worker pool, `php.ini` overrides, rate limiter, and feature set. A request to `b.com` can never read files from `a.com`'s root. |
| **Default vhost** | The first registered vhost is the default (Nginx `default_server`); requests with an unknown `Host` fall back to it. |
| **Hot-reloadable** | The router is `RwLock<Vec<Arc<Server>>>`; the control panel swaps the vhost set atomically without restarting the listener. |
| **Lazy features** | Each vhost loads only the features it enables, on first use. Disabled features are never initialized. |

---

## 2. Design Patterns

| Pattern | Where | Purpose |
|---------|-------|---------|
| **Registry** | `VHostRouter` | Holds the set of `Arc<Server>` vhosts; `find_vhost` / `add` / `remove` / `replace_all`. |
| **Strategy** | `FeatureManager::ensure(kind, init)` | The `init` closure is the load strategy; the manager decides *when* (once, lazily) and *whether* (only if enabled) to run it. |
| **Lazy Initialization** | `FeatureManager` | `Mutex<HashSet<FeatureKind>>` tracks loaded kinds; `init` runs at most once per kind. |
| **Flyweight** | `Arc<Server>` | Vhosts are shared via `Arc` across connections; cheap to clone the handle. |
| **Adapter** | `parse_vhost_spec` / config parsers | CLI / Apache / Nginx / Caddy formats → canonical `Server` / `ServerConfig`. |

### Why lazy features matter

Compiling all features in (the default) gives you every capability, but
**initializing** a feature has a cost: spawning a QUIC listener, creating a
Tokio runtime for ACME, loading certificates, spawning worker processes. The
`FeatureManager` ensures a vhost that does not enable a feature **never pays
that cost** — the `init` closure is simply not called.

```
ensure(Http3, || start_quic_listener())   ──►  disabled?  ──► return false (no init)
                                              │ enabled & not loaded?
                                              ├─ yes ──► run init once, mark loaded
                                              └─ no  ──► return true (already loaded)
```

---

## 3. VHostRouter — Multi-Tenant Routing

`VHostRouter` is the registry that maps a request's `Host` header to an
isolated `Server` (virtual host).

```rust
pub struct VHostRouter {
    vhosts: RwLock<Vec<Arc<Server>>>,
}

impl VHostRouter {
    pub fn new(vhosts: Vec<Arc<Server>>) -> Self;
    pub fn single(server: Arc<Server>) -> Self;       // v1 single-app compat
    pub fn find_vhost(&self, host: &str) -> Option<Arc<Server>>;
    pub fn default_vhost(&self) -> Option<Arc<Server>>;
    pub fn list(&self) -> Vec<Arc<Server>>;
    pub fn replace_all(&self, vhosts: Vec<Arc<Server>>); // hot-reload
    pub fn add(&self, server: Arc<Server>);              // add/replace by domain
    pub fn remove(&self, domain: &str) -> bool;
}
```

### Host matching (`Server::matches_host`)

Matching is **case-insensitive**, strips the port, and supports **wildcard
aliases** like Apache/LiteSpeed/Nginx:

| `Host` header | Vhost `name` | Vhost `aliases` | Match? |
|---------------|--------------|-----------------|--------|
| `example.com` | `example.com` | — | ✅ exact |
| `example.com:8080` | `example.com` | — | ✅ port stripped |
| `EXAMPLE.COM` | `example.com` | — | ✅ case-insensitive |
| `www.example.com` | `example.com` | `www.example.com` | ✅ alias |
| `foo.example.com` | `test.org` | `*.example.com` | ✅ wildcard (one label) |
| `a.b.example.com` | `test.org` | `*.example.com` | ❌ wildcard = one label only |
| `unknown.com` | (no match) | — | ↳ falls back to **default vhost** |

### Request flow (per connection)

```
accept connection
   │
   ▼
service_fn(req):
   host = req.headers["host"]
   vhost = router.find_vhost(host).or(router.default_vhost())
   if vhost is None:           → 404 "No virtual host for this Host"
   if !vhost.is_registered():  → 503 Service Unavailable
   root       = vhost.root            ← isolated document root
   workers    = vhost.workers         ← isolated worker pool
   php_ini    = vhost.php_ini         ← isolated php.ini overrides
   rate_limit = vhost.rate_limiter    ← isolated rate limiter
   ... route request within this vhost ...
```

---

## 4. Per-Vhost Feature System

### FeatureKind

The registry of runtime-toggleable features (mirrors the Cargo features but
controllable per vhost):

| `FeatureKind` | Name | Cargo feature | What it initializes |
|---------------|------|---------------|---------------------|
| `Http3` | `http3` | `http3` | QUIC/UDP listener + Alt-Svc advertisement |
| `Acme` | `acme` | `acme` | Let's Encrypt account, order, cert download, renewal loop |
| `Embedded` | `embedded` | `embedded` | In-process PHP runtime (`LARVA_PHP_EXECUTOR=ffi`) |
| `Workers` | `workers` | — | Multi-process worker pool (`init_pool`) |
| `Mercure` | `mercure` | `mercure` | Mercure real-time hub |
| `Watcher` | `watcher` | `watcher` | File watching / hot reload |
| `Fcgi` | `fcgi` | `fcgi` | FastCGI client |
| `Panel` | `panel` | `panel` | Web control panel thread |

### VhostFeatures

A declarative set of enabled kinds. **Defaults to all** (enable everything),
opt out with `-name`:

```rust
use larva_php::{VhostFeatures, FeatureKind};

// All features (default)
let f = VhostFeatures::all();

// Only these three
let f = VhostFeatures::parse("http3,acme,embedded");

// All except http3/acme/panel
let mut f = VhostFeatures::all();
f.apply_delta("-http3,-acme,-panel");

// Delta: add panel, remove acme
f.apply_delta("+panel,-acme");
```

### FeatureManager (lazy)

Per-vhost manager. `ensure` runs the `init` closure **at most once**, and
**only if the feature is enabled**:

```rust
use larva_php::{FeatureManager, FeatureKind};

let mgr = FeatureManager::all();

// First call: runs init, marks loaded, returns true.
mgr.ensure(FeatureKind::Acme, || { /* load ACME */ Ok(()) });

// Second call: already loaded, returns true without re-running.
mgr.ensure(FeatureKind::Acme, || unreachable!());

// Disabled feature: returns false, init NEVER runs (zero overhead).
let mgr2 = FeatureManager::new(VhostFeatures::none());
mgr2.ensure(FeatureKind::Http3, || { panic!("must not run") }); // → false
```

---

## 5. CLI Usage (Full Reference)

### Basic

```powershell
# Single site (v1 compatible) — all features enabled by default
larva-php --root C:\www\app --addr 0.0.0.0:8000

# Run a PHP script directly
larva-php script.php arg1 arg2
```

### Multi-vhost (one port, many sites)

```powershell
larva-php `
  --root C:\www\a `                       # default vhost document root
  --server-name a.com `                  # default vhost domain (default: localhost)
  --vhost "b.com=C:\www\b" `             # additional vhost
  --vhost "c.com=C:\www\c;www.c.com,alias.c.com" `  # with aliases
  --addr 0.0.0.0:8000
```

### Per-vhost features

```powershell
# Default vhost: all features except http3/acme/panel
larva-php --root C:\www\a --features all,-http3,-acme,-panel --addr 0.0.0.0:8000

# Per-vhost: site A full, site B only embedded+workers
larva-php `
  --root C:\www\a --server-name a.com `
  --vhost "b.com=C:\www\b" `
  --vhost-features "b.com=-http3,-acme,-panel,-embedded" `
  --addr 0.0.0.0:8000
```

### Full flag reference

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--root <DIR>` | `-r` | (required) | Document root of the **default** vhost |
| `--server-name <NAME>` | — | `localhost` | Primary domain of the default vhost |
| `--vhost <SPEC>` | — | — | Additional vhost: `DOMAIN=ROOT` or `DOMAIN=ROOT;ALIAS1,ALIAS2` (repeatable) |
| `--features <LIST>` | — | `all` | Features for the default vhost: `http3,acme,...` or `all,-http3` |
| `--vhost-features <SPEC>` | — | — | Per-vhost feature delta: `DOMAIN=LIST` (repeatable) |
| `--addr <ADDR>` | `-a` | `127.0.0.1:8000` | Bind address |
| `--php-ini <K=V>` | — | — | php.ini override (repeatable) |
| `--workers <N>` | — | `0` | Worker pool size (enables `Workers` feature) |
| `--http3` | — | off | Enable HTTP/3 (enables `Http3` feature) |
| `--embedded` | — | off | Embedded PHP (enables `Embedded` feature) |
| `--lets-encrypt` | `-L` | off | ACME (enables `Acme` feature) |
| `--acme-email <E>` | — | — | ACME contact email |
| `--acme-domains <D>` | — | — | ACME domains (comma-separated) |
| `--acme-dir <DIR>` | — | `~/.larva-php/letsencrypt` | ACME storage |
| `--acme-challenge-dir <DIR>` | — | `<root>/.well-known/acme-challenge` | HTTP-01 challenge dir |
| `--acme-staging` | — | off | Use Let's Encrypt staging |
| `--acme-renew-days <N>` | — | `30` | Renew when ≤ N days remain |
| `--cert <FILE>` / `--key <FILE>` | — | auto | TLS cert/key for HTTP/3 |
| `--panel <PORT>` | — | off | Control panel port (enables `Panel` feature) |
| `--debug` | — | off | Verbose logging |
| `--version` / `--help` | `-v`/`-h` | — | Version / help |

> **Backward compatibility:** the legacy flags (`--http3`, `--lets-encrypt`,
> `--embedded`, `--panel`, `--workers N>0`) still work — they map **additively**
> onto the default vhost's feature set.

### `--features` / `--vhost-features` syntax

| Syntax | Meaning |
|--------|---------|
| `http3,acme,embedded` | **Absolute set** — only these features |
| `all` | All features |
| `all,-http3,-acme` | All **except** http3 and acme (delta) |
| `+panel,-acme` | Delta: add panel, remove acme |
| (empty) | No features |

Unknown names are ignored. Matching is case-insensitive.

---

## 6. Supported Configuration Formats

LarvaPHP accepts configuration in multiple formats through the **Strategy
pattern** (`ConfigParser` trait + `ConfigLoader` registry). All formats are
adapted to a single internal `ServerConfig`.

### 6.1 CLI format

```powershell
larva-php --root /var/www/html --addr 0.0.0.0:8000
```

### 6.2 Apache format (`<VirtualHost>` / `.htaccess`)

```apache
<VirtualHost *:80>
    DocumentRoot "C:/www/site"
    ServerName example.com
    ServerAlias www.example.com
    Listen 8080
</VirtualHost>
```

Supported directives: `DocumentRoot`, `ServerName`, `ServerAlias`, `Listen`.

### 6.3 Nginx format (`server` block)

```nginx
server {
    listen 80;
    server_name example.com;
    root /var/www/html;
    index index.php;
}
```

Supported directives: `root`, `server_name`, `listen`, `index`.

### 6.4 Caddy format (Caddyfile site block)

```caddy
example.com {
    root * /var/www/html
    listen 8080
}
```

Supported directives: `root` (supports `*` placeholder), `listen`.

### 6.5 Using the parsers programmatically

```rust
use larva_php::config::ConfigLoader;

let loader = ConfigLoader::default();

// CLI
let c = loader.parse("cli", "C:\\www\\app")?;

// Apache
let c = loader.parse("apache", r#"
<VirtualHost *:80>
    DocumentRoot "C:/www/app"
    ServerName example.com
    ServerAlias www.example.com
</VirtualHost>
"#)?;

// Nginx
let c = loader.parse("nginx", "server { listen 80; root /var/www/html; }")?;

// Caddy
let c = loader.parse("caddy", "example.com { root * /var/www/html }")?;

// Register a custom parser
let mut loader = ConfigLoader::default();
loader.register("custom", Box::new(MyCustomParser));
```

> **Note:** the file parsers produce a **single** `ServerConfig` (one vhost).
> For **multi-vhost** at runtime, use the CLI `--vhost` / `--vhost-features`
> flags or the `VHostRouter` API directly (see §10). A single config file with
> multiple `<VirtualHost>` / `server` blocks is parsed as one vhost (last
> directive wins) — multi-vhost is expressed via repeated `--vhost` flags or
> the panel.

### 6.6 Format → vhost mapping

| Format | Multi-vhost mechanism | Isolation |
|--------|----------------------|-----------|
| CLI `--vhost` | Repeatable `DOMAIN=ROOT;ALIASES` | Full (per-vhost root/workers/ini) |
| Apache | `<VirtualHost>` per site (programmatic) | Full |
| Nginx | `server { }` per site (programmatic) | Full |
| Caddy | Site block per site (programmatic) | Full |
| Panel | Websites table → `RouterReloader` | Full (hot-reload) |

---

## 7. Control Panel Integration

The web control panel (port set by `--panel`) manages websites in a SQLite
DB. After any create/update/delete, it calls a `VHostReloader` that rebuilds
the live router **without restarting the listener**.

```
Panel (separate thread, own Tokio runtime)
   │  POST /api/websites  (create)
   ▼
SQLite: websites table
   │  reload_vhosts()
   ▼
VHostReloader::reload(websites)        ← trait defined in panel crate
   │  (implemented in main.rs)
   ▼
RouterReloader { router: Arc<VHostRouter> }
   │  build Vec<Arc<Server>> from active websites
   ▼
router.replace_all(vhosts)             ← atomic hot-reload
```

The `VHostReloader` trait lives in the **panel crate** to avoid a circular
dependency (`larva-php` → `larva-php-panel`); the concrete `RouterReloader`
lives in `main.rs` and converts each panel website into an isolated `Server`
(own `document_root`, `php_version`, `worker_count`, aliases from
`acme_domains`).

---

## 8. Security & Isolation

| Threat | Mitigation |
|--------|------------|
| **Cross-site file access** | Each vhost has its own `root`; `resolve_script_path_static` verifies `sanitized.starts_with(root)` — a request to `b.com` cannot read `a.com`'s files. |
| **Path traversal (`../`)** | `sanitize_path()` resolves/rejects `..` segments; null bytes rejected. |
| **Sensitive files** | `.env`, `.git/`, `wp-config.php`, `composer.lock`, `.htaccess`, `*.sql` → 403. |
| **Body DoS** | Per-vhost `max_body_size` (default 10 MB). |
| **Rate abuse** | Per-vhost fixed-window rate limiter (default 60 req/60 s per IP). |
| **Unknown Host** | Falls back to default vhost (never leaks another site's content). |

---

## 9. Examples

### 9.1 Two WordPress sites on one port

```powershell
larva-php `
  --root C:\www\site-a --server-name a.com `
  --vhost "b.com=C:\www\site-b;www.b.com" `
  --workers 4 --addr 0.0.0.0:8000
```

- `Host: a.com` → site A (default)
- `Host: b.com` or `Host: www.b.com` → site B
- `Host: unknown` → site A (default)

### 9.2 Full-featured default + minimal vhost

```powershell
larva-php `
  --root C:\www\main --server-name main.com `
  --vhost "api.com=C:\www\api" `
  --vhost-features "api.com=-http3,-acme,-panel" `
  --http3 --lets-encrypt --acme-email admin@main.com --acme-domains main.com `
  --panel 9000 --addr 0.0.0.0:8000
```

- `main.com` (default): http3 + acme + panel + workers + embedded (all)
- `api.com`: everything **except** http3/acme/panel (lazy — those subsystems
  are never initialized for this vhost)

### 9.3 Disable everything for a static-only vhost

```powershell
larva-php `
  --root C:\www\static --server-name static.com `
  --features -http3,-acme,-panel,-embedded,-workers `
  --addr 0.0.0.0:8000
```

Log confirms: `default vhost features: mercure, watcher, fcgi` — no QUIC
listener, no panel thread, no worker pool.

---

## 10. Rust API

```rust
use std::sync::Arc;
use larva_php::server::{Server, VHostRouter};
use larva_php::{VhostFeatures, FeatureKind, FeatureManager};

// Build vhosts
let mut a = Server::new("/var/www/a")?;
a.set_domains("a.com".into(), vec!["www.a.com".into()]);
a.set_features(VhostFeatures::parse("http3,acme,embedded"));
a.activate();

let mut b = Server::new("/var/www/b")?;
b.set_domains("b.com".into(), vec![]);
b.set_features(VhostFeatures::all()); // default: all
b.activate();

// Router (multi-tenant)
let router = Arc::new(VHostRouter::new(vec![Arc::new(a), Arc::new(b)]));

// Serve on one shared port
Server::serve("0.0.0.0:8000".parse()?, router).await?;

// Hot-reload (e.g. from the panel)
router.replace_all(vec![/* new vhosts */]);
```

### FeatureManager direct usage

```rust
use larva_php::{FeatureManager, FeatureKind, VhostFeatures};

let mgr = FeatureManager::new(VhostFeatures::parse("http3,embedded"));

// Lazy: init runs once, only if enabled.
mgr.ensure(FeatureKind::Http3, || {
    // start QUIC listener ...
    Ok(())
});

mgr.is_enabled(FeatureKind::Acme);  // false
mgr.is_loaded(FeatureKind::Http3);  // true after ensure
```

---

## See Also

- [`architecture.md`](architecture.md) — overall system architecture
- [`configuration.md`](configuration.md) — config formats & `ServerConfig`
- [`runtime-architecture.md`](runtime-architecture.md) — 2-layer worker runtime
- [`acme-letsencrypt.md`](acme-letsencrypt.md) — ACME / Let's Encrypt
- [`http3.md`](http3.md) — HTTP/3 (QUIC)
- [`embedded-php.md`](embedded-php.md) — embedded in-process PHP