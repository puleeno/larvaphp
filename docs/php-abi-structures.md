# PHP ABI Structures and Performance Exploitation

This document explains the PHP 8.3 ABI structures that LarvaPHP's embedded
runtime (`larva_embed.c` / `larva_embed.h`) depends on, the SAPI lifecycle they
drive, and the concrete techniques used to extract maximum performance out of
an in-process PHP runtime on Windows (NTS `php8.dll`).

The material here is the reference companion to [`embedded-php.md`](embedded-php.md)
(how the embedding works) and [`performance.md`](performance.md) (measured
numbers). It answers two questions:

1. **What structures does the shim mirror, and why are they laid out the way
   they are?**
2. **How do you get the most throughput per core?**

---

## 1. Why a struct mirror at all

`larva_embed.c` links against **no** PHP headers and **no** import library. It
loads `php8.dll` at runtime with `LoadLibraryA` and resolves every symbol with
`GetProcAddress` (see [`embedded-php.md`](embedded-php.md) §"Why a C Shim").

That means the shim cannot `#include "SAPI.h"` to learn struct layouts. It must
re-declare them. Because PHP uses the **system C ABI** (cdecl on x64 Windows,
no hidden calling convention differences for these plain functions), a struct
mirror that matches the field order, types, and alignment of the real PHP
struct is binary-compatible: the same bytes PHP writes into the struct are read
back by the shim, and vice versa.

Two rules make the mirrors safe:

- **Match the exact upstream layout.** Field order and widths come from
  `main/SAPI.h` and `Zend/zend_stream.h` of the *same* PHP version
  (8.3.x). A struct from a different version (e.g. 8.4) can silently misalign
  later fields.
- **Only model what you touch.** `sapi_globals_struct` has dozens of fields the
  shim never reads, so it models just the prefix it needs
  (`sapi_globals_partial`) — `server_context` followed by `request_info`. This
  is safe because those two fields are the first members of the real struct.

---

## 2. The mirrored structures

### 2.1 `sapi_module_struct`

The central descriptor. One instance lives for the life of the process. It
carries the SAPI's identity (`name`, `pretty_name`) and a table of **callbacks**
the Zend engine invokes at well-defined points. The shim fills it once in
`larva_embed_init()` and passes it to `sapi_startup()`.

Layout (PHP 8.3, `main/SAPI.h`), with the callbacks the shim implements
annotated:

| Field | Type | Shim role |
|-------|------|-----------|
| `name` | `char *` | **`"frankenphp"`** — must be in opcache's `supported_sapis` list (see §4.1). |
| `pretty_name` | `char *` | `"LarvaPHP Embedded"` (cosmetic, appears in `phpcredits()`). |
| `startup` | `fn(sapi_module_struct *) -> int` | left NULL (PHP provides default). |
| `shutdown` | `fn(sapi_module_struct *) -> int` | left NULL. |
| `activate` | `fn() -> int` | left NULL. |
| `deactivate` | `fn() -> int` | left NULL. |
| `ub_write` | `fn(const char *, size_t) -> size_t` | **`my_ub_write`** — captures script output into a growable buffer (§3.1). |
| `flush` | `fn(void *) -> void` | `my_flush` — no-op (we never stream incrementally). |
| `send_headers` | `fn(void *) -> int` | **`my_send_headers`** — returns `SAPI_HEADER_DO_SEND` to force per-header delivery. |
| `send_header` | `fn(void *, void *) -> void` | **`my_send_header`** — captures status line + each header line. |
| `read_post` | `fn(char *, size_t) -> size_t` | **`my_read_post`** — streams the request body for `php://input` / POST parsing. |
| `read_cookies` | `fn() -> char *` | **`my_read_cookies`** — hands PHP the `Cookie` header value (ownership note in §3.2). |
| `register_server_variables` | `fn(void *) -> void` | **`my_register_server_variables`** — populates `$_SERVER` (§3.3). |
| `log_message` | `fn(const char *, int) -> void` | `my_log_message` — writes to stderr. |
| `php_ini_path_override` | `char *` | full path to `php.ini` when explicitly resolved. |
| `ini_entries` | `const char *` | newline-separated INI directives (opcache etc.); **set after `sapi_startup`**, before `php_module_startup` (§4.1). |
| `input_filter_init` | `fn() -> unsigned int` | left NULL. |

