# Linux Daemon Service Setup

This guide installs a release build of LarvaPHP with **all features enabled**
as a systemd service. The shipped unit uses the real embedded PHP runtime,
HTTP/3, ACME, Mercure, hot reload and FastCGI support.

## 1. Requirements

Same as the [Linux release build guide](build-release-linux.md), plus:

- systemd (any modern distribution)
- A dedicated system user (the units use `www-data`)
- `sudo` access on the target machine

Runtime requirements for the built binary:

- `libphp.so` must exist in the directory embedded in `RUNPATH` (`/usr/lib` by
  default), i.e. install `libphp8.3-embed` on the target machine too.
- PHP extensions/INI for the embed SAPI come from
  `/etc/php/8.3/embed/php.ini`.

## 2. Build a full-feature release

```bash
export PHP_LIB_DIR=/usr/lib
export PHP_EMBED_LIB=php8.3
export PHP_INCLUDES="$(php-config --includes)"

cargo build --release -p larva-php --bin larva-php --all-features
cargo test --lib --all-features
```

`--all-features` enables every feature declared in `crates/larva-php/Cargo.toml`:

| Feature | Effect in the daemon |
|---------|----------------------|
| `embedded` | In-process Zend PHP (no `php-cgi` spawn) |
| `http3` | QUIC listener on the same port via `--http3` |
| `acme` | Let's Encrypt certificate support via `--lets-encrypt` |
| `mercure` | Built-in Mercure hub |
| `watcher` | File watching dependency |
| `hotreload` | Config hot reload (implies `mercure` + `watcher`) |
| `fcgi` | FastCGI support |

Verify the binary before installing:

```bash
./target/release/larva-php --version
# LarvaPHP 1.13.5
# PHP 8.3.33

readelf -d target/release/larva-php | grep -E 'RUNPATH|libphp'
# RUNPATH [/usr/lib]
# NEEDED  libphp.so
```

## 3. Install the binary and runtime directories

```bash
sudo install -m 0755 target/release/larva-php /usr/local/bin/larva-php

sudo mkdir -p /etc/larva-php
sudo install -m 0644 deploy/systemd/larva-php.env.example \
  /etc/larva-php/larva-php.env

sudo systemd-tmpfiles --create deploy/systemd/larva-php-tmpfiles.conf
```

If you do not use tmpfiles.d, create the directories manually:

```bash
sudo install -d -o www-data -g www-data -m 0755 /var/lib/larva-php/tls
sudo install -d -o www-data -g www-data -m 0755 /var/lib/larva-php/acme
sudo install -d -o www-data -g www-data -m 0755 /var/log/larva-php
```

## 4. Configure the service

Edit `/etc/larva-php/larva-php.env`:

```ini
LARVA_ROOT=/var/www/myapp/public
LARVA_ADDR=127.0.0.1:8000
LARVA_WORKERS=0
LARVA_CERT=/var/lib/larva-php/tls/cert.pem
LARVA_KEY=/var/lib/larva-php/tls/key.pem
```

Important notes:

- `LARVA_ADDR` is used for both the TCP listener and the HTTP/3 UDP listener.
- `LARVA_WORKERS=0` runs a single process. With embedded PHP, prefer `0` or a
  small number; each worker is a separate process with its own PHP instance.
- The certificate directory must already exist and be writable by `www-data`.
  A self-signed cert is generated automatically on first start if the files
  are missing.
- Because the unit sets `ProtectHome=true`, keep the app under `/var/www` or
  `/srv`. If your app lives under `/home`, change it to `ProtectHome=read-only`.

## 5. Enable and start

```bash
sudo cp deploy/systemd/larva-php.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now larva-php

systemctl status larva-php
journalctl -u larva-php -f
```

Smoke test:

```bash
curl -I http://127.0.0.1:8000/
```

With `LARVA_ADDR=0.0.0.0:443`, the unit's `CAP_NET_BIND_SERVICE` allows binding
the privileged port without running as root.

## 6. Multiple sites (template unit)

Use `larva-php@.service` with one env file per instance:

```bash
sudo cp deploy/systemd/larva-php@.service /etc/systemd/system/
sudo cp /etc/larva-php/larva-php.env /etc/larva-php/blog.env
# edit /etc/larva-php/blog.env (root, addr, cert paths)

sudo systemctl daemon-reload
sudo systemctl enable --now larva-php@blog

systemctl status larva-php@blog
```

Each instance name (`blog`) maps to `/etc/larva-php/<name>.env`.

## 7. Let's Encrypt / ACME

The ACME feature is compiled in, but the unit does not enable it by default.
To use it, add these flags to `ExecStart` in the unit (or create a drop-in with
`systemctl edit larva-php`):

```text
--lets-encrypt \
--acme-email admin@example.com \
--acme-domains example.com,www.example.com \
--acme-dir /var/lib/larva-php/acme \
--acme-challenge-dir /var/www/myapp/public/.well-known/acme-challenge
```

> HTTP-01 validation needs port 80 reachable from the internet **while the
> certificate is requested**. LarvaPHP requests the certificate before its own
> listeners start, so serve `/.well-known/acme-challenge/` with an existing
> web server/reverse proxy on port 80, or obtain the certificate out-of-band
> and pass it via `--cert`/`--key`.

## 8. Management cheatsheet

```bash
systemctl restart larva-php
systemctl stop larva-php
systemctl disable --now larva-php
journalctl -u larva-php --since "10 min ago"
```

## 9. Troubleshooting

| Symptom | Fix |
|---------|-----|
| `libphp.so: cannot open shared object file` | Install `libphp8.3-embed` on the target; ensure `/usr/lib/libphp.so` exists. |
| `Failed to write cert: Permission denied` | `chown www-data:www-data /var/lib/larva-php/tls`. |
| Unit fails with empty path errors | The `EnvironmentFile` is missing or a `LARVA_*` variable is unset. |
| App assets return 403 | Check `LARVA_ROOT` permissions for `www-data`; adjust `ProtectHome` if under `/home`. |
| Port 80/443 bind denied | The unit already grants `CAP_NET_BIND_SERVICE`; check for another listener with `ss -tulpn`. |
| HTTP/3 unavailable from clients | Ensure UDP port equals `LARVA_ADDR` port and is open in the firewall. |
