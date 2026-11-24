//! Let's Encrypt / ACME v2 client with automatic certificate renewal.
//!
//! Obtains and renews TLS certificates using the ACME protocol (RFC 8555)
//! with HTTP-01 challenges. No external ACME client (certbot, lego, etc.) is
//! required.
//!
//! # How it works
//!
//! 1. **Discovery** — fetch the Let's Encrypt directory to get endpoint URLs.
//! 2. **Registration** — create an ACME account (RSA key stored on disk).
//! 3. **Order** — create an order for the requested domains.
//! 4. **HTTP-01 challenge** — write the challenge token to
//!    `/.well-known/acme-challenge/<token>` so the Let's Encrypt validation
//!    server can fetch it over HTTP.
//! 5. **Finalize** — send a CSR and download the issued certificate.
//! 6. **Renewal** — a background task checks the certificate's `notAfter` and
//!    renews when it falls within `--acme-renew-days` of expiry.
//!
//! # Crypto
//!
//! RSA keys and CSRs are generated with the system `openssl` CLI (confirmed
//! available on the build host). JWS signatures for ACME requests are also
//! signed with `openssl dgst -sha256 -sign`. This keeps the Rust side focused
//! on the ACME protocol rather than ASN.1/DER encoding.
//!
//! # Example
//!
//! ```no_run
//! use larva_php::acme::{AcmeClient, AcmeConfig};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> larva_php::Result<()> {
//!     let config = AcmeConfig {
//!         email: "admin@example.com".to_string(),
//!         domains: vec!["example.com".to_string()],
//!         cert_dir: PathBuf::from("/etc/larva-php/letsencrypt"),
//!         challenge_dir: PathBuf::from("/var/www/html/.well-known/acme-challenge"),
//!         staging: false,
//!         renew_days: 30,
//!     };
//!     let client = AcmeClient::new(config)?;
//!     let cert = client.obtain_certificate().await?;
//!     tracing::info!("cert: {:?}", cert.cert_path);
//!     Ok(())
//! }
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use reqwest::Client;
use serde_json::{json, Value};

use crate::errors::{Error, Result};

/// Let's Encrypt directory endpoints.
const ACME_PRODUCTION: &str = "https://acme-v02.api.letsencrypt.org/directory";
const ACME_STAGING: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";

/// File name for the ACME account key.
const ACCOUNT_KEY_FILENAME: &str = "account.key.pem";

/// File name for the issued certificate.
const CERT_FILENAME: &str = "cert.pem";

/// File name for the private key.
const KEY_FILENAME: &str = "key.pem";

/// File name for the full certificate chain (PEM).
const FULLCHAIN_FILENAME: &str = "fullchain.pem";

/// How often the renewal loop checks for expiry.
const RENEWAL_CHECK_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour

/// Seconds in a day.
const SECONDS_PER_DAY: u64 = 86_400;

/// Result of obtaining or renewing a certificate.
#[derive(Debug, Clone)]
pub struct CertResult {
    /// Path to the certificate (PEM).
    pub cert_path: PathBuf,
    /// Path to the private key (PEM).
    pub key_path: PathBuf,
    /// Path to the full certificate chain (PEM).
    pub fullchain_path: PathBuf,
}

/// Configuration for the ACME client.
#[derive(Debug, Clone)]
pub struct AcmeConfig {
    /// Contact email for the Let's Encrypt account.
    pub email: String,
    /// Domains to include in the certificate (SANs).
    pub domains: Vec<String>,
    /// Directory to store account key, certificate, and key.
    pub cert_dir: PathBuf,
    /// Directory where HTTP-01 challenge tokens are written. The HTTP server
    /// must serve this path at `/.well-known/acme-challenge/`.
    pub challenge_dir: PathBuf,
    /// Use Let's Encrypt staging (rate-limit friendly, not trusted by browsers).
    pub staging: bool,
    /// Renew when the certificate is this many days (or fewer) from expiry.
    pub renew_days: u32,
}

impl Default for AcmeConfig {
    fn default() -> Self {
        let home = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            email: String::new(),
            domains: Vec::new(),
            cert_dir: home.join(".larva-php").join("letsencrypt"),
            challenge_dir: PathBuf::from(".well-known/acme-challenge"),
            staging: false,
            renew_days: 30,
        }
    }
}

/// The ACME v2 client.
pub struct AcmeClient {
    config: AcmeConfig,
    http: Client,
    directory_url: String,
}

impl AcmeClient {
    /// Create a new ACME client. Ensures the cert directory exists.
    pub fn new(config: AcmeConfig) -> Result<Self> {
        fs::create_dir_all(&config.cert_dir)
            .map_err(|e| Error::Acme(format!("create cert dir: {}", e)))?;
        let directory_url = if config.staging {
            ACME_STAGING.to_string()
        } else {
            ACME_PRODUCTION.to_string()
        };

        // reqwest is built with `rustls-no-provider`; install ring as the
        // process-wide crypto provider. The first call wins, later clients
        // simply reuse it.
        let _ = rustls::crypto::CryptoProvider::install_default(
            rustls::crypto::ring::default_provider(),
        );

        let http = Client::builder()
            .user_agent("LarvaPHP/1.13.5 (https://larvaphp.dev)")
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Acme(format!("build http client: {}", e)))?;

        Ok(Self {
            config,
            http,
            directory_url,
        })
    }

    /// Obtain a certificate. Returns existing cert paths if already valid.
    pub async fn obtain_certificate(&self) -> Result<CertResult> {
        // If a valid cert already exists and isn't due for renewal, reuse it.
        if self.cert_exists_and_valid()? {
            return Ok(self.cert_paths());
        }

        self.run_acme_flow().await
    }

    /// Check whether the certificate needs renewal and renew if so.
    /// Returns `true` if a renewal was performed.
    pub async fn renew_if_needed(&self) -> Result<bool> {
        if !self.cert_exists_and_valid()? {
            // No cert at all: obtain one.
            self.obtain_certificate().await?;
            return Ok(true);
        }

        if self.is_near_expiry()? {
            tracing::info!("certificate within renewal window, renewing");
            self.obtain_certificate().await?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Background renewal loop. Checks for expiry every hour and renews when
    /// needed. Runs until the provided receiver is dropped.
    pub async fn renewal_loop(
        &self,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    tracing::info!("ACME renewal loop shutting down");
                    return Ok(());
                }
                _ = tokio::time::sleep(RENEWAL_CHECK_INTERVAL) => {
                    match self.renew_if_needed().await {
                        Ok(renewed) => {
                            if renewed {
                                tracing::info!("certificate renewed successfully");
                            }
                        }
                        Err(e) => {
                            tracing::warn!("certificate renewal check failed: {}", e);
                        }
                    }
                }
            }
        }
    }

    // ---- Private: full ACME flow ----

    async fn run_acme_flow(&self) -> Result<CertResult> {
        let dir = self.fetch_directory().await?;
        let account_key = self.ensure_account_key()?;
        let account_jwk = self.account_jwk(&account_key)?;
        let account_url = self.register_account(&dir, &account_key, &account_jwk).await?;

        // Generate domain key + CSR.
        let domain_key_path = self.config.cert_dir.join(KEY_FILENAME);
        let csr_path = self.config.cert_dir.join("domain.csr.pem");
        self.generate_domain_key_and_csr(&domain_key_path, &csr_path)?;

        let csr_der = fs::read(&csr_path)
            .map_err(|e| Error::Acme(format!("read CSR: {}", e)))?;
        let csr_b64 = URL_SAFE_NO_PAD.encode(&csr_der);

        let order_url = self.create_order(&dir, &account_url).await?;
        self.solve_challenges(&dir, &account_url, &account_jwk, &order_url).await?;
        self.finalize_order(&dir, &account_url, &order_url, &csr_b64).await?;
        let cert_pem = self.download_certificate(&dir, &order_url).await?;

        // Save cert + chain.
        let cert_path = self.config.cert_dir.join(CERT_FILENAME);
        let key_path = self.config.cert_dir.join(KEY_FILENAME);
        let fullchain_path = self.config.cert_dir.join(FULLCHAIN_FILENAME);

        fs::write(&cert_path, &cert_pem)
            .map_err(|e| Error::Acme(format!("write cert: {}", e)))?;
        // The domain key is already at key_path (generated above).
        // Build fullchain: cert + any intermediates.
        let fullchain = cert_pem.clone();
        fs::write(&fullchain_path, &fullchain)
            .map_err(|e| Error::Acme(format!("write fullchain: {}", e)))?;

        tracing::info!("certificate obtained: {:?}", cert_path);
        Ok(CertResult {
            cert_path,
            key_path,
            fullchain_path,
        })
    }

    // ---- Private: directory ----

    async fn fetch_directory(&self) -> Result<Value> {
        let resp = self
            .http
            .get(&self.directory_url)
            .send()
            .await
            .map_err(|e| Error::Acme(format!("fetch directory: {}", e)))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| Error::Acme(format!("read directory body: {}", e)))?;
        if !status.is_success() {
            return Err(Error::Acme(format!(
                "directory request failed: {} {}",
                status, body
            )));
        }
        serde_json::from_str(&body)
            .map_err(|e| Error::Acme(format!("parse directory: {}", e)))
    }

    // ---- Private: account ----

    fn ensure_account_key(&self) -> Result<PathBuf> {
        let key_path = self.config.cert_dir.join(ACCOUNT_KEY_FILENAME);
        if key_path.exists() {
            return Ok(key_path);
        }
        let status = Command::new("openssl")
            .args([
                "genpkey",
                "-algorithm",
                "RSA",
                "-pkeyopt",
                "rsa_keygen_bits:2048",
                "-out",
                key_path.to_str().ok_or_else(|| {
                    Error::Acme("invalid account key path".to_string())
                })?,
            ])
            .status()
            .map_err(|e| Error::Acme(format!("openssl genpkey: {}", e)))?;
        if !status.success() {
            return Err(Error::Acme("openssl genpkey failed".to_string()));
        }
        Ok(key_path)
    }

    fn account_jwk(&self, account_key: &Path) -> Result<Value> {
        let (modulus_b64, _) = self.rsa_public_parts(account_key)?;
        let jwk = json!({
            "e": "AQAB",
            "kty": "RSA",
            "n": modulus_b64,
        });
        Ok(jwk)
    }

    /// Extract base64url(modulus) and base64url(exponent) from an RSA key.
    fn rsa_public_parts(&self, key_path: &Path) -> Result<(String, String)> {
        let output = Command::new("openssl")
            .args(["rsa", "-in", key_path.to_str().unwrap(), "-text", "-noout"])
            .output()
            .map_err(|e| Error::Acme(format!("openssl rsa -text: {}", e)))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut modulus_hex = String::new();
        let mut in_modulus = false;
        let mut exponent: Option<String> = None;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == "modulus:" {
                in_modulus = true;
                continue;
            }
            if in_modulus {
                if trimmed.starts_with("publicExponent:") {
                    in_modulus = false;
                    // publicExponent: 65537 (0x10001)
                    if let Some(paren) = trimmed.rfind("0x") {
                        let hex = &trimmed[paren + 2..];
                        let exp_val = u32::from_str_radix(hex, 16).unwrap_or(65537);
                        exponent = Some(exp_val.to_string());
                    }
                    continue;
                }
                if !trimmed.is_empty() {
                    modulus_hex.push_str(&trimmed.replace(':', ""));
                }
            }
        }

        if modulus_hex.is_empty() {
            return Err(Error::Acme("could not parse RSA modulus".to_string()));
        }

        // Remove leading 0x00 byte if present (modulus is stored as unsigned).
        let hex = if modulus_hex.starts_with("00") {
            &modulus_hex[2..]
        } else {
            &modulus_hex
        };
        let modulus_bytes = Self::decode_hex(hex).map_err(|e| {
            Error::Acme(format!("decode modulus hex: {}", e))
        })?;
        let modulus_b64 = URL_SAFE_NO_PAD.encode(&modulus_bytes);

        // Exponent: encode as base64url of the big-endian bytes.
        let exp_val = exponent.unwrap_or_else(|| "65537".to_string());
        let exp_int: u32 = exp_val.parse().unwrap_or(65537);
        let exp_bytes = exp_int.to_be_bytes();
        // Trim leading zeros.
        let exp_trimmed: Vec<u8> = exp_bytes
            .into_iter()
            .skip_while(|&b| b == 0)
            .collect();
        let exp_b64 = URL_SAFE_NO_PAD.encode(&exp_trimmed);

        Ok((modulus_b64, exp_b64))
    }

    /// Compute the JWK thumbprint (RFC 7638) of the account key.
    fn account_jwk_thumbprint(&self, account_key: &Path) -> Result<String> {
        let jwk = self.account_jwk(account_key)?;
        // Canonical form: keys sorted alphabetically, no whitespace.
        let canonical = format!(
            r#"{{"e":"{}","kty":"{}","n":"{}"}}"#,
            jwk["e"].as_str().unwrap_or(""),
            jwk["kty"].as_str().unwrap_or(""),
            jwk["n"].as_str().unwrap_or("")
        );
        let digest = self.sha256_hex(canonical.as_bytes())?;
        let digest_bytes = Self::decode_hex(&digest).map_err(|e| {
            Error::Acme(format!("decode sha256 hex: {}", e))
        })?;
        Ok(URL_SAFE_NO_PAD.encode(&digest_bytes))
    }

    /// SHA-256 hex digest via openssl.
    fn sha256_hex(&self, data: &[u8]) -> Result<String> {
        // Write data to a temp file and hash it.
        let temp = std::env::temp_dir().join(format!(
            "larva-acme-sha-{}.dat",
            std::process::id()
        ));
        fs::write(&temp, data)
            .map_err(|e| Error::Acme(format!("write temp for sha256: {}", e)))?;
        let output = Command::new("openssl")
            .args(["dgst", "-sha256", "-hex", temp.to_str().unwrap()])
            .output()
            .map_err(|e| Error::Acme(format!("openssl dgst: {}", e)))?;
        let _ = fs::remove_file(&temp);
        let text = String::from_utf8_lossy(&output.stdout);
        // Output: SHA2-256(file)= <hex>
        let hex = text
            .split('=')
            .nth(1)
            .ok_or_else(|| Error::Acme("unexpected dgst output".to_string()))?
            .trim()
            .to_string();
        Ok(hex)
    }

    /// Sign a JWS signing input with the account key (RS256).
    fn jws_sign(&self, account_key: &Path, signing_input: &[u8]) -> Result<String> {
        let temp_in = std::env::temp_dir().join(format!(
            "larva-acme-jws-in-{}.dat",
            std::process::id()
        ));
        let temp_out = std::env::temp_dir().join(format!(
            "larva-acme-jws-out-{}.bin",
            std::process::id()
        ));
        fs::write(&temp_in, signing_input)
            .map_err(|e| Error::Acme(format!("write jws input: {}", e)))?;

        let status = Command::new("openssl")
            .args([
                "dgst",
                "-sha256",
                "-sign",
                account_key.to_str().unwrap(),
                "-out",
                temp_out.to_str().unwrap(),
                temp_in.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| Error::Acme(format!("openssl dgst -sign: {}", e)))?;
        let _ = fs::remove_file(&temp_in);
        if !status.success() {
            let _ = fs::remove_file(&temp_out);
            return Err(Error::Acme("openssl sign failed".to_string()));
        }

        let sig = fs::read(&temp_out)
            .map_err(|e| Error::Acme(format!("read signature: {}", e)))?;
        let _ = fs::remove_file(&temp_out);
        Ok(URL_SAFE_NO_PAD.encode(&sig))
    }

    /// Build a JWS protected header.
    fn protected_header(&self, nonce: &str, url: &str, jwk: &Value) -> Result<String> {
        let protected = json!({
            "alg": "RS256",
            "jwk": jwk,
            "nonce": nonce,
            "url": url,
        });
        Ok(URL_SAFE_NO_PAD.encode(serde_json::to_string(&protected)?.as_bytes()))
    }

    /// Send a signed POST request to an ACME endpoint.
    async fn post_signed(
        &self,
        account_key: &Path,
        jwk: &Value,
        url: &str,
        payload: Option<&Value>,
        nonce: &str,
    ) -> Result<(reqwest::Response, String)> {
        let protected = self.protected_header(nonce, url, jwk)?;
        let payload_b64 = match payload {
            Some(p) => URL_SAFE_NO_PAD.encode(serde_json::to_string(p)?.as_bytes()),
            None => URL_SAFE_NO_PAD.encode(b""),
        };
        let signing_input = format!("{}.{}", protected, payload_b64);
        let signature = self.jws_sign(account_key, signing_input.as_bytes())?;

        let body = json!({
            "protected": protected,
            "payload": payload_b64,
            "signature": signature,
        });

        let resp = self
            .http
            .post(url)
            .header("Content-Type", "application/jose+json")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Acme(format!("acme post: {}", e)))?;

        let new_nonce = resp
            .headers()
            .get("replay-nonce")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(nonce)
            .to_string();

        Ok((resp, new_nonce))
    }

    /// Register an ACME account. Returns the account location URL.
    async fn register_account(
        &self,
        dir: &Value,
        account_key: &Path,
        jwk: &Value,
    ) -> Result<String> {
        let new_account = dir["newAccount"]
            .as_str()
            .ok_or_else(|| Error::Acme("directory missing newAccount".to_string()))?;

        let (_, new_nonce) = self.post_signed(account_key, jwk, new_account, None, "").await?;

        // Re-post with the fresh nonce (ACME requires a valid nonce on the
        // registration request).
        let (resp, _) = self
            .post_signed(account_key, jwk, new_account, None, &new_nonce)
            .await?;

        let status = resp.status();
        if !status.is_success()
            && status != reqwest::StatusCode::CREATED
            && status != reqwest::StatusCode::OK
        {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Acme(format!(
                "account registration failed: {} {}",
                status, body
            )));
        }

        let location = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                Error::Acme("account registration missing Location header".to_string())
            })?;

        Ok(location)
    }

    // ---- Private: order ----

    async fn create_order(&self, dir: &Value, _account_url: &str) -> Result<String> {
        let new_order = dir["newOrder"]
            .as_str()
            .ok_or_else(|| Error::Acme("directory missing newOrder".to_string()))?;

        let account_key_path = self.config.cert_dir.join(ACCOUNT_KEY_FILENAME);
        let jwk = self.account_jwk(&account_key_path)?;

        let identifiers: Value = self
            .config
            .domains
            .iter()
            .map(|d| json!({ "type": "dns", "value": d }))
            .collect();

        let (_, new_nonce) = self
            .post_signed(
                &account_key_path,
                &jwk,
                new_order,
                None,
                "",
            )
            .await?;

        // Re-post with the fresh nonce.
        let (resp, _) = self
            .post_signed(
                &account_key_path,
                &jwk,
                new_order,
                Some(&json!({ "identifiers": identifiers })),
                &new_nonce,
            )
            .await?;

        let order_status = resp.status();
        if !order_status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Acme(format!(
                "create order failed: {} {}",
                order_status, body
            )));
        }

        let location = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Acme("create order missing Location header".to_string()))?;

        Ok(location)
    }

    /// Solve HTTP-01 challenges for the order.
    async fn solve_challenges(
        &self,
        _dir: &Value,
        _account_url: &str,
        jwk: &Value,
        order_url: &str,
    ) -> Result<()> {
        let account_key_path = self.config.cert_dir.join(ACCOUNT_KEY_FILENAME);

        // Fetch the order to get authorizations.
        let mut nonce = String::new();
        let (resp, new_nonce) = self.post_signed(
            account_key_path.as_path(),
            jwk,
            order_url,
            None,
            &nonce,
        )
        .await?;
        nonce = new_nonce;

        let order: Value = serde_json::from_str(
            &resp.text().await.map_err(|e| Error::Acme(format!("read order: {}", e)))?,
        )
        .map_err(|e| Error::Acme(format!("parse order: {}", e)))?;

        let authorizations = order["authorizations"]
            .as_array()
            .ok_or_else(|| Error::Acme("order missing authorizations".to_string()))?;

        let thumbprint = self.account_jwk_thumbprint(&account_key_path)?;

        for authz in authorizations {
            let authz_url = authz["url"].as_str().ok_or_else(|| {
                Error::Acme("authorization missing url".to_string())
            })?;

            // Fetch the authorization to get challenges.
            let (resp, new_nonce) = self.post_signed(
                account_key_path.as_path(),
                jwk,
                authz_url,
                None,
                &nonce,
            )
            .await?;
            nonce = new_nonce;

            let authz_detail: Value = serde_json::from_str(
                &resp.text().await.map_err(|e| Error::Acme(format!("read authz: {}", e)))?,
            )
            .map_err(|e| Error::Acme(format!("parse authz: {}", e)))?;

            let challenges = authz_detail["challenges"].as_array().ok_or_else(|| {
                Error::Acme("authorization missing challenges".to_string())
            })?;

            let http01 = challenges.iter().find(|c| c["type"] == "http-01").ok_or_else(|| {
                Error::Acme("no http-01 challenge found".to_string())
            })?;

            let token = http01["token"]
                .as_str()
                .ok_or_else(|| Error::Acme("challenge missing token".to_string()))?;
            let challenge_url = http01["url"]
                .as_str()
                .ok_or_else(|| Error::Acme("challenge missing url".to_string()))?;

            // Write the token file: content = keyAuthorization = token.thumbprint.
            self.write_challenge_file(token, &format!("{}.{}", token, thumbprint))?;

            // Tell the ACME server the challenge is ready.
            let (resp, new_nonce) = self.post_signed(
                account_key_path.as_path(),
                jwk,
                challenge_url,
                Some(&json!({})),
                &nonce,
            )
            .await?;
            nonce = new_nonce;

            let challenge_status = resp.status();
            if !challenge_status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(Error::Acme(format!(
                    "challenge validation failed: {} {}",
                    challenge_status, body
                )));
            }

            // Poll until valid.
            self.poll_authorization(account_key_path.as_path(), jwk, authz_url, &mut nonce).await?;
        }

        Ok(())
    }

    /// Poll an authorization until it reaches "valid" status.
    async fn poll_authorization(
        &self,
        account_key: &Path,
        jwk: &Value,
        authz_url: &str,
        nonce: &mut String,
    ) -> Result<()> {
        for _ in 0..60 {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let (resp, new_nonce) = self.post_signed(account_key, jwk, authz_url, None, nonce).await?;
            *nonce = new_nonce;

            if !resp.status().is_success() {
                continue;
            }

            let detail: Value = serde_json::from_str(
                &resp.text().await.unwrap_or_default(),
            )
            .unwrap_or(Value::Null);

            let status = detail["status"].as_str().unwrap_or("");
            if status == "valid" {
                return Ok(());
            }
            if status == "invalid" || status == "failed" {
                let err = detail["error"].as_str().unwrap_or("unknown");
                return Err(Error::Acme(format!(
                    "authorization failed: {}",
                    err
                )));
            }
        }

        Err(Error::Acme("authorization timed out".to_string()))
    }

    /// Finalize the order with the CSR.
    async fn finalize_order(
        &self,
        _dir: &Value,
        _account_url: &str,
        order_url: &str,
        csr_b64: &str,
    ) -> Result<()> {
        let order: Value = serde_json::from_str(
            &self
                .http
                .get(order_url)
                .send()
                .await
                .map_err(|e| Error::Acme(format!("get order: {}", e)))?
                .text()
                .await
                .map_err(|e| Error::Acme(format!("read order: {}", e)))?,
        )
        .map_err(|e| Error::Acme(format!("parse order: {}", e)))?;

        let finalize_url = order["finalize"]
            .as_str()
            .ok_or_else(|| Error::Acme("order missing finalize".to_string()))?;

        let account_key_path = self.config.cert_dir.join(ACCOUNT_KEY_FILENAME);
        let jwk = self.account_jwk(&account_key_path)?;
        let mut nonce = String::new();

        let (_, new_nonce) = self
            .post_signed(
                &account_key_path,
                &jwk,
                finalize_url,
                Some(&json!({ "csr": csr_b64 })),
                &nonce,
            )
            .await?;
        nonce = new_nonce;

        // Re-post with fresh nonce (finalize may need it).
        let (resp, _) = self
            .post_signed(
                &account_key_path,
                &jwk,
                finalize_url,
                Some(&json!({ "csr": csr_b64 })),
                &nonce,
            )
            .await?;

        let finalize_status = resp.status();
        if !finalize_status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Acme(format!(
                "finalize failed: {} {}",
                finalize_status, body
            )));
        }

        Ok(())
    }

    /// Download the issued certificate.
    async fn download_certificate(&self, _dir: &Value, order_url: &str) -> Result<String> {
        // Poll the order until the certificate is available.
        for _ in 0..60 {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let resp = self
                .http
                .get(order_url)
                .send()
                .await
                .map_err(|e| Error::Acme(format!("get order for cert: {}", e)))?;
            if !resp.status().is_success() {
                continue;
            }
            let order: Value = serde_json::from_str(
                &resp.text().await.unwrap_or_default(),
            )
            .unwrap_or(Value::Null);
            let cert_url = order["certificate"].as_str();
            if let Some(cert_url) = cert_url {
                let cert_resp = self
                    .http
                    .get(cert_url)
                    .send()
                    .await
                    .map_err(|e| Error::Acme(format!("download cert: {}", e)))?;
                if cert_resp.status().is_success() {
                    return cert_resp
                        .text()
                        .await
                        .map_err(|e| Error::Acme(format!("read cert: {}", e)));
                }
            }
        }

        Err(Error::Acme("certificate not available after polling".to_string()))
    }

    // ---- Private: challenge file ----

    /// Write an HTTP-01 challenge token file.
    fn write_challenge_file(&self, token: &str, content: &str) -> Result<()> {
        fs::create_dir_all(&self.config.challenge_dir)
            .map_err(|e| Error::Acme(format!("create challenge dir: {}", e)))?;
        let path = self.config.challenge_dir.join(token);
        fs::write(&path, content)
            .map_err(|e| Error::Acme(format!("write challenge file: {}", e)))?;
        Ok(())
    }

    // ---- Private: cert state ----

    fn cert_exists_and_valid(&self) -> Result<bool> {
        let cert_path = self.config.cert_dir.join(CERT_FILENAME);
        if !cert_path.exists() {
            return Ok(false);
        }
        if self.is_near_expiry()? {
            return Ok(false);
        }
        Ok(true)
    }

    /// Check if the certificate is within the renewal window using
    /// `openssl x509 -checkend`.
    fn is_near_expiry(&self) -> Result<bool> {
        let cert_path = self.config.cert_dir.join(CERT_FILENAME);
        if !cert_path.exists() {
            return Ok(true);
        }
        let seconds = self.config.renew_days as u64 * SECONDS_PER_DAY;
        // -checkend returns exit code 0 if the cert will NOT expire within
        // `seconds`, non-zero if it will (or on error).
        let status = Command::new("openssl")
            .args([
                "x509",
                "-in",
                cert_path.to_str().unwrap(),
                "-noout",
                "-checkend",
                &seconds.to_string(),
            ])
            .status()
            .map_err(|e| Error::Acme(format!("openssl x509 -checkend: {}", e)))?;
        // Exit code non-zero means the cert is within the renewal window.
        Ok(!status.success())
    }

    fn cert_paths(&self) -> CertResult {
        CertResult {
            cert_path: self.config.cert_dir.join(CERT_FILENAME),
            key_path: self.config.cert_dir.join(KEY_FILENAME),
            fullchain_path: self.config.cert_dir.join(FULLCHAIN_FILENAME),
        }
    }

    /// Generate a domain private key + CSR using openssl.
    fn generate_domain_key_and_csr(&self, key_path: &Path, csr_path: &Path) -> Result<()> {
        // Generate key if missing.
        if !key_path.exists() {
            let status = Command::new("openssl")
                .args([
                    "genpkey",
                    "-algorithm",
                    "RSA",
                    "-pkeyopt",
                    "rsa_keygen_bits:2048",
                    "-out",
                    key_path.to_str().unwrap(),
                ])
                .status()
                .map_err(|e| Error::Acme(format!("openssl genpkey: {}", e)))?;
            if !status.success() {
                return Err(Error::Acme("openssl genpkey failed".to_string()));
            }
        }

        // Build subjectAltName for all domains.
        let mut san = String::from("subjectAltName=");
        for (i, d) in self.config.domains.iter().enumerate() {
            if i > 0 {
                san.push(',');
            }
            san.push_str(&format!("DNS:{}", d));
        }

        let subject = format!("/CN={}", self.config.domains[0]);

        let status = Command::new("openssl")
            .args([
                "req",
                "-new",
                "-key",
                key_path.to_str().unwrap(),
                "-subj",
                &subject,
                "-addext",
                &san,
                "-out",
                csr_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| Error::Acme(format!("openssl req: {}", e)))?;
        if !status.success() {
            return Err(Error::Acme("openssl req failed".to_string()));
        }

        Ok(())
    }

    /// Decode a hex string into bytes (lowercase or uppercase).
    fn decode_hex(hex: &str) -> Result<Vec<u8>> {
        let hex = hex.trim();
        if hex.len() % 2 != 0 {
            return Err(Error::Acme("odd-length hex string".to_string()));
        }
        let mut out = Vec::with_capacity(hex.len() / 2);
        for i in (0..hex.len()).step_by(2) {
            let byte = u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|e| Error::Acme(format!("invalid hex byte: {}", e)))?;
            out.push(byte);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_renew_days() {
        let cfg = AcmeConfig::default();
        assert_eq!(cfg.renew_days, 30);
        assert!(!cfg.staging);
    }

    #[test]
    fn cert_result_paths() {
        let dir = std::env::temp_dir().join(format!("larva-acme-test-{}", std::process::id()));
        let config = AcmeConfig {
            email: "test@example.com".into(),
            domains: vec!["example.com".into()],
            cert_dir: dir.clone(),
            challenge_dir: dir.join("challenges"),
            staging: true,
            renew_days: 30,
        };
        let _ = std::fs::create_dir_all(&dir);
        let client = AcmeClient::new(config).unwrap();
        let paths = client.cert_paths();
        assert_eq!(paths.cert_path, dir.join("cert.pem"));
        assert_eq!(paths.key_path, dir.join("key.pem"));
        assert_eq!(paths.fullchain_path, dir.join("fullchain.pem"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn decode_hex_works() {
        let bytes = AcmeClient::decode_hex("deadbeef").unwrap();
        assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn decode_hex_odd_length_errors() {
        assert!(AcmeClient::decode_hex("dead").is_ok());
        assert!(AcmeClient::decode_hex("dea").is_err());
    }
}