The tail of the struct (fields after `ini_entries`, plus
`additional_functions`, `get_request_time`, `terminate_process`, etc.) is left
zeroed; the shim never needs them and PHP tolerates NULL callbacks.

### 2.2 `sapi_request_info`

Per-request state, reached through `SG(request_info)`. The shim populates it
**before** `php_request_startup()` in `larva_embed_execute_request()`.

| Field | Type | Populated with |
|-------|------|----------------|
| `request_method` | `const char *` | `req->method` (`"GET"`, `"POST"`, …). |
| `query_string` | `char *` | `req->query_string` (or NULL). PHP splits this into `$_GET`. |
| `cookie_data` | `char *` | the malloc'd cookie copy (PHP may take ownership). |
| `content_length` | `int64_t` | `req->content_length`. |
| `path_translated` | `char *` | `script_path` — becomes `SCRIPT_FILENAME` / `PATH_TRANSLATED`. |
| `request_uri` | `char *` | `req->request_uri`. |
| `request_body` | `void *` | left NULL; body is delivered via the `read_post` callback instead. |
| `content_type` | `const char *` | `req->content_type`. |
| `headers_only` | `unsigned char` | `0` (we want a body). |
| `no_headers` | `unsigned char` | `0` (we want headers). |
| `headers_read` | `unsigned char` | `0` before startup; PHP sets it to 1 after calling `read_post` once. |
| `auth_*`, `argv*`, `current_user*`, `proto_num` | — | left zeroed (not used). |

### 2.3 `sapi_globals_partial`

The shim never dereferences the real `sapi_globals_struct` wholesale. It models
only the **prefix** it accesses — `server_context` then `request_info`:

```c
typedef struct {
    void *server_context;          /* real struct: httpd/worker context ptr   */
    sapi_request_info request_info;/* the per-request fields above             */
    /* ... many more fields (sapi_headers, etc.) not modelled ...             */
} sapi_globals_partial;
```

`g_sapi_globals` is the **address** returned by `GetProcAddress(php8, "sapi_globals")`
(it is a data export, not a function — `GetProcAddress` on it returns a
pointer to the global). The shim reads/writes
`g_sapi_globals->request_info` and sets `g_sapi_globals->server_context`.

Setting `server_context` to a non-NULL dummy is **required**: `sapi_activate()`
only runs the POST/cookie activation path when `server_context != NULL`. Leaving
it NULL makes `php_request_startup()` skip `$_POST` and `$_COOKIE` entirely.

### 2.4 `zend_file_handle`

Passed to `php_execute_script()` to tell it which file to run. In 8.3 the handle
is an 80-byte, 8-byte-aligned structure (verified against
`Zend/zend_stream.h`). The shim uses the **stream** variant of the `handle`
union:

```c
zend_file_handle fh;
memset(&fh, 0, sizeof(fh));
p_zend_stream_init_filename(&fh, script_path); /* fills filename + type */
fh.primary_script = 1;                          /* it's the entry script   */
p_zend_stream_open(&fh);                        /* open/validate the file  */
p_php_execute_script(&fh);
```

`primary_script = 1` matters: it tells the stream layer this is the top-level
script (affects `__FILE__` and error reporting). `zend_stream_init_filename`
sets `type` and `filename`; `zend_stream_open` is what actually reads/validates
the file and populates the stream callbacks if needed.

### 2.5 `sapi_header_struct`

Delivered to `send_header` one at a time. The shim reads `header` (the full
line, e.g. `"HTTP/1.0 200 OK"` or `"Content-Type: text/html"`) and
`header_len`, copies the line into its own buffer, and appends `'\n'`.

The first header line is the status line. `my_send_header` peeks for the
`HTTP/` prefix and extracts the status code with `atoi` after the first space.
`send_headers` must return `SAPI_HEADER_DO_SEND` (value `2`) so PHP actually
invokes `send_header` per line rather than assuming it sent them itself.

---

## 3. Callback semantics the shim relies on

### 3.1 Output capture — `my_ub_write`

The engine calls `ub_write(str, len)` for every `echo`, `print`, buffered
flush, etc. The shim appends into a **growable heap buffer** (`g_output`) with
doubling growth, NUL-terminates, and reports the true length. Because the buffer
is realloc'd, it is **freed by the shim** and the pointer is handed back to Rust
via `larva_embed_response.body`; Rust copies it out and the shim frees it on
`larva_embed_response_free()`.

