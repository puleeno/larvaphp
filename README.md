# LarvaPHP

**LarvaPHP** - A modern PHP application server written in Rust. A port of [FrankenPHP](https://frankenphp.dev) that embeds PHP via the Zend Engine C API and serves PHP applications through a high-performance HTTP server built on [Hyper](https://hyper.rs) and [Tokio](https://tokio.rs).

## Project Structure

```
larva-php/
├── Cargo.toml                      # Workspace root
├── README.md
├── docs/
│   ├── architecture.md             # System architecture
│   ├── embedded-php.md             # Embedded PHP via C shim (in-process PHP)
│   ├── configuration.md            # Configuration reference
│   ├── api.md                      # Public API documentation
│   ├── hot-reload.md               # Hot reload guide
│   ├── middleware.md               # Middleware & rewrite rules
│   ├── troubleshooting.md          # Known issues & diagnosis (HTTP/2 cookies, login loops)
│   └── examples.md                 # Usage examples
└── crates/
    ├── larva-php/                  # Main crate
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs              # Public API entry point
    │       ├── main.rs             # CLI binary
    │       ├── config.rs           # Server config (Strategy/Builder/Chain of Responsibility)
    │       ├── server.rs           # Server blocks & rate limiting
    │       ├── context.rs          # Per-request PHP context
    │       ├── cgi.rs              # CGI variables & path splitting
    │       ├── thread.rs           # PHP thread state machine
    │       ├── worker.rs           # Persistent worker mode
    │       ├── watcher.rs          # File watching & hot reload
    │       ├── errors.rs           # Error types
    │       ├── options.rs          # Configuration options (Builder pattern)
    │       ├── env.rs              # Environment variable management
    │       ├── types.rs            # PHP zval wrappers
    │       ├── metrics.rs          # Prometheus-compatible metrics
    │       ├── scaling.rs          # Auto-scaling
    │       ├── mercure.rs          # Mercure real-time
    │       ├── embed.rs            # Embedded PHP app
    │       ├── cli.rs              # CLI script execution
    │       └── phpinfo.rs          # phpinfo() integration
    ├── larva-php-sys/              # FFI bindings to Zend Engine
    ├── larva-php-caddy/            # Caddy module
    └── larva-php-extgen/           # Extension generator
```

## Features

- **PHP 8.2+ support** via Zend Engine C API (embedded, not CGI)
- **High-performance HTTP server** built on Hyper/Tokio
- **Multiple config formats**: CLI `--root`, Apache `<DocumentRoot>`, Nginx `root`, Caddyfile `root`
- **Middleware chain**: Chain of Responsibility pattern for request preprocessing (rewrite, .htaccess-style rules)
- **Rate limiting**: Fixed-window per-IP rate limiter with configurable limits
- **Hot reload**: File watching with debounce and atomic config swap (ArcSwap)
- **Security headers**: `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`
- **Sensitive file protection**: Blocks access to `.env`, `.git/`, `wp-config.php`, etc.
- **Worker mode**: Persistent PHP scripts for reduced overhead
- **Prometheus metrics**: Built-in metrics collection
- **Mercure support**: Real-time updates (optional feature)
- **Caddy module**: Run as a Caddy PHP module

## Quick Start

### Build

**Windows (recommended):** use `build.ps1`, which auto-configures the MSVC
environment (finds Visual Studio via vswhere, enters the x64 dev shell) and runs
cargo with your arguments:

```powershell
.\build.ps1 --release                     # release build
.\build.ps1 --features embedded           # with HTTP/3 + embedded PHP FFI
.\build.ps1 test --lib                    # run tests
```

**Manual (any platform):** ensure the MSVC toolchain is in PATH (run from a
"Developer PowerShell for VS 2022" or set `PATH`/`INCLUDE`/`LIB`), then:

```powershell
cargo build --release
```

### Platform build strategies (Windows / Linux / macOS)

Build-time platform handling uses the **Strategy pattern** in
[`crates/larva-php-sys/build/platform.rs`](crates/larva-php-sys/build/platform.rs).
`build.rs` talks only to the `PlatformBuild` trait; `select()` is the factory
that picks exactly one strategy for the host OS, so platform code never
conflicts or gets mixed together:

| Strategy | Host OS | C bridge | Linking | Embedded PHP |
|----------|---------|----------|---------|--------------|
| `WindowsPlatform` | Windows | `stubs.c` + `larva_embed.c` | `php8.lib`, MSVC libs | supported (`php8.dll`) |
| `LinuxPlatform` | Linux | `stubs.c` + `larva_embed_unix.c` | `libphp.so`, system PHP libs | supported (real Zend SAPI) |
| `MacosPlatform` | macOS | `stubs.c` + `larva_embed_unix.c` | PHP embed library | supported (real Zend SAPI) |

Linux and macOS share the `UnixPlatform` implementation but are distinct
named strategies, so they can diverge later without touching shared code.
The old MSVC-only `+crt-static` flag was removed from
[`.cargo/config.toml`](.cargo/config.toml) for Linux/macOS (it broke host
proc-macro artifacts).

On Linux/macOS the real embed path needs PHP development headers and the PHP
embed library. No no-op PHP stubs are compiled into this release.

See [`docs/build-release-linux.md`](docs/build-release-linux.md) for the full
requirements, environment variables, verification steps and troubleshooting.

```bash
# Debian/Ubuntu (quick start)
sudo apt install -y build-essential cmake clang libclang-dev \
  php8.3-dev php8.3-cli libphp8.3-embed
sudo ln -sf /usr/lib/libphp8.3.so /usr/lib/libphp.so

export PHP_LIB_DIR=/usr/lib
export PHP_EMBED_LIB=php8.3
export PHP_INCLUDES="$(php-config --includes)"
cargo build --release -p larva-php --bin larva-php \
  --features embedded,http3,acme,mercure
cargo test --lib --features embedded,http3,acme,mercure
```

`PHP_LIB_DIR` is embedded in the binary's RUNPATH, so the resulting binary runs
without setting `LD_LIBRARY_PATH`.

To run the all-feature release as a daemon, see
[`docs/linux-daemon-service.md`](docs/linux-daemon-service.md). Ready-to-use
systemd units and an env template live in
[`deploy/systemd/`](deploy/systemd/).

### Serve a PHP Application

```powershell
# Default (CGI spawn per request)
.\target\release\larva-php.exe --root C:\path\to\php-app --addr 127.0.0.1:8000

# 2-layer fault-isolated runtime (embedded PHP in worker processes, ~10x faster)
.\build.ps1 --release --features embedded
.\target\release\larva-php.exe --root C:\path\to\php-app --workers 4 --addr 127.0.0.1:8000

# Run a PHP script directly
.\target\release\larva-php.exe script.php arg1 arg2
```

### Transport modes

| Mode | Command | PHP runs in | Speed |
|------|---------|-------------|-------|
| CGI (default) | `--root app` | php-cgi.exe spawn/request | ~85 ms |
| FastCGI | `--features fcgi` | php-cgi -b | ~80 ms (Windows) |
| **2-layer (FFI)** | `--features embedded --workers N` | **worker process (in-process)** | **~8 ms** |

The 2-layer mode is a LiteSpeed-style architecture: the master (Rust/Hyper)
handles HTTP and never runs PHP; N worker child processes each embed PHP via
FFI. A segfault in a worker is isolated and respawned by the master. See
[`docs/runtime-architecture.md`](docs/runtime-architecture.md).

### Test with WordPress

```powershell
.\target\release\larva-php.exe --root C:\Users\Administrator\Projects\wordpress.org --addr 127.0.0.1:9000
# Visit http://127.0.0.1:9000
```

## Configuration

### CLI Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--root <DIR>` | `-r` | (required for server mode) | Document root directory |
| `--addr <ADDR>` | `-a` | `127.0.0.1:8000` | Bind address |
| `--version` | `-v` | - | Show version |
| `--help` | `-h` | - | Show help |

### Supported Config Formats

LarvaPHP accepts configuration in multiple formats through the Strategy pattern:

| Format | Parser | Example |
|--------|--------|---------|
| CLI | `CliParser` | `--root /var/www/html` |
| Apache | `ApacheParser` | `<DocumentRoot "/var/www/html">` |
| Nginx | `NginxParser` | `root /var/www/html;` |
| Caddy | `CaddyParser` | `root * /var/www/html` |

See [`docs/configuration.md`](docs/configuration.md) for details.

### Rate Limiting

Fixed-window per-IP rate limiter (default: 60 requests per 60 seconds):

```rust
use larva_php::config::{RateLimitConfig, RateLimiter};

let limiter = RateLimiter::new(RateLimitConfig {
    max_requests: 30,
    window_secs: 60,
});

if limiter.check("192.168.1.1") {
    // allow request
} else {
    // return HTTP 429
}
```

## Hot Reload

LarvaPHP supports hot reloading of configuration and PHP source files using the `notify` crate with debounce coalescing.

```rust
use larva_php::config::hot_reload::HotReloadManager;

let mut mgr = HotReloadManager::new("cli", "/path/to/config", loader.clone());
mgr.watch_dir(PathBuf::from("/path/to/watch"));
mgr.start()?;

// When files change, config is re-parsed and atomically swapped
```

See [`docs/hot-reload.md`](docs/hot-reload.md) for details.

## Middleware & Rewrite

LarvaPHP includes a Chain of Responsibility middleware pipeline for request preprocessing:

```rust
use larva_php::config::{MiddlewareChain, RewriteMiddleware, RewriteRule, RewriteFlag};

// .htaccess-style rewrite rules
let rules = vec![
    RewriteRule::new(r"^/old/(.+)$", "/new/$1", RewriteFlag::Last)?,
    RewriteRule::new(r"^/legacy/(.+)$", "/current/$1", RewriteFlag::Next)?,
];

let mw = RewriteMiddleware::new(rules);
let chain = MiddlewareChain::new().add(Arc::new(mw));
```

See [`docs/middleware.md`](docs/middleware.md) for details.

## Testing

```powershell
cargo test --lib
```

All tests run without requiring a PHP installation (pure Rust unit tests).

## Architecture

See [`docs/architecture.md`](docs/architecture.md) for detailed architecture.

## Troubleshooting

See [`docs/troubleshooting.md`](docs/troubleshooting.md) for known issues and
diagnosis steps, including the HTTP/2 duplicate `Cookie` header bug that caused
WordPress login loops with `reauth=1`.

## License

MIT License. See [LICENSE](LICENSE) for details.

## Related Projects

- [FrankenPHP](https://frankenphp.dev) - The Go-based PHP application server that inspired this port
- [PHP](https://php.net) - The Hypertext Preprocessor language
- [Hyper](https://hyper.rs) - Fast HTTP library for Rust
- [Tokio](https://tokio.rs) - Async runtime for Rust
