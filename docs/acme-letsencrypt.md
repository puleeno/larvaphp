# Let's Encrypt / ACME Support

LarvaPHP can obtain and automatically renew TLS certificates from Let's Encrypt
using the **ACME v2 protocol (RFC 8555)** with the **HTTP-01 challenge** — no
external ACME client (certbot, lego, etc.) is required.

The feature is gated behind `--features acme` and works hand-in-hand with the
HTTP/3 (QUIC) server (`--features http3`).

```
┌─────────────────────────────────────────────────────────────────────┐
│  LarvaPHP                                                           │
│  ┌──────────────┐  ┌─────────────────────────────────────────────┐ │
│  │ HTTP server  │  │ ACME client (Rust, reqwest)                 │ │
│  │ (TCP/TLS)    │  │ • directory → register → order →            │ │
│  │              │  │   challenge → finalize → download           │ │
│  │ serves       │  │ • HTTP-01 tokens at /.well-known/...        │ │
│  │ /.well-known │  │ • openssl for keys/CSRs/JWS signatures      │ │
│  │ /acme-challenge│ │ • hourly renewal loop (openssl -checkend)  │ │
│  └──────────────┘  └─────────────────────────────────────────────┘ │
│                            │                                       │
│              Let's Encrypt validates → issues cert                 │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Requirements

- **Rust feature:** `--features acme` (pulls in `reqwest` + `base64`).
- **openssl CLI** on the host (confirmed available: `OpenSSL 3.6.3`). Used for
  RSA key generation, CSR creation, and RS256 JWS signing.
- **Port 80 reachable** from the public internet (HTTP-01 validation fetches
  `http://<domain>/.well-known/acme-challenge/<token>`).
- **HTTP/3 feature** recommended (`--features http3,acme`) so the issued
  certificate is actually served over QUIC/TLS.

---

## CLI flags

| Flag | Short | Meaning |
|------|-------|---------|
| `--lets-encrypt` | `-L` | Enable ACME certificate provisioning. |
| `--acme-email` | | Contact email for the Let's Encrypt account (required). |
| `--acme-domains` | | Comma-separated domains for the certificate (required). |
| `--acme-dir` | | Directory to store account key + certificates. Default: `~/.larva-php/letsencrypt`. |
| `--acme-challenge-dir` | | Directory where HTTP-01 tokens are written. Default: `<root>/.well-known/acme-challenge`. |
| `--acme-staging` | | Use Let's Encrypt staging (not browser-trusted, good for testing). |
| `--acme-renew-days` | | Renew when this many days (or fewer) remain before expiry. Default: `30`. |

---

## Usage

### Obtain a certificate and start serving

```powershell
larva-php --features acme,http3 `
  --root C:\var\www `
  --addr 0.0.0.0:80 `
  --http3 `
  --lets-encrypt `
  --acme-email admin@example.com `
  --acme-domains example.com,www.example.com
```

What happens on first run:

1. LarvaPHP creates an ACME account (RSA key saved to `--acme-dir`).
2. Creates an order for `example.com` and `www.example.com`.
3. Writes the HTTP-01 challenge token to
   `C:\var\www\.well-known\acme-challenge\<token>`.
4. The running HTTP server serves that path on port 80 so Let's Encrypt can
   fetch it.
5. Once validated, finalizes the order with a CSR and downloads the cert.
6. Saves `cert.pem`, `key.pem`, `fullchain.pem` to `--acme-dir`.
7. Starts the HTTP/3 server using the issued certificate.

On subsequent runs, the existing certificate is reused if it is not within the
renewal window. A background task checks every hour and renews automatically.

### Test with staging (recommended first)

Let's Encrypt production has strict rate limits. Use staging for your first run:

```powershell
larva-php --features acme,http3 `
  --root C:\var\www `
  --addr 0.0.0.0:80 `
  --http3 `
  --lets-encrypt --acme-staging `
  --acme-email admin@example.com `
  --acme-domains example.com
```

