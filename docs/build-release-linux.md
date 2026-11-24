# Linux Release Build Guide

This guide builds a **production release** of LarvaPHP on Linux with all
optional features enabled:

```text
embedded + http3 + acme + mercure
```

The Linux release uses the **real embedded PHP SAPI** (`larva_embed_unix.c`).
No no-op PHP stubs are compiled into it.

Verified on Debian 12 (bookworm), PHP 8.3.37, Rust 1.100.0.

---

## 1. Requirements

| Component | Why it is needed | Debian/Ubuntu package |
|-----------|------------------|-----------------------|
| Rust stable + Cargo | Builds the project | `rustup` (curl install) |
| C/C++ toolchain | Compiles the C bridge and native crates | `build-essential` |
| CMake | Required by BoringSSL for `http3` | `cmake` |
| Clang / libclang | Required by `bindgen` used by native crates | `clang`, `libclang-dev` |
| PHP 8.3 headers | PHP C API declarations for the SAPI shim | `php8.3-dev` |
| PHP embed library | Runtime Zend Engine (`libphp8.3.so`) | `libphp8.3-embed` |
| PHP CLI / `php-config` | Discovers PHP include paths | `php8.3-cli` |
| curl + CA certificates | Install Rust toolchain | `curl`, `ca-certificates` |
| pkg-config | Helps native dependency discovery | `pkg-config` |

> Cargo.lock is version 4; use a current stable Rust toolchain (1.83+ recommended).

### Install system packages

```bash
sudo apt update
sudo apt install -y \
  curl ca-certificates build-essential cmake clang libclang-dev pkg-config \
  php8.3-dev php8.3-cli libphp8.3-embed
```

### Install Rust (if needed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup update stable
rustc --version
cargo --version
```

---

## 2. Prepare the PHP embed library

The embed library's SONAME is `libphp.so`, while Debian installs
`/usr/lib/libphp8.3.so`. Create the expected link if it is missing:

```bash
readelf -d /usr/lib/libphp8.3.so | grep SONAME   # Library soname: [libphp.so]
sudo ln -sf /usr/lib/libphp8.3.so /usr/lib/libphp.so
```

Verify the PHP headers:

```bash
php-config --version
php-config --includes
```

---

## 3. Build environment

The build script reads these variables:

| Variable | Meaning | Example |
|----------|---------|---------|
| `PHP_LIB_DIR` | Directory containing `libphp.so`; embedded in the binary RUNPATH | `/usr/lib` |
| `PHP_EMBED_LIB` | Name passed to `-l` for the embed library | `php8.3` |
| `PHP_INCLUDES` | PHP include flags (falls back to `php-config --includes`) | `-I/usr/include/php/20230831 ...` |
| `LIBCLANG_PATH` | Only if bindgen cannot find libclang automatically | `/usr/lib/llvm-14/lib` |

```bash
export PHP_LIB_DIR=/usr/lib
export PHP_EMBED_LIB=php8.3
export PHP_INCLUDES="$(php-config --includes)"
# Optional, only if needed:
# export LIBCLANG_PATH=/usr/lib/llvm-14/lib
```

---

## 4. Build and test

```bash
cargo build --release -p larva-php --bin larva-php \
  --features embedded,http3,acme,mercure

cargo test --lib --features embedded,http3,acme,mercure
```

The binary is written to `target/release/larva-php`.

### What each feature adds

| Feature | Implementation | Extra build requirement |
|---------|----------------|-------------------------|
| `embedded` | Real Zend SAPI via `larva_embed_unix.c` | PHP headers + `libphp.so` |
| `http3` | `quiche` + BoringSSL | CMake, Clang/libclang |
| `acme` | `reqwest` with rustls + `ring` provider | Network access when issuing certs |
| `mercure` | Pure Rust | None |

The ACME client deliberately uses `rustls-no-provider` with the `ring` provider.
This avoids the `aws-lc-sys` backend, which requires extra UBSan runtime symbols
on some Linux toolchains. It also avoids `native-tls`, whose `libssl` name
conflicts with BoringSSL's static library in the same link.

---

## 5. Verify the release binary

```bash
./target/release/larva-php --version
# LarvaPHP 1.13.5
# PHP 8.3.33

readelf -d target/release/larva-php | grep -E 'RUNPATH|libphp'
# 0x... (RUNPATH) Library runpath: [/usr/lib]
# 0x... (NEEDED)   Shared library: [libphp.so]

ldd target/release/larva-php | grep libphp
# libphp.so => /usr/lib/libphp.so (0x...)
```

Smoke-test real embedded PHP execution:

```bash
cat > /tmp/larva_embed_check.php <<'PHP'
<?php
echo "EMBED_OK:" . PHP_VERSION . ":" . strtoupper("larva") . "\n";
PHP

./target/release/larva-php --ffi-test /tmp/larva_embed_check.php
```

Expected tail of the output:

```text
--- second run output ---
EMBED_OK:8.3.37:LARVA
```

You can also verify the full request context (`$_SERVER`, `$_COOKIE`,
`php://input`, headers and status) with `--ffi-request-test <script.php>`.

---

## 6. Runtime requirements

The binary is **not fully static**: it dynamically links `libphp.so`.

- On the target machine, install `libphp8.3-embed` (or provide `libphp.so` in
  the directory baked into `RUNPATH`).
- Configure the embed SAPI through `/etc/php/8.3/embed/php.ini` or
  `--php-ini key=value`.
- PHP extensions available to the embed SAPI are the ones installed for PHP
  8.3 on that machine.

To bundle PHP with your deployment, place `libphp.so` in a private directory,
set `PHP_LIB_DIR` to that directory, and rebuild; the RUNPATH will point there.

---

## 7. No-stub guarantee

The old `unix_stubs.c` has been removed. The Linux/macOS strategy compiles:

```text
crates/larva-php-sys/src/stubs.c              (neutral FFI declarations)
crates/larva-php-sys/src/larva_embed_unix.c   (real PHP SAPI shim)
```

Check locally with:

```bash
grep -rn unix_stubs . --exclude-dir=.git --exclude-dir=target
# no output
```

---

## 8. Troubleshooting

| Symptom | Fix |
|---------|-----|
| `cannot produce proc-macro ... +crt-static` | Remove `+crt-static` from `RUSTFLAGS`; it is MSVC-only and is no longer in `.cargo/config.toml`. |
| `cmake: command not found` | Install `cmake`. |
| `Failed to find LLVM FileCheck` / bindgen libclang errors | Install `clang libclang-dev`; set `LIBCLANG_PATH` if needed. |
| `PHP headers not found; set PHP_INCLUDES` | Install `php8.3-dev` or export `PHP_INCLUDES="$(php-config --includes)"`. |
| `libphp.so: cannot open shared object file` | Create `/usr/lib/libphp.so` symlink or rebuild with the correct `PHP_LIB_DIR`. |
| `lock file version 4 was found` | Upgrade Rust to a current stable toolchain. |
| `undefined symbol: __ubsan_handle_*` | Ensure ACME uses `rustls-no-provider` + `ring`, not the default `aws-lc-rs`. |
| Embedded script returns status `0` | Rebuild with the current shim; status lines like `HTTP/1.0 0 X` are normalised to `200`. |
