# Error pages (40x / 50x)

LarvaPHP serves one shared, self-contained HTML page for every `4xx` and `5xx`
response. The goal is a page that still looks intentional when PHP, the CMS, or
the upstream is unhealthy — and that adds nothing to the normal request path.

## Design

- **One dynamic template.** The status code selects the title, message and
  client/server visual family. There are no per-code template files.
- **Zero external assets.** One inline `<style>` block. No stylesheets, fonts,
  images, scripts or preloads. The page is a single small response.
- **Terminal-neon look.** Monospace system fonts, scanline texture, window
  chrome, oversized gradient status code, and a deterministic technical trace.
- **Accessible by default.** Dark background with high-contrast text, one `h1`
  that includes the status code, `≥48×48` action targets, visible focus rings,
  `lang="vi"`, and no `aria-hidden` on focusable elements.
- **No request data.** Host, path, cookies, user agent and other caller-controlled
  values are never interpolated into the page, so error responses cannot leak
  request state or create cross-connection contamination.

## Covered response paths

| Source | Behaviour |
| --- | --- |
| Unregistered server | `503` page |
| Missing script / missing static file / sensitive file | `404` / `403` page |
| Static pipeline failures and validators | `405`, `412`, `416`, `500` page |
| Rate limit | `429` page with preserved `Retry-After` |
| Request body over limit | `413` page |
| PHP execution failure | `500` page |
| PHP/CGI `Status:` header in `400..=599` | PHP body replaced with the shared page |
| Jankx edge-cache entry with an error status | cached body replaced with the shared page |
| HTTP/3 worker error / full queue | `500` / `503` page |
| ACME challenge miss | `404` page |

`3xx` responses, including the canonical host redirect, are untouched.

The same renderer is used by HTTP/1.1, HTTP/2 and HTTP/3 through
`Server::handle_request_parts()` and the Hyper adapter.

## Header policy

Every generated error page carries:

- `content-type: text/html; charset=UTF-8`
- `content-length` describing the **error page**, including on `HEAD`
- `cache-control: no-store`
- `server: LarvaPHP`
- `x-content-type-options: nosniff`
- `x-frame-options: DENY`
- `referrer-policy: no-referrer`
- `alt-svc` when HTTP/3 is enabled

Semantic headers from the original response are preserved (for example
`Retry-After` and `WWW-Authenticate`), while representation headers describing
the discarded body (`Content-Type`, `Content-Length`, `Content-Encoding`, `ETag`,
`Last-Modified`, `Expires`, `Age`, `Link`, `X-Sendfile`) are dropped.

HTTP/3 responses now preserve an explicit `Content-Length` instead of always
recomputing it from the transmitted body. That keeps `HEAD` consistent with the
Hyper path.

## Copy selection

Common codes have dedicated Vietnamese copy (`400`, `401`, `403`, `404`, `405`,
`408`, `409`, `410`, `411`, `412`, `413`, `414`, `415`, `416`, `418`, `422`,
`429`, `451`, `500`, `501`, `502`, `503`, `504`, `505`, `507`). Any other `4xx`
or `5xx` falls back to a generic message while still showing the exact status
code and canonical reason phrase.

## Verification

```bash
cargo fmt --all -- --check
cargo test --lib
cargo test --lib -p larva-php --features embedded,http3,acme,mercure,tls
```

The suite covers self-contained HTML output, client/server visual switching,
HTML escaping, HEAD content length, stale-header removal, PHP status
replacement, HTTP/3 static delivery, and concurrent request isolation.

## Rollback

This feature needs no configuration change. Reverting the commits restores the
previous plain-text error bodies; on a deployed server, replace the binary with
the pre-deploy backup before restarting.