### Custom cert directory

```powershell
larva-php --features acme,http3 `
  --root C:\var\www `
  --lets-encrypt `
  --acme-email admin@example.com `
  --acme-domains example.com `
  --acme-dir C:\etc\larva-php\letsencrypt `
  --acme-renew-days 14
```

---

## How it works (ACME v2 flow)

```
1. Directory GET      →  https://acme-v02.api.letsencrypt.org/directory
   (or staging endpoint)

2. POST newAccount    →  create account, save RSA key to disk

3. POST newOrder      →  order for the requested domains (SANs)

4. HTTP-01 challenge
   ├─ write <token> file to challenge dir
   ├─ server serves it at /.well-known/acme-challenge/<token>
   ├─ POST challenge ready
   └─ poll authorization → valid

5. POST finalize      →  send CSR, get certificate URL

6. GET certificate    →  download PEM cert, save cert.pem / fullchain.pem
```

### Crypto details

- **Account key:** `openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048`
- **Domain key + CSR:** `openssl req -new -key ... -subj /CN=<domain> -addext subjectAltName=DNS:...`
- **JWS signatures (RS256):** `openssl dgst -sha256 -sign <account.key>`
- **JWK thumbprint (RFC 7638):** SHA-256 of the canonical JWK, base64url-nopad.
- **Expiry check:** `openssl x509 -in cert.pem -noout -checkend <seconds>`

### Renewal

A background task runs every hour (`tokio::time::sleep(1h)` inside a
`tokio::select!` with a shutdown channel). It calls `openssl x509 -checkend`
with the configured `--acme-renew-days` window. When the certificate is due for
renewal, the full ACME flow runs again and overwrites `cert.pem`/`key.pem`
in-place.

---

## Integration with HTTP/3

The ACME client is designed to feed the HTTP/3 (QUIC) server:

```powershell
# --lets-encrypt implies you want TLS; pair with --http3
larva-php --features acme,http3 --root /var/www --addr :80 --http3 --lets-encrypt ...
```

The certificate paths returned by the ACME client are passed to
`quiche::Config::load_cert_chain_from_pem_file` / `load_priv_key_from_pem_file`
inside `src/http3.rs`. TCP (HTTP/1.1 + HTTP/2) can also use the same
certificate via hyper's TLS config.

---

## File layout

```
<acme-dir>/
  account.key.pem        # ACME account RSA private key
  cert.pem               # issued leaf certificate (PEM)
  key.pem                # domain private key (PEM)
  fullchain.pem          # leaf + intermediates (PEM)
  domain.csr.pem         # CSR (temporary, kept for reference)
```

```
<challenge-dir>/
  <token-1>              # HTTP-01 token content (keyAuthorization)
  <token-2>
  ...
```

---

## Limitations

- **HTTP-01 only.** DNS-01 and TLS-ALPN-01 challenges are not supported.
- **Port 80 required** for validation. If you're behind a firewall/load
  balancer, ensure `/.well-known/acme-challenge/*` is forwarded to LarvaPHP.
- **Windows-first.** The crypto shell-outs use the `openssl` CLI which is
  present on the build host. Porting to a host without openssl would require a
  pure-Rust RSA/CSR/JWS implementation.
- **Staging certs are not browser-trusted.** Use `--acme-staging` only for
  testing; drop it for production.
- **Single certificate.** One order covers all `--acme-domains` as SANs.
  Multiple certificates require multiple LarvaPHP instances or a future
  multi-order extension.

---

## See also

- [`embedded-php.md`](embedded-php.md) — in-process PHP runtime (no CGI spawn).
- [`php-abi-structures.md`](php-abi-structures.md) — PHP SAPI structs and peak
  throughput techniques.
- [`performance.md`](performance.md) — measured numbers and per-request breakdown.
- [`http3.md`](http3.md) — HTTP/3 (QUIC) transport that consumes the ACME cert.
- [`runtime-architecture.md`](runtime-architecture.md) — master/worker model.