For the CLI-like `larva_embed_execute()` (no request context), the output is
copied into a caller buffer that the caller grows and retries if it exactly
filled (the shim NUL-terminates only when it fits and reports true length).

### 3.2 Cookie ownership — `my_read_cookies`

`sapi_activate()` calls `read_cookies()` and stores the return value into
`SG(request_info).cookie_data`. **PHP may take ownership and may not free it.**
The shim keeps a copy (`g_cookie_copy`), tracks the pointer it handed over
(`g_cookie_orig`), and frees whichever one PHP did not consume after
`php_request_shutdown()`. This avoids both a leak and a double-free.

### 3.3 `$_SERVER` population — `my_register_server_variables`

Called once per request with a `track_vars` array. The shim:

1. Adds variables derived from `SG(request_info)` — `REQUEST_METHOD`,
   `QUERY_STRING`, `CONTENT_TYPE`, `PATH_TRANSLATED`, `REQUEST_URI`,
   `SCRIPT_FILENAME`.
2. Iterates the `g_server_vars` array supplied by Rust (HTTP_* headers,
   `SCRIPT_NAME`, `SERVER_NAME`, `SERVER_PORT`, `DOCUMENT_ROOT`, …) and adds
   each with `add_assoc_stringl_ex` (a length-aware insert, so values may
   contain embedded NULs / non-ASCII bytes).

Rust builds these variables from the request headers in `embed_php.rs::execute_request`
(converting `Host` → `HTTP_HOST`, etc.) and always adds a standard block.

---

## 4. Getting the most performance

Performance in this runtime comes from three levers, in order of impact:

1. **Never recompile bytecode** (opcache).
2. **Run PHP in-process** (no process spawn) and **serialize as little as possible**.
3. **Keep the per-request C↔Rust boundary cheap** (zero-copy where possible,
   no per-request allocation churn).

### 4.1 opcache is the single biggest win

A cold embedded runtime compiles every script from source on first hit. For a
CMS like WordPress that is the dominant cost. The shim configures opcache via
`ini_entries` (see `runtime/php_ini.rs::IniEntriesResolver::entries`):

```ini
opcache.enable=1
opcache.enable_cli=1
opcache.validate_timestamps=0      ; never stat files → biggest single win
opcache.revalidate_freq=0
opcache.memory_consumption=512     ; MB of SHM for bytecode
opcache.interned_strings_buffer=32 ; MB for deduplicated strings
opcache.max_accelerated_files=100000
opcache.save_comments=1
opcache.fast_shutdown=1
opcache.file_cache=<temp>/larva-php-opcache
opcache.file_cache_only=0          ; prefer SHM, fall back to file cache
```

Key rules:

- **Do not add `zend_extension=php_opcache.dll`** — the system `php.ini` already
  loads opcache (`zend_extension=opcache`); adding it again causes
  "Cannot load Zend OPcache — it was already loaded".
- **Set `ini_entries` between `sapi_startup` and `php_module_startup`.** As
  noted in §2.1, `sapi_startup()` nulls `sf->ini_entries` before copying to the
  global `sapi_module`; `php_module_startup()` re-copies `sapi_module = *sf`.
  Set it in the wrong place and the directives are silently dropped.
- **`opcache.validate_timestamps=0`** means the cache is never invalidated by
  file mtime — correct for production. During development, re-enable it so edits
  are picked up.
- **`opcache.file_cache`** gives cross-process reuse: when `--workers N` is used
  (see §4.3), compiled bytecode is persisted to disk and reloaded by worker
  processes that did not compile the script themselves.

### 4.2 The SAPI name gates opcache

opcache's `accel_find_sapi()` (in `ext/opcache/ZendAccelerator.c`) only starts
if `sapi_module.name` is in its `supported_sapis` list. If the name is not
recognised, opcache logs *"Opcode Caching is only supported in ... SAPIs"* and
runs **uncached** — every request recompiles. The shim therefore names itself
`"frankenphp"` (which is in the list). This is a one-line change with a
**3–15×** effect on cache-hit workloads.

### 4.3 Parallelism: NTS → multi-process

`php8.dll` is **NTS (non-thread-safe)**. Multiple OS threads cannot call into
PHP concurrently in one process; doing so corrupts the engine. Two strategies:

| Mode | Flag | Concurrency | Notes |
|------|------|-------------|-------|
| Single-process | `--embedded` | serialized via `Mutex` | simplest; fine for testing and low traffic. |
| Multi-process | `--embedded --workers N` | **true N-way** | each worker embeds its own `php8.dll`; master dispatches round-robin over shared memory. |

In multi-process mode:

- Each worker has its own `php8.dll` instance, its own `PHP_LOCK` mutex, and
  its own request/response buffers — so the mutex only serializes *within* a
  worker, and the N workers run genuinely in parallel.
- **opcache shared memory is named** on Windows, so all workers share one
  compiled-bytecode cache. The first worker to compile a script populates the
  cache; the rest are instant hits.
- The **master never runs PHP** — it is pure Rust + Tokio (crash-proof). A
  segfault, `exit()`, or OOM in one worker kills only that worker; the master
  respawns it.

This is the "fault-isolated, N-way parallel" model described in
[`runtime-architecture.md`](runtime-architecture.md).

### 4.4 Minimize the cost inside the mutex

Because the embedded path is serialized by `PHP_LOCK`, time spent inside the
lock is time other requests wait. Reduce it:

- **Build the `$_SERVER` C-string array once, outside the lock where possible.**
  In `embed_php.rs::execute_request`, header → `HTTP_*` conversion and the
  standard-variable block are pure Rust; keep them out of any locked region and
  only hold the lock around the actual `larva_embed_execute_request` FFI call.
- **Reuse buffers across requests.** The shim already grows `g_output` /
  `g_headers` and only frees them at shutdown or when handing them to Rust;
  avoid forcing a fresh allocation per request.
- **Avoid copies on the response path.** `larva_embed_response` hands Rust
  *owned* pointers; Rust copies once (`slice::to_vec`) then frees the C buffer.
  For very large responses, prefer moving/copying once rather than double
  buffering.

### 4.5 Keep the C↔Rust boundary thin

- **Query-string splitting and header parsing are pure Rust** and cheap; they
  do not touch the PHP lock. Keep them there.
- **Pass `server_vars` as a flat array of `{key,value}` pairs** (see
  `larva_embed_server_var`), not as nested structures the shim must walk.
- **Set `server_context` to non-NULL** (§2.3) so `$_POST` / `$_COOKIE` are
  populated without extra round-trips.
- **Use `add_assoc_stringl_ex`** (length-aware) rather than NUL-terminated
  inserts when populating `$_SERVER`, so binary/UTF-8 values survive intact.

### 4.6 Stack size

PHP's compiler is recursive and uses a large stack. `cargo test` threads
default to 2 MB, which can overflow during compilation. The embedded tests
(`embed_php.rs` `run_on_big_stack`) spawn a dedicated **64 MB stack** thread and
serialize with a test mutex. If you drive the embedded runtime from your own
short-lived threads (rather than Tokio worker threads with a large stack), give
them a correspondingly large stack.

### 4.7 Linux parity

The shim is Windows-first (`LoadLibraryA`/`GetProcAddress`). On Linux the same
structure mirrors work with `dlopen`/`dlsym`; replace only the dynamic-loading
calls. The SAPI sequence, callback semantics, and opcache configuration are
identical. For highest throughput on Linux, pair the embedded runtime with the
system `php-fpm` FastCGI backend (`--features fcgi`) — see
[`performance.md`](performance.md) §"Linux".

---

## 5. Checklist: extracting peak performance

- [ ] opcache enabled via `ini_entries` (not `zend_extension=`).
- [ ] SAPI named `"frankenphp"` (or another `supported_sapis` entry).
- [ ] `opcache.validate_timestamps=0` in production.
- [ ] `opcache.file_cache` set for cross-worker reuse.
- [ ] Multi-process (`--workers N`) for parallelism with NTS.
- [ ] `server_context` non-NULL so POST/cookie activation runs.
- [ ] Large stack threads when driving PHP off Tokio's runtime.
- [ ] `$_SERVER` build outside the PHP lock; only the FFI call inside it.
- [ ] Length-aware `add_assoc_stringl_ex` for binary-safe variables.

---

## 6. See also

- [`embedded-php.md`](embedded-php.md) — how embedding works, critical fixes.
- [`runtime-architecture.md`](runtime-architecture.md) — 2-layer master/worker model.
- [`performance.md`](performance.md) — measured numbers and per-request breakdown.
- [`feature-comparison.md`](feature-comparison.md) — vs FrankenPHP / mod_php / php-fpm.