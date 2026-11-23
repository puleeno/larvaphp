use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use larva_php::{self, VERSION};

/// LarvaPHP - a modern PHP application server
#[derive(Parser, Debug)]
#[command(version, about, long_about = None, disable_version_flag = true, disable_help_flag = true)]
struct Args {
    /// Run a PHP script file
    #[arg(value_name = "SCRIPT")]
    script: Option<String>,

    /// Document root directory for serving PHP applications
    #[arg(short = 'r', long = "root", value_name = "DIR")]
    root: Option<String>,

    /// Address to bind the server to
    #[arg(short = 'a', long = "addr", default_value = "127.0.0.1:8000")]
    addr: String,

    /// Set a php.ini directive (repeatable), e.g. --php-ini precision=14
    #[arg(long = "php-ini", value_name = "KEY=VALUE")]
    php_ini: Vec<String>,

    /// Enable debug logging (verbose output)
    #[arg(long = "debug")]
    debug: bool,

    /// Number of runtime worker processes (0 = use CGI spawn). Enables the
    /// 2-layer fault-isolated runtime when > 0.
    #[arg(long = "workers", value_name = "N", default_value = "0")]
    workers: usize,

    /// Generate a shell completion script and exit (bash|zsh|fish|powershell)
    #[arg(long = "completion", value_name = "SHELL")]
    completion: Option<String>,

    /// Enable HTTP/3 (QUIC) on the same port (UDP). Requires --features http3.
    #[arg(long = "http3")]
    http3: bool,

    /// Use the embedded PHP runtime (in-process, no php-cgi spawn).
    /// Requires --features embedded. The NTS php8.dll is not thread-safe,
    /// so --workers must be 1 (or 0 for single-process mode).
    #[arg(long = "embedded")]
    embedded: bool,

    /// Path to a TLS certificate (PEM) for HTTP/3. Auto-generated if missing.
    #[arg(long = "cert", value_name = "FILE")]
    cert: Option<String>,

    /// Path to a TLS private key (PEM) for HTTP/3. Auto-generated if missing.
    #[arg(long = "key", value_name = "FILE")]
    key: Option<String>,

    /// Enable Let's Encrypt automatic certificate provisioning (ACME v2,
    /// HTTP-01 challenge). Requires --features acme.
    #[arg(long = "lets-encrypt", short = 'L')]
    lets_encrypt: bool,

    /// Contact email for the Let's Encrypt account (required with
    /// --lets-encrypt).
    #[arg(long = "acme-email", value_name = "EMAIL")]
    acme_email: Option<String>,

    /// Comma-separated list of domains for the certificate (required with
    /// --lets-encrypt).
    #[arg(long = "acme-domains", value_name = "DOMAINS")]
    acme_domains: Option<String>,

    /// Directory to store Let's Encrypt account key and certificates.
    #[arg(long = "acme-dir", value_name = "DIR")]
    acme_dir: Option<String>,

    /// Directory where HTTP-01 challenge tokens are written. The HTTP server
    /// must serve this at /.well-known/acme-challenge/. Defaults to
    /// <root>/.well-known/acme-challenge.
    #[arg(long = "acme-challenge-dir", value_name = "DIR")]
    acme_challenge_dir: Option<String>,

    /// Use Let's Encrypt staging server (not trusted by browsers, useful for
    /// testing).
    #[arg(long = "acme-staging")]
    acme_staging: bool,

    /// Renew certificate when this many days (or fewer) remain before expiry.
    #[arg(long = "acme-renew-days", value_name = "DAYS", default_value = "30")]
    acme_renew_days: u32,

    /// Show version information
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// Show help information
    #[arg(short = 'h', long = "help")]
    help: bool,

    // ---- Internal: worker process mode (spawned by the master) ----
    /// Hidden: run as a worker process (internal).
    #[arg(long = "worker", hide = true)]
    worker: bool,

    /// Hidden: worker id (internal).
    #[arg(long = "id", hide = true)]
    worker_id: Option<usize>,

