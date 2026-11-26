# Performance

LarvaPHP's request path and how to get the most out of it, with measured numbers.

Connection/request state isolation tests are documented in
[`connection-isolation.md`](connection-isolation.md).

## TL;DR

- **Static files** (css/js/images): **~3 ms** — the server is not the bottleneck.
- **PHP requests**: **~85 ms** on Windows — dominated by `php-cgi` per-request
  initialization, not the server.
- **WordPress `setup-config.php` taking 15 s** is **not the server** — it is
  WordPress trying to reach MySQL and hitting a TCP connect timeout.

## Measured breakdown

All numbers measured on Windows 11, PHP 8.3, `larva-php` debug build, loopback.

| Path | Time | What dominates |
|------|------|----------------|
| Static file (`GET /style.css`) | 2.6 – 3.3 ms | hyper + file read |
| `php-cgi -v` (with php.ini) | 87 – 117 ms | process creation + php.ini load |
| `php-cgi -n -v` (no php.ini) | 53 – 79 ms | process creation only |
| PHP script via LarvaPHP | 75 – 108 ms | php-cgi startup + execute |
| WordPress `setup-config.php` (no DB) | ~1.5 s | WordPress bootstrap + DB connect |
| WordPress `setup-config.php` (unreachable DB) | ~15 s | TCP connect timeout to MySQL |

**Key takeaway:** the server adds only **~3–5 ms** of overhead. The rest is
`php-cgi` doing PHP initialization on every request.

## Where the time goes (PHP request)

```
HTTP request
  │
  ├─ hyper parse + routing ........ ~1 ms
  ├─ rate limit / path resolve .... ~1 ms
  ├─ spawn php-cgi (Windows) ...... ~50 ms   ← process creation
  ├─ load php.ini + extensions .... ~30 ms   ← php-cgi startup
  ├─ execute PHP .................. ~2 ms
  └─ send response ................ ~1 ms
```

## Why `php-cgi` is slow (and FastCGI doesn't fully fix it on Windows)

- **CGI model**: every request spawns a fresh `php-cgi.exe` process. Process
  creation on Windows is ~50 ms, plus ~30 ms to parse `php.ini` and load
  extensions.
- **FastCGI (`php-cgi -b`)**: avoids the *process* spawn, but `php-cgi`'s
  FastCGI responder still re-initializes PHP per request on Windows, so the
  gain is small there. On **Linux with `php-fpm`** it is a large win (true
  persistent workers + opcache reuse → ~5 ms/request).

LarvaPHP's `fcgi` feature (`--features fcgi`) implements a FastCGI client with
a connection pool, so it is ready for `php-fpm` on Linux.

## What LarvaPHP does to stay fast

| Optimization | Where | Effect |
|--------------|-------|--------|
| Static file serving without PHP | `server.rs` | ~3 ms for css/js/img |
| Cached `find_php_cgi()` | `server.rs` | no `stat` calls per request |
| Allocation-free path resolution | `path_resolver.rs` | no `Box` per request |
| Pre-compiled rewrite regex | `config.rs` | compiled once at load |
| Rate limiter via `Arc<DashMap>` | `server.rs` | lock-free per-IP counter |
| `ArcSwap` config hot-reload | `watcher.rs` | lock-free config reads |
| FastCGI connection pool | `fastcgi.rs` | reuse TCP connection (Linux) |
| Edge cache (Jankx page cache) | `edge_cache.rs`, `server.rs` | serve rendered pages without starting PHP |
| Cached standard-level gzip | `server.rs` | smaller HTML responses; recompressed only on cache change |

## Edge-cache compression

`LARVA_EDGE_CACHE` lets LarvaPHP read Jankx's serialized page cache before PHP
starts. The decoded HTML is compressed once and stored in `GZIP_CACHE`, keyed by
the cache file and its source length; a changed page is recompressed on the next
request.

The gzip result uses `Compression::default()` rather than `Compression::fast()`.
On a real WordPress homepage this reduced the response from **37.3 KB to 27.4 KB**
gzip (124,835 bytes of HTML) with no per-request compression cost. This matters
most on mobile profiles where the document itself is on the critical path.

## How to make PHP fast

### Linux (recommended for production)

Use `php-fpm` as the FastCGI backend and build LarvaPHP with FastCGI:

```bash
cargo build --release -p larva-php --features fcgi
# php-fpm listens on 127.0.0.1:9000; point LarvaPHP's fcgi backend at it.
```

With `php-fpm`, each PHP request is served by a pre-warmed worker with opcache
already populated — typically **5–10× faster** than CGI spawn.

### Windows

Options, in order of impact:

1. **Fix the app, not the server.** The 15 s `setup-config.php` is a MySQL
   connect timeout. Configure `wp-config.php` with a reachable DB, and/or set
   `define('DB_CONNECT_TIMEOUT', 5);`. Use `127.0.0.1` instead of `localhost`
   for `DB_HOST` to skip a slow IPv6 lookup.

2. **Enable opcache** (helps if you move to a persistent SAPI such as php-fpm
   under WSL2, or a future embedded-PHP build).

3. **Embedded PHP (FFI)** — the real fix for Windows: run PHP in-process like
  `mod_php`, eliminating the per-request process entirely. This is the hardest
   path (the original LarvaPHP/FrankenPHP approach) and is future work.

## Diagnosing slowness

Add timing to see whether the time is in the server or in PHP:

```powershell
# Time a static file vs a PHP script
curl.exe -s -o NUL -w "static: %{time_total}s\n" http://127.0.0.1:8000/style.css
curl.exe -s -o NUL -w "php:    %{time_total}s\n" http://127.0.0.1:8000/index.php
```

If `static` is fast (~3 ms) but `php` is slow, the cost is `php-cgi`, not
LarvaPHP. To confirm it is the app, time a trivial script:

```php
<?php echo 'ok'; ?>
```

If the trivial script is fast but your page is slow, the page (DB, external
HTTP calls, etc.) is the cause.

## Configuration that affects performance

| Setting | Effect |
|---------|--------|
| `--root` | document root (resolved once, cached) |
| `--php-ini key=value` | php.ini overrides via `PHP_INI_SCAN_DIR` (cached per set) |
| `--features fcgi` | use persistent FastCGI backend instead of CGI spawn |
| `resolve_root_symlink` | symlinks resolved once at startup |

## See also

- [`php-abi-structures.md`](php-abi-structures.md) — PHP SAPI structs and how to
  extract peak throughput (opcache, NTS multi-process, zero-copy)
- [`embedded-php.md`](embedded-php.md) — in-process PHP runtime (no CGI spawn)
- [`architecture.md`](architecture.md) — module breakdown
- [`http3.md`](http3.md) — HTTP/3 transport
- [`configuration.md`](configuration.md) — config formats