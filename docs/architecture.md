# Architecture

LarvaPHP is a Rust port of [FrankenPHP](https://frankenphp.dev). It embeds the PHP Zend Engine via C FFI and serves PHP applications through a high-performance asynchronous HTTP server.

> **v2 multi-tenant:** a single port serves many isolated virtual hosts
> (Nginx/Apache/LiteSpeed/Caddy-style) with per-vhost lazy features. See
> [`multi-vhost.md`](multi-vhost.md) for the virtual-host router, feature
> system, and configuration formats.

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                          HTTP Client                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Hyper HTTP Server (Tokio)                    │
│  • TLS/TCP listener                                             │
│  • Request/Response handling                                    │
│  • Security headers (nosniff, DENY)                             │
│  • Rate limiting (fixed-window per-IP)                          │
│  • Max body size enforcement                                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Middleware Chain (CoR)                       │
│  • RewriteMiddleware (.htaccess-style)                          │
│  • Custom user middleware                                       │
│  • Chain executed once per request                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    PhpContext (per-request)                     │
│  • CGI environment variables                                    │
│  • Request method, URI, headers                                 │
│  • Server block reference                                       │
│  • Worker assignment                                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   PHP Thread State Machine                      │
│  States: Reserved → Booting → Ready → Inactive                  │
│  • Thread pooling                                               │
│  • State transitions via condition variables                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Zend Engine (via FFI)                          │
│  • PHP script execution                                         │
│  • Output buffering                                             │
│  • CGI SAPI emulation                                           │
└─────────────────────────────────────────────────────────────────┘
```

## Crates

| Crate | Purpose |
|-------|---------|
| `larva-php` | Main library - HTTP server, PHP embedding, configuration |
| `larva-php-sys` | FFI bindings to the Zend Engine C API |
| `larva-php-caddy` | Caddy web server module for PHP |
| `larva-php-extgen` | PHP extension code generator |

## Modules (larva-php crate)

### `server` - Server Blocks & Virtual Hosts

The `Server` struct represents a scoped server block (virtual host). Each server has its own document root, rate limiter, worker pool, file cache, and **feature set**. In v2 a single listener serves many `Server` vhosts via the `VHostRouter` (see [`multi-vhost.md`](multi-vhost.md)).

Key responsibilities:
- HTTP request handling via Hyper
- Rate limiting (fixed-window per-IP)
- File caching with ETag/MTIME
- Sensitive file protection
- MIME type detection
- Host-based vhost matching (`matches_host` with wildcard alias support)

### `features` - Per-Vhost Lazy Feature System

Each vhost declares which features it wants (`VhostFeatures`, defaults to all) and a `FeatureManager` **lazily** initializes a feature on first use only. Disabled features are never initialized (zero overhead). Uses the Strategy pattern (`ensure(kind, init_closure)`) + Registry (`FeatureKind`) + Lazy Initialization (`Mutex<HashSet>`). See [`multi-vhost.md`](multi-vhost.md).

### `context` - Per-Request Context

`PhpContext` holds all CGI environment variables and request state for a single PHP request. It maps HTTP request data to PHP's `$_SERVER` superglobal.

Key fields:
- `document_root`, `script_filename` - Path resolution
- `query_string`, `request_uri`, `path_info` - URI decomposition
- `request_method`, `content_type`, `content_length` - HTTP request data
- `server_name`, `server_port`, `https` - Server identification
- `is_done` - Request completion flag

### `cgi` - CGI Variables & Path Splitting

CGI variable registration (HTTP headers → `HTTP_*` variables) and PATH_INFO/SCRIPT_NAME splitting.

Key functions:
- `header_name()` - Maps HTTP header to PHP `$_SERVER` variable
- `split_pos()` - Finds the PHP extension split point in URI
- `clean_request_path()` - Normalizes Windows/Unix paths
- `c_string_http_method()` - Returns valid HTTP method strings

### `thread` - PHP Thread State Machine

Manages PHP thread lifecycle with a state machine:

```
Reserved → Booting → Ready → Inactive → (repeat)
     ↑                                              │
     └─────── Rebooting → ForceRebooting ←──────────┘
```

Thread states:
- `Reserved` - Thread slot allocated but not initialized
- `Booting` - PHP interpreter being initialized
- `Ready` - Thread ready to handle requests
- `Inactive` - Thread waiting for work
- `ShuttingDown` - Thread being torn down

### `worker` - Persistent Workers

Worker mode runs persistent PHP scripts that stay loaded between requests, reducing bootstrap overhead. Workers can have request matchers to route specific paths.

### `watcher` - File Watching & Hot Reload

Cross-platform file system watching via the `notify` crate with debounce coalescing (150ms). Used for hot reload of configuration and PHP source files.

### `errors` - Error Types

The `Error` enum covers all failure modes. "Rejected" errors carry HTTP status codes for direct mapping to responses:

| Error | Status | Meaning |
|-------|--------|---------|
| `InvalidRequestPath` | 400 | Malformed request path |
| `InvalidScriptPath` | 404 | PHP script not found |
| `InvalidContentLengthHeader` | 400 | Bad Content-Length |
| `MaxWaitTimeExceeded` | 503 | Request timeout |
| `RequestBodyTooLarge` | 413 | Body exceeds limit |

### `options` - Configuration Options

Builder pattern for configuring LarvaPHP:
- `Option` - Global configuration
- `WorkerOption` - Worker configuration
- `ServerOption` - Server block configuration
- `RequestOption` - Per-request configuration

### `env` - Environment Management

Prepares NUL-terminated environment blocks for passing to PHP's Zend Engine. Handles the `prepare_env()` / `ensure_prepared_env()` conversion with empty-key filtering.

### `types` - PHP Value Wrappers

Safe wrappers for `zval` conversion between PHP and Rust:
- `PhpValue` - Null, Bool, Long, Double, String, Array
- `AssociativeArray` - Ordered PHP arrays with key preservation
- `call_php_callable()` - Invoke PHP callables from Rust

### `metrics` - Prometheus Metrics

Thread-safe metrics collection:
- `AtomicMetrics` - Thread count, busy threads, queue depth
- `NullMetrics` - No-op implementation for when metrics are disabled
- `StopReason` - Crash, Restart, BootFailure

### `scaling` - Auto-Scaling

CPU-based auto-scaling of PHP threads. Probes CPU usage and adjusts thread count up/down within configured bounds.

### `mercure` - Real-Time Updates

Optional Mercure hub integration for Server-Sent Events (SSE) real-time updates.

### `embed` - Embedded Apps

Embed PHP applications directly in the binary for distribution without external file dependencies.

### `cli` - Script Execution

Execute PHP scripts via the CLI SAPI for command-line usage.

### `phpinfo` - phpinfo() Integration

Generates PHP-compatible phpinfo() output for debugging.

## Design Patterns

| Pattern | Where | Purpose |
|---------|-------|---------|
| Strategy | `config.rs` | Multiple config format parsers (CLI, Apache, Nginx, Caddy) |
| Strategy | `features.rs` | `FeatureManager::ensure(kind, init)` — lazy feature loading |
| Builder | `options.rs` | Step-by-step configuration construction |
| Chain of Responsibility | `config.rs` | Middleware pipeline for request preprocessing |
| Adapter | `cgi.rs` | HTTP headers → PHP `$_SERVER` variables |
| Flyweight | `server.rs` | `Arc<Server>` vhosts shared across connections |
| Registry | `server.rs` / `features.rs` | `VHostRouter` (vhosts) / `FeatureKind` (features) |
| Lazy Initialization | `features.rs` | `Mutex<HashSet>` tracks loaded features, init once |
| State Machine | `thread.rs` | PHP thread lifecycle management |
| Observer | `watcher.rs` | File change notifications |
| Atomic Swap | `watcher.rs` | `ArcSwap` for lock-free config hot-reload |

## Performance Considerations

1. **Rate limiter**: `Arc<DashMap>` shared across connections - no lock contention on read path
2. **Middleware chain**: Built once at startup, O(n) iteration per request with no allocation
3. **Regex**: Pre-compiled at config load time (Flyweight)
4. **File cache**: `DashMap` with size/MTIME/ETag for conditional requests
5. **Hot reload**: `ArcSwap` allows lock-free config reads during atomic swaps
6. **Worker mode**: Persistent PHP interpreters avoid per-request bootstrap cost

## Security

- **X-Content-Type-Options: nosniff** - Prevents MIME sniffing
- **X-Frame-Options: DENY** - Prevents clickjacking
- **Sensitive file blocking** - `.env`, `.git/`, `wp-config.php`, `composer.lock`, `.htaccess`, `.sql` files are rejected with 403
- **Path traversal protection** - `..` segments are resolved/rejected in `sanitize_path()`
- **Null byte rejection** - Request paths containing `\0` are rejected
- **Max body size** - Default 10 MB limit prevents body-based DoS
