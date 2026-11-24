# Troubleshooting

## WordPress login loops back to `wp-login.php?reauth=1` on HTTP/2

### Symptoms

- Submitting the WordPress login form in a browser redirects back to
  `wp-login.php?...&reauth=1` instead of `/wp-admin/`.
- The same login works with `curl`, or in Chrome when HTTP/2 is disabled
  (`--disable-http2`).
- PHP receives `wordpress_test_cookie` and `wordpress_logged_in_*`, but the
  `wordpress_sec_*` cookie is missing from `$_COOKIE`.

### Root cause

LarvaPHP uses `hyper_util::server::conn::auto`, which negotiates HTTP/2 via
ALPN. Browsers may split the `Cookie` header into several HTTP/2 header fields.
The request handler copied every header into `PreparedEnv`, a
`HashMap<Vec<u8>, Vec<u8>>`, so duplicate `cookie` keys overwrote each other and
only the last field survived.

WordPress runs over HTTPS, so `is_ssl()` is true. `auth_redirect()` calls
`wp_validate_auth_cookie( '', '' )`; with an empty scheme and HTTPS, WordPress
reads `SECURE_AUTH_COOKIE` (`wordpress_sec_*`), not `LOGGED_IN_COOKIE`. Because
LarvaPHP dropped all but the last cookie field, that auth cookie never reached
PHP and WordPress forced a re-authentication redirect.

### Fix

`combine_cookie_header_values()` in `crates/larva-php/src/server.rs` joins all
`cookie` header fields into a single `; `-separated value. The handler skips
individual `cookie` fields when building `PreparedEnv` and inserts the combined
value once, so PHP always gets a complete `HTTP_COOKIE`.

A regression test (`combine_cookie_header_values_joins_repeated_fields`) covers
single, repeated and absent Cookie headers.

### How to diagnose

Send repeated Cookie fields directly and inspect what PHP sees:

```bash
curl -sS --http2 \
  -H 'Cookie: a=1' -H 'Cookie: b=2' -H 'Cookie: c=3' \
  https://example.com/cookie-dump.php
```

Before the fix, only `c=3` arrived; after the fix, PHP sees
`a=1; b=2; c=3`. Comparing a browser run with and without `--disable-http2`
also isolates the issue to HTTP/2 header handling.

### Verification

After rebuilding, installing and restarting the service:

- Browser (HTTP/2 enabled) login ends at `/wp-admin/` with the Dashboard title.
- Both `wordpress_sec_*` and `wordpress_logged_in_*` cookies are present.
- `cargo test --release -p larva-php --features embedded,acme,tls,mercure --lib`
  passes, including the new cookie test.

## Related: "Cookies are blocked" when opening `www.example.com`

WordPress generated absolute form actions for the bare host while the login
test cookie was scoped to `www`, so the POST arrived without the test cookie.
LarvaPHP now redirects `www.` to the bare host (except ACME HTTP-01 paths) to
keep the browser on one origin. See `canonical_redirect_location()` in
`server.rs`.

## Related: `Uncaught ReferenceError: _ is not defined` on `wp-login.php`

This one is **not** a server bug. The Jankx theme's `PerformanceService`
adds `defer` to `underscore.min.js` but not to `wp-util.min.js`, so `wp-util`
runs before `_` exists. On the affected site it was disabled with the
WordPress option `jankx_perf_defer_scripts = no` rather than by patching theme
code or `wp-config.php`.
