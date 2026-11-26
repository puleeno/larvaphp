# Connection and request isolation

Multi-connection bugs are usually not CPU/RAM bugs. They appear as one request
seeing another request's headers, cookie, cache entry, or cancellation state.
This document records the invariants LarvaPHP now tests directly.

## Invariants

1. Every request owns a fresh `PhpContext`. Its `env`, `request_uri`,
   `query_string`, `request_method`, `server_name`, `headers_written`, `is_done`
   and `client_had_closed` belong to that request only.
2. `Host` without a port must still produce the real hostname. Falling back to
   `localhost` would mix virtual hosts and edge-cache keys between clients.
3. The in-memory gzip cache must distinguish payloads that share a path but
   have different bytes, even when their lengths are equal.
4. `Server` is shared read-only across connections. Mutable caches are keyed
   (`DashMap`) or intentionally scoped (rate limiter per client IP).
5. `handle_request_parts()` must return only the response built from the
   headers/body passed to that call, even when many threads call it at once.
6. A slow or stalled connection must never block the TCP accept loop.

## Tests

Located in `crates/larva-php/src/context.rs` and
`crates/larva-php/src/server.rs`:

| Test | Guards |
|------|--------|
| `concurrent_contexts_keep_isolated_request_state` | 16 concurrent requests with unique method, URI, Host, `x-request-id` and cookie; asserts no neighbour value leaks into a context |
| `contexts_do_not_share_env_or_done_state` | Mutating/closing one context does not affect another |
| `host_without_port_keeps_the_real_hostname` | Regression for the `Host: example.com` → `localhost` bug |
| `host_with_ipv6_and_port_is_split_correctly` | `[::1]:8443`, `[::1]`, `example.com:8080`, `example.com` |
| `rate_limiter_state_is_scoped_to_each_ip` | Counters are per IP, not shared across clients |
| `cached_gzip_bytes_isolates_equal_length_payloads` | 12 concurrent keys with same-length, different payloads all decompress to their own data |
| `cached_gzip_bytes_invalidates_same_length_content_change` | A rewritten Jankx cache file with the same length does not serve stale HTML |
| `concurrent_handle_request_parts_keep_each_response_isolated` | 16 concurrent static-file requests return their own body |
| `concurrent_handle_request_parts_isolate_request_headers` | 12 concurrent requests with per-request headers return their own body |

The stalled-TLS-handshake accept-loop regression is covered operationally in
[`http3-architecture-and-hang-bugs.md`](http3-architecture-and-hang-bugs.md#10-bug-12-tls-handshake-chặn-vòng-accept-tcp):
a raw TCP client that never sends `ClientHello` must not stop another client
from getting `200`.

## Running

```bash
export PHP_LIB_DIR=/usr/lib
export PHP_EMBED_LIB=php8.3
export PHP_INCLUDES="$(php-config --includes)"

cargo test -p larva-php --lib
cargo test -p larva-php --lib --features embedded,http3,acme,mercure,tls
```

Latest results: **237 passed** with default features, **242 passed** with the
full production feature set. Both runs use real threads and real filesystem
temp directories; no mocks hide shared state.

## Fixes found by these tests

- `PhpContext::from_request()` used `split_once(':')`, so `Host: nghesachxua.vn`
  became `SERVER_NAME=localhost`. Now a dedicated `split_host_header()` handles
  no-port and bracketed IPv6 Host values.
- `cached_gzip_bytes()` keyed only by source path and size. A Jankx page
  rewritten with the same length could serve old HTML to every connection. The
  key now includes an FNV-1a hash of the payload.