    /// Hidden: path to the worker's shared-memory segment (internal).
    #[arg(long = "shm", hide = true)]
    shm: Option<String>,

    /// Hidden: run a self-test of the 2-layer runtime (spawns workers).
    #[arg(long = "runtime-demo", hide = true)]
    runtime_demo: bool,

    /// Hidden: test the embedded PHP FFI executor directly on a script.
    #[arg(long = "ffi-test", hide = true, value_name = "SCRIPT")]
    ffi_test: Option<String>,

    /// Hidden: test the embedded PHP request executor with mock CGI data.
    #[arg(long = "ffi-request-test", hide = true, value_name = "SCRIPT")]
    ffi_request_test: Option<String>,
}

fn print_usage(out: &mut dyn std::io::Write) {
    let _ = writeln!(
        out,
        "LarvaPHP {} - a modern PHP application server",
        VERSION
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "Usage:");
    let _ = writeln!(
        out,
        "  larva-php <script.php> [args...]    Run a PHP script"
    );
    let _ = writeln!(
        out,
        "  larva-php --root <dir>              Serve PHP application from directory"
    );
    let _ = writeln!(
        out,
        "  larva-php --php-ini key=value       Set a php.ini directive (repeatable)"
    );
    let _ = writeln!(out, "  larva-php --debug                   Enable debug logging");
    let _ = writeln!(
        out,
        "  larva-php --completion <shell>      Generate shell completion (bash|zsh|fish|powershell)"
    );
    let _ = writeln!(out, "  larva-php --version                 Show version");
    let _ = writeln!(out, "  larva-php --help                    Show this help");
}

fn main() -> ExitCode {
    let args = Args::parse();

    if args.help {
        print_usage(&mut std::io::stdout());
        return ExitCode::SUCCESS;
    }

    if args.version {
        println!("LarvaPHP {}", VERSION);
        let version = larva_php::version();
        let version_str = unsafe {
            std::ffi::CStr::from_ptr(version.version)
                .to_string_lossy()
                .into_owned()
        };
        println!("PHP {}", version_str);
        return ExitCode::SUCCESS;
    }

    // Shell completion generation (exits early).
    if let Some(shell_name) = &args.completion {
        let bin = std::env::args()
            .next()
            .and_then(|p| {
                std::path::Path::new(&p)
                    .file_stem()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "larva-php".to_string());
        match larva_php::Shell::from_name(shell_name) {
            Some(shell) => {
                shell.write_to(&bin, &mut std::io::stdout()).ok();
                return ExitCode::SUCCESS;
            }
            None => {
                eprintln!(
                    "Unsupported shell '{}'. Supported: bash, zsh, fish, powershell",
                    shell_name
                );
                return ExitCode::from(1);
            }
        }
    }

    // Worker process mode (internal, spawned by the master via re-exec).
    if args.worker {
        return match &args.shm {
            Some(shm) => match larva_php::runtime::run_worker(shm) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("worker error: {}", e);
                    ExitCode::from(1)
                }
            },
            None => {
                eprintln!("--worker requires --shm <path>");
                ExitCode::from(1)
            }
        };
    }

    // Runtime demo: spawn a worker pool and dispatch a request (self-test).
    if args.runtime_demo {
        return runtime_demo();
    }

    // Direct FFI test: execute a script via the embedded runtime.
    if let Some(_script) = &args.ffi_test {
        tracing_subscriber::fmt::init();
        #[cfg(feature = "embedded")]
        {
            println!("embedded available: {}", larva_php::embed_php::is_available());
            // Run twice: the first run compiles+caches, the second should be a
            // cache hit (opcache) and noticeably faster.
            for i in 1..=2 {
                let t0 = std::time::Instant::now();
                match larva_php::embed_php::execute_script(_script) {
                    Ok(out) => {
                        let dt = t0.elapsed();
                        println!("run {}: {} bytes in {:?}", i, out.len(), dt);
                        if i == 2 {
                            println!("--- second run output ---");
                            println!("{}", String::from_utf8_lossy(&out));
                        }
                    }
                    Err(e) => {
                        eprintln!("FFI error on run {}: {}", i, e);
                        return ExitCode::from(1);
                    }
                }
            }
            return ExitCode::SUCCESS;
        }
        #[cfg(not(feature = "embedded"))]
        {
            eprintln!("built without --features embedded");
            return ExitCode::from(1);
        }
    }

    // Request-context FFI test: execute a script with full CGI request data.
    if let Some(script) = &args.ffi_request_test {
        tracing_subscriber::fmt::init();
        #[cfg(feature = "embedded")]
        {
            use larva_php::runtime::executor::Request;
            let req = Request {
                method: "GET".to_string(),
                uri: "/test.php?foo=bar&baz=qux".to_string(),
                headers: vec![
                    ("Host".to_string(), "127.0.0.1:8000".to_string()),
                    ("Cookie".to_string(), "session=abc123".to_string()),
                    ("User-Agent".to_string(), "LarvaPHP-Test/1.0".to_string()),
                ],
                body: Vec::new(),
                script_path: script.clone(),
                server_name: "127.0.0.1".to_string(),
                server_port: "8000".to_string(),
                script_name: "/test.php".to_string(),
            };
            let t0 = std::time::Instant::now();
            match larva_php::embed_php::execute_request(&req) {
                Ok(resp) => {
                    println!("status: {} in {:?}", resp.status, t0.elapsed());
                    println!("headers:");
                    for (k, v) in &resp.headers {
                        println!("  {}: {}", k, v);
                    }
                    println!("body ({} bytes):", resp.body.len());
                    println!("{}", String::from_utf8_lossy(&resp.body));
                    return ExitCode::SUCCESS;
                }
                Err(e) => {
                    eprintln!("request error: {}", e);
                    return ExitCode::from(1);
                }
            }
        }
        #[cfg(not(feature = "embedded"))]
        {
            eprintln!("built without --features embedded");
            return ExitCode::from(1);
        }
    }

    // Initialize tracing (DEBUG when --debug is set).
    if args.debug {
        use tracing::Level;
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }

    if let Some(root) = args.root {
        let addr: std::net::SocketAddr = match args.addr.parse() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("Invalid address '{}': {}", args.addr, e);
                return ExitCode::from(1);
            }
        };

        // Embedded mode: use the in-process PHP runtime (no php-cgi spawn).
        if args.embedded {
            #[cfg(feature = "embedded")]
            {
                std::env::set_var("LARVA_PHP_EXECUTOR", "ffi");
                tracing::info!("embedded PHP mode enabled (in-process, no php-cgi spawn)");
            }
            #[cfg(not(feature = "embedded"))]
            {
                eprintln!("--embedded requires building with --features embedded");
                return ExitCode::from(1);
            }
        }

        // Initialize PHP runtime
        let mut server = larva_php::server::Server::new(&root).unwrap();

        // Apply php.ini overrides from --php-ini KEY=VALUE flags.
        if !args.php_ini.is_empty() {
            let mut ini = std::collections::HashMap::new();
            for kv in &args.php_ini {
                match kv.split_once('=') {
                    Some((k, v)) => {
                        ini.insert(k.trim().to_string(), v.trim().to_string());
                    }
                    None => {
                        eprintln!("Invalid --php-ini value '{}' (expected KEY=VALUE)", kv);
                        return ExitCode::from(1);
                    }
                }
            }
            server.set_php_ini(ini);
        }

        server.activate();

        // Optionally start the 2-layer runtime worker pool (fault-isolated PHP).
        if args.workers > 0 {
            match larva_php::runtime::init_pool(args.workers) {
                Ok(()) => tracing::info!("runtime worker pool: {} workers", args.workers),
                Err(e) => {
                    eprintln!("Failed to start worker pool: {}", e);
                    return ExitCode::from(1);
                }
            }
        }

        // Start PHP main thread
        if let Err(e) = larva_php::thread::init(Vec::new()) {
            eprintln!("Failed to initialize PHP runtime: {}", e);
            return ExitCode::from(1);
        }

        // Optionally start the HTTP/3 (QUIC/UDP) listener on the same port.
        // Browsers that don't support HTTP/3 transparently keep using the TCP
        // server (HTTP/2 / HTTP/1.1).
        //
        // If --lets-encrypt is set, obtain a certificate via ACME first. The
        // HTTP-01 challenge tokens are served from the challenge directory
        // (default: <root>/.well-known/acme-challenge/), so the HTTP server
        // must be reachable on port 80 for validation.
        let h3_addr = std::net::SocketAddr::new(addr.ip(), addr.port()).to_string();
        let (h3_cert, h3_key) = match (args.cert.clone(), args.key.clone()) {
            (Some(c), Some(k)) => (c, k),
            _ => {
                let dir = std::env::temp_dir();
                let c = dir.join("larva-php-cert.pem").to_string_lossy().to_string();
                let k = dir.join("larva-php-key.pem").to_string_lossy().to_string();
                (c, k)
            }
        };

        #[cfg(feature = "http3")]
        if args.http3 || args.lets_encrypt {
            // Obtain a Let's Encrypt certificate if requested.
            let (le_cert, le_key) = if args.lets_encrypt {
                if args.acme_email.is_none() || args.acme_domains.is_none() {
                    eprintln!(
                        "--lets-encrypt requires --acme-email and --acme-domains"
                    );
                    return ExitCode::from(1);
                }
                let email = args.acme_email.as_deref().unwrap();
                let domains: Vec<String> = args
                    .acme_domains
                    .as_deref()
                    .unwrap_or("")
                    .split(',')
                    .map(|d| d.trim().to_string())
                    .filter(|d| !d.is_empty())
                    .collect();
                if domains.is_empty() {
                    eprintln!("--acme-domains must not be empty");
                    return ExitCode::from(1);
                }
                let cert_dir = args
                    .acme_dir
                    .as_deref()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        std::env::home_dir()
                            .unwrap_or_else(|| PathBuf::from("."))
                            .join(".larva-php")
                            .join("letsencrypt")
                    });
                let challenge_dir = args
                    .acme_challenge_dir
                    .as_deref()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        PathBuf::from(&root).join(".well-known/acme-challenge")
                    });

                #[cfg(feature = "acme")]
                {
                    match obtain_lets_encrypt_certs(
                        email,
                        &domains,
                        &cert_dir,
                        &challenge_dir,
                        args.acme_staging,
                        args.acme_renew_days,
                    )
                    .await
                    {
                        Ok(result) => {
                            tracing::info!(
                                "Let's Encrypt certificate obtained: {:?}",
                                result.cert_path
                            );
                            (
                                result.cert_path.to_string_lossy().to_string(),
                                result.key_path.to_string_lossy().to_string(),
                            )
                        }
                        Err(e) => {
                            eprintln!("Let's Encrypt certificate failed: {}", e);
                            return ExitCode::from(1);
                        }
                    }
                }
                #[cfg(not(feature = "acme"))]
                {
                    eprintln!(
                        "--lets-encrypt requires building with --features acme"
                    );
                    return ExitCode::from(1);
                }
            } else {
                (h3_cert.clone(), h3_key.clone())
            };

            let server_arc = std::sync::Arc::new(larva_php::server::Server::new(&root).unwrap());
            match larva_php::http3::ensure_cert(&le_cert, &le_key) {
                Ok((cert, key)) => {
                    let h3 = larva_php::http3::Http3Server::new(
                        h3_addr.clone(),
                        cert,
                        key,
                        server_arc,
                    );
                    // Advertise HTTP/3 on TCP responses via Alt-Svc.
                    larva_php::server::set_http3_port(addr.port());
                    std::thread::spawn(move || {
                        if let Err(e) = h3.run() {
                            eprintln!("HTTP/3 server error: {}", e);
                        }
                    });
                    tracing::info!("HTTP/3 (QUIC) listening on {}", h3_addr);
                }
                Err(e) => {
                    eprintln!("Failed to prepare HTTP/3 certificate: {}", e);
                    return ExitCode::from(1);
                }
            }
        }

        #[cfg(not(feature = "http3"))]
        if args.lets_encrypt {
            eprintln!(
                "--lets-encrypt requires building with --features http3,acme"
            );
            return ExitCode::from(1);
        }

        // Start HTTP server
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = server.start_server(addr).await {
                eprintln!("Server error: {}", e);
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        })
    } else if let Some(script) = args.script {
        let script_args: Vec<String> = std::env::args().skip(2).collect();
        let status = larva_php::execute_script_cli(&script, &script_args, Vec::new());
        ExitCode::from(status as u8)
    } else {
        print_usage(&mut std::io::stderr());
        ExitCode::from(1)
    }
}

/// Obtain a Let's Encrypt certificate via ACME v2 (HTTP-01 challenge).
///
/// Returns the [`CertResult`] with paths to the certificate, private key, and
/// full chain. On the first run this performs the full ACME flow (account
/// registration, order, challenge validation, finalize, download). On
/// subsequent runs it reuses the existing certificate if it is not near expiry.
#[cfg(feature = "acme")]
async fn obtain_lets_encrypt_certs(
    email: &str,
    domains: &[String],
    cert_dir: &std::path::Path,
    challenge_dir: &std::path::Path,
    staging: bool,
    renew_days: u32,
) -> larva_php::errors::Result<larva_php::acme::CertResult> {
    use std::path::PathBuf;

    let config = larva_php::acme::AcmeConfig {
        email: email.to_string(),
        domains: domains.to_vec(),
        cert_dir: cert_dir.to_path_buf(),
        challenge_dir: challenge_dir.to_path_buf(),
        staging,
        renew_days,
    };

    let client = larva_php::acme::AcmeClient::new(config)?;
    client.obtain_certificate().await
}

#[cfg(not(feature = "acme"))]
async fn obtain_lets_encrypt_certs(
    _email: &str,
    _domains: &[String],
    _cert_dir: &std::path::Path,
    _challenge_dir: &std::path::Path,
    _staging: bool,
    _renew_days: u32,
) -> larva_php::errors::Result<larva_php::acme::CertResult> {
    Err(crate::errors::Error::Acme(
        "acme feature not enabled".to_string(),
    ))
}

/// Self-test of the 2-layer runtime: spawn workers and dispatch a request.
fn runtime_demo() -> ExitCode {
    use larva_php::runtime::{Request, WorkerPool};

    tracing_subscriber::fmt::init();

    let pool = match WorkerPool::spawn(2) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("failed to spawn worker pool: {}", e);
            return ExitCode::from(1);
        }
    };

    // Give workers a moment to boot.
    std::thread::sleep(std::time::Duration::from_millis(300));

    let req = Request {
        method: "GET".to_string(),
        uri: "/index.php".to_string(),
        headers: vec![("host".to_string(), "localhost".to_string())],
        body: Vec::new(),
        script_path: "/app/index.php".to_string(),
        server_name: "localhost".to_string(),
        server_port: "80".to_string(),
        script_name: "/index.php".to_string(),
    };

    let mut ok = 0;
    for i in 0..5 {
        match pool.dispatch(&req) {
            Ok(resp) => {
                println!(
                    "request {} -> {} {}",
                    i + 1,
                    resp.status,
                    String::from_utf8_lossy(&resp.body)
                );
                ok += 1;
            }
            Err(e) => eprintln!("request {} failed: {}", i + 1, e),
        }
    }

    println!("runtime demo: {}/5 requests succeeded", ok);
    ExitCode::from(if ok == 5 { 0 } else { 1 })
}