# Embedded PHP via C Shim

LarvaPHP runs PHP **in-process** (like `mod_php` or FrankenPHP) instead of
spawning a `php-cgi.exe` process per request. This eliminates the ~80 ms
process-spawn overhead and gives **3–4× faster** response times for
lightweight scripts and **15–30 %** for database-heavy apps like WordPress.

> On Linux/macOS the release build uses a separate real SAPI shim
> (`larva_embed_unix.c`) linked against the system PHP embed library; there are
> no no-op stubs. See [`build-release-linux.md`](build-release-linux.md) for
> requirements and verification.

## Why a C Shim?

Calling the PHP C API directly from Rust (`larva-php-sys`) crashed inside
`sapi_startup()` with `0xC0000005` (access violation) on Windows. The crash
is an MSVC/Rust ABI/CRT interaction that could not be resolved with struct
padding, calling conventions, or stack alignment fixes.

The solution — the same one FrankenPHP uses — is a **C shim** (`larva_embed.c`)
compiled with the same MSVC toolchain as `php8.dll`. The shim:

1. Resolves all PHP entry points dynamically via `LoadLibraryA("php8.dll")` +
   `GetProcAddress` — **no import library or PHP headers needed**.
2. Runs the full SAPI sequence: `sapi_startup` → `php_module_startup` →
   `php_request_startup` → `php_execute_script` → `php_request_shutdown`.
3. Implements SAPI callbacks (`ub_write`, `send_header`, `read_post`,
   `read_cookies`, `register_server_variables`) to capture output and
   populate `$_SERVER`, `$_GET`, `$_POST`, `$_COOKIE`.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Tokio (56 threads)  —  HTTP connections, static files      │
└──────────────────────────┬──────────────────────────────────┘
                           │  execute_php_cgi()
┌──────────────────────────▼──────────────────────────────────┐
│  Master process (Rust)                                       │
│  • Round-robin dispatch → worker processes via shared memory │
│  • Never runs PHP (fault isolation)                          │
└──────┬──────────┬──────────┬──────────┬─────────────────────┘
       │          │          │          │
   ┌───▼───┐  ┌──▼───┐  ┌──▼───┐  ┌──▼───┐
   │Worker0│  │Wrkr1 │  │Wrkr2 │  │Wrkr3 │  ← separate processes
   │PHP8.3 │  │PHP8.3│  │PHP8.3│  │PHP8.3│    each embeds php8.dll
   │opcache│  │opcach│  │opcach│  │opcach│  ← shared via named SHM
   └───────┘  └──────┘  └──────┘  └──────┘
```

### Multi-process parallelism (NTS)

PHP 8.3.33 NTS (`php8.dll`) is **not thread-safe** — multiple threads cannot
call PHP concurrently in the same process. LarvaPHP solves this with a
**multi-process** model:

- **1 process** (`--workers 0`): embedded PHP runs in the main process,
  serialised by a `Mutex`. Safe but single-threaded.
- **N processes** (`--workers N`): each worker process embeds its own
  `php8.dll` instance. Tokio dispatches requests in parallel → **true N-way
  parallelism** with no thread-safety issues.

opcache uses **named shared memory** on Windows, so all workers share the
same compiled-bytecode cache. The first worker to compile a script caches it;
all other workers benefit.

## Usage

### Build

```powershell
# Debug build with embedded PHP
cargo build --features embedded

# Release build
cargo build --release --features embedded
```

### Run

```powershell
# Single-process embedded (serialised, good for testing)
larva-php --root C:\path\to\app --addr 127.0.0.1:8000 --embedded

# Multi-process embedded (parallel, production)
larva-php --root C:\path\to\app --addr 127.0.0.1:8000 --embedded --workers 4
```

The `--embedded` flag sets `LARVA_PHP_EXECUTOR=ffi`, which makes the worker
use the embedded FFI executor instead of spawning `php-cgi.exe`.

### Test flags (hidden)

```powershell
# Run a script twice (cold + warm) to verify opcache caching
larva-php --ffi-test script.php

# Run a script with mock CGI request data ($_SERVER, $_GET, $_COOKIE)
larva-php --ffi-request-test script.php
```

## Performance

| Workload | Embedded (warm) | CGI spawn | Speedup |
|----------|----------------|-----------|---------|
| Simple script (`echo`) | 21–28 ms | ~80 ms | **3–4×** |
| WordPress homepage | 240 ms | ~280–350 ms | **15–30 %** |
| 4× concurrent (500 ms each) | 678 ms total | ~2040 ms | **3×** |

The first request is slower (~600–820 ms) because it includes PHP
initialisation and opcache compilation. Subsequent requests are cache hits.

## Implementation Details

### C Shim (`larva_embed.c`)

The shim defines minimal ABI-compatible struct mirrors for PHP 8.3:

| Struct | Purpose |
|--------|---------|
| `sapi_module_struct` | SAPI callbacks and configuration |
| `sapi_request_info` | Request method, URI, query string, cookies, content type |
| `sapi_globals_partial` | Access to `server_context` and `request_info` |
| `zend_file_handle` | Script file handle (80 bytes, align-8) |

PHP symbols are resolved dynamically:

```c
g_php = LoadLibraryA("php8.dll");
p_sapi_startup = GetProcAddress(g_php, "sapi_startup");
p_php_module_startup = GetProcAddress(g_php, "php_module_startup");
/* ... */
g_sapi_globals = GetProcAddress(g_php, "sapi_globals");
```

### Critical Fixes

#### 1. `ini_entries` applied after `sapi_startup`

`sapi_startup()` nulls `sf->ini_entries` before copying the struct to the
global `sapi_module`. `php_module_startup()` later re-copies the struct
(`sapi_module = *sf`). Therefore `ini_entries` must be set **after**
`sapi_startup` but **before** `php_module_startup`:

```c
p_sapi_startup(&sapi);
sapi.ini_entries = ini_entries;  // AFTER sapi_startup
int rc = p_php_module_startup(&sapi, NULL);
```

#### 2. SAPI name for opcache

opcache's `accel_find_sapi()` checks `sapi_module.name` against a
`supported_sapis` list. If the name is not in the list, opcache refuses to
start with "Opcode Caching is only supported in ... SAPIs". The fix is to use
`"frankenphp"` (which is in the list):

```c
sapi.name = "frankenphp";  // not "larva-embed"
```

#### 3. Cookie data lifecycle

`sapi_activate()` calls `read_cookies()` and assigns the return value to
`SG(request_info).cookie_data`. PHP may not free it. The shim tracks the
pointer and frees it after `php_request_shutdown`:

```c
static char *my_read_cookies(void) {
    char *ret = g_cookie_copy;
    g_cookie_orig = g_cookie_copy;  // track for later free
    g_cookie_copy = NULL;
    return ret;
}
```

#### 4. NULL body pointer on PHP fatal errors

When PHP has a fatal error, no output is produced and the response body
pointer is NULL. `slice::from_raw_parts` panics on NULL even for 0-length
slices. The Rust side checks for NULL before slicing.

### opcache Configuration

opcache is loaded by the system `php.ini` (`zend_extension=opcache`). The shim
does **not** add `zend_extension=php_opcache.dll` to avoid double-loading.
Instead, it configures opcache via `ini_entries`:

```ini
opcache.enable=1
opcache.enable_cli=1
opcache.validate_timestamps=0
opcache.revalidate_freq=0
opcache.memory_consumption=512
opcache.interned_strings_buffer=32
opcache.max_accelerated_files=100000
opcache.file_cache=<temp>/larva-php-opcache
```

### Thread Safety (NTS)

The NTS `php8.dll` is not thread-safe. The Rust side uses a `Mutex` to
serialise all embedded PHP access:

```rust
static PHP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn execute_request(req: &Request) -> Result<Response> {
    ensure_init()?;
    let _guard = php_lock().lock().unwrap_or_else(|e| e.into_inner());
    // ... call C shim ...
}
```

In multi-process mode (`--workers N`), each worker has its own mutex and PHP
instance, so requests are processed in parallel across workers.

## Files

| File | Purpose |
|------|---------|
| `crates/larva-php-sys/src/larva_embed.c` | C shim (SAPI callbacks, request context, symbol resolution) |
| `crates/larva-php-sys/src/larva_embed.h` | Request/response API header |
| `crates/larva-php-sys/build.rs` | Compiles `larva_embed.c` via `cc` crate |
| `crates/larva-php-sys/src/lib.rs` | FFI declarations + `LarvaEmbedRequest`/`LarvaEmbedResponse` structs |
| `crates/larva-php/src/embed_php.rs` | Rust API (mutex, `ensure_init`, `execute_request`) |
| `crates/larva-php/src/runtime/executor.rs` | `FfiExecutor` uses `execute_request` |
| `crates/larva-php/src/server.rs` | `--embedded` path in `execute_php_cgi` |
| `crates/larva-php/src/runtime/php_ini.rs` | opcache directives |
| `crates/larva-php/src/main.rs` | `--embedded`, `--ffi-test`, `--ffi-request-test` flags |

## Limitations

- **NTS only**: requires `php8.dll` (NTS). For thread-based parallelism,
  a ZTS build (`php8ts.dll`) would be needed.
- **Windows-first**: the C shim uses `LoadLibraryA`/`GetProcAddress` (Windows
  API). Porting to Linux/macOS would use `dlopen`/`dlsym`.
- **Serialized within a process**: single-process embedded mode is
  single-threaded (mutex). Use `--workers N` for parallelism.
- **opcache shared memory**: workers share bytecode cache via named shared
  memory on Windows. Cold start compiles all scripts.

## See also

- [`php-abi-structures.md`](php-abi-structures.md) — the mirrored PHP structs,
  callback semantics, and peak-throughput techniques
- [`performance.md`](performance.md) — measured numbers and per-request breakdown
- [`runtime-architecture.md`](runtime-architecture.md) — master/worker model