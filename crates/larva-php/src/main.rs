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

    /// Primary domain name for the default virtual host (Nginx `server_name`).
    /// Defaults to "localhost".
    #[arg(long = "server-name", value_name = "NAME", default_value = "localhost")]
    server_name: String,

    /// Additional virtual host, repeatable. Format: `domain=root` or
    /// `domain=root;alias1,alias2` (aliases are Nginx/Apache ServerAlias-style).
    /// Each vhost is isolated (own document root, workers, php.ini) but shares
    /// the same listening port, selected by the request Host header.
    #[arg(long = "vhost", value_name = "DOMAIN=ROOT[;ALIASES]")]
    vhost: Vec<String>,

    /// Features enabled for the default virtual host (comma-separated).
    /// Default: all. Use `-name` to disable, e.g. `all,-http3` or
    /// `http3,acme,embedded`. Lazy-loaded: disabled features are never
    /// initialized (zero runtime overhead).
    #[arg(long = "features", value_name = "LIST")]
    features: Option<String>,

    /// Features for a specific virtual host, repeatable.
    /// Format: `domain=feat1,feat2` or `domain=+feat1,-feat2` (delta).
    #[arg(long = "vhost-features", value_name = "DOMAIN=LIST")]
    vhost_features: Vec<String>,

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
    #[arg(long = "lua-module", value_name = "FILE")]
    lua_module: Option<String>,

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

    /// Serve HTTPS directly on --addr using the certificate from --cert/--key
    /// or from --lets-encrypt. Requires --features tls.
    #[arg(long = "tls")]
    tls: bool,

    /// Path to a TLS certificate (PEM) for HTTP/3 and HTTPS.
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

    /// Enable the built-in web control panel (CyberPanel/OpenLiteSpeed style).
    /// Requires --features panel. Accessible at http://<addr>:<panel-port>.
    #[arg(long = "panel", value_name = "PORT")]
    panel: Option<String>,

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

/// Hot-reload bridge: the control panel calls this after website changes so the
/// live server rebuilds its virtual-host routing table. Each panel website
/// becomes an isolated [`Server`] vhost (own document root, workers, php.ini).
#[cfg(feature = "panel")]
struct RouterReloader {
    router: std::sync::Arc<larva_php::server::VHostRouter>,
}

struct WorkerProviderImpl;

#[cfg(feature = "panel")]
impl larva_php::panel::WorkerProvider for WorkerProviderImpl {
    fn list_workers(&self) -> Vec<larva_php::panel::api::WorkerStatus> {
        let mut out = Vec::new();
        if let Some(pool) = larva_php::runtime::pool() {
            for (id, pid, busy) in pool.status() {
                out.push(larva_php::panel::api::WorkerStatus {
                    id: id.to_string(),
                    name: format!("worker-{}", id),
                    status: if busy { "busy".into() } else { "running".into() },
                    pid,
                    requests: 0,
                    uptime: None,
                });
            }
        }
        out
    }
}

#[cfg(feature = "panel")]
impl larva_php::panel::VHostReloader for RouterReloader {
    fn reload(&self, websites: Vec<larva_php::panel::api::Website>) {
        let vhosts: Vec<std::sync::Arc<larva_php::server::Server>> = websites
            .into_iter()
            .filter(|w| w.is_active)
            .map(|w| {
                let aliases: Vec<String> = w
                    .acme_domains
                    .as_deref()
                    .unwrap_or("")
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                match larva_php::server::Server::new(&w.document_root) {
                    Ok(mut s) => {
                        s.set_domains(w.domain.clone(), aliases);
                        s.activate();
                        std::sync::Arc::new(s)
                    }
                    Err(_) => std::sync::Arc::new(larva_php::server::Server::fallback()),
                }
            })
            .collect();
        tracing::info!("panel hot-reload: {} active vhost(s)", vhosts.len());
        self.router.replace_all(vhosts);
    }
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

        // Initialize PHP runtime
        let mut server = larva_php::server::Server::new(&root).unwrap();

        // Resolve the default vhost's feature set (Strategy + Lazy). Start from
        // --features (absolute set, or delta on all when it contains '-'/'+'/
        // 'all'), then apply legacy flags additively for backward compat.
        let mut default_features = match &args.features {
            Some(spec) if spec.contains('-') || spec.contains('+') || spec.contains("all") => {
                let mut f = larva_php::VhostFeatures::all();
                f.apply_delta(spec);
                f
            }
            Some(spec) => larva_php::VhostFeatures::parse(spec),
            None => larva_php::VhostFeatures::all(),
        };
        if args.embedded {
            default_features.enable(larva_php::FeatureKind::Embedded);
        }
        if args.workers > 0 {
            default_features.enable(larva_php::FeatureKind::Workers);
        }
        if args.http3 {
            default_features.enable(larva_php::FeatureKind::Http3);
        }
        if args.lets_encrypt {
            default_features.enable(larva_php::FeatureKind::Acme);
        }
        if args.panel.is_some() {
            default_features.enable(larva_php::FeatureKind::Panel);
        }
        server.set_features(default_features);
        tracing::info!(
            "default vhost features: {}",
            server
                .features()
                .features()
                .enabled_names()
                .join(", ")
        );

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

        // Load Lua module if specified.
        #[cfg(feature = "lua")]
        if let Some(m) = &args.lua_module {
            match larva_php::modules::lua::load_lua_module(m) {
                Ok(()) => { tracing::info!("loaded lua module: {}", m); #[cfg(feature="lua")] tracing::info!("lua routes: {}", larva_php::modules::lua::routes().len()); }
                Err(e) => eprintln!("failed to load lua module: {}", e),
            }
        }

        // Embedded mode: use the in-process PHP runtime (lazy: only if enabled).
        if server.feature_enabled(larva_php::FeatureKind::Embedded) {
            #[cfg(feature = "embedded")]
            {
                std::env::set_var("LARVA_PHP_EXECUTOR", "ffi");
                tracing::info!("embedded PHP mode enabled (in-process, no php-cgi spawn)");
            }
            #[cfg(not(feature = "embedded"))]
            {
                eprintln!("embedded feature requires building with --features embedded");
                return ExitCode::from(1);
            }
        }

        // Optionally start the 2-layer runtime worker pool (lazy: only if the
        // Workers feature is enabled for this vhost).
        if args.workers > 0 && server.feature_enabled(larva_php::FeatureKind::Workers) {
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

        // HTTP/3 (QUIC) needs a certificate. If --lets-encrypt is set, obtain
        // a certificate via ACME first. The HTTP-01 challenge tokens are served
        // from the challenge directory (default:
        // <root>/.well-known/acme-challenge/), so the HTTP server must be
        // reachable on port 80 for validation.
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
        if server.feature_enabled(larva_php::FeatureKind::Http3)
            || server.feature_enabled(larva_php::FeatureKind::Acme)
        {
            // Obtain a Let's Encrypt certificate only when the user explicitly
            // requests it (--lets-encrypt). The Acme *feature* being compiled in
            // (default: all) must not force certificate provisioning.
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
                    server.set_acme_challenge_dir(challenge_dir.clone());
                }

                #[cfg(feature = "acme")]
                {
                    // ACME uses async reqwest; `main` is synchronous, so drive
                    // the future on a short-lived runtime before the server's
                    // own runtime is created below.
                    let acme_rt = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt,
                        Err(e) => {
                            eprintln!("failed to start ACME runtime: {}", e);
                            return ExitCode::from(1);
                        }
                    };
                    match acme_rt.block_on(obtain_lets_encrypt_certs(
                        email,
                        &domains,
                        &cert_dir,
                        &challenge_dir,
                        args.acme_staging,
                        args.acme_renew_days,
                    )) {
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

            let h3_addr = std::net::SocketAddr::new(addr.ip(), addr.port()).to_string();
            match larva_php::http3::ensure_cert(&le_cert, &le_key) {
                Ok((cert, key)) => {
                    let h3 = larva_php::http3::Http3Server::new(
                        h3_addr.clone(),
                        cert,
                        key,
                        std::sync::Arc::new(larva_php::server::Server::new(&root).unwrap()),
                    );
                    // Only advertise HTTP/3 when the operator explicitly asked
                    // for it (`--http3`). `--lets-encrypt` implicitly starts the
                    // UDP listener for backwards compatibility, but browsers
                    // that see `Alt-Svc: h3=...` (notably Waterfox/Firefox) may
                    // try QUIC and then stall instead of falling back quickly.
                    // Keeping the header off lets ALPN pick h2, or http/1.1
                    // when the client does not support h2.
                    if args.http3 {
                        larva_php::server::set_http3_port(addr.port());
                    } else {
                        tracing::info!(
                            "HTTP/3 listener active but Alt-Svc advertisement disabled; TCP clients use h2/http/1.1"
                        );
                    }
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
        if server.feature_enabled(larva_php::FeatureKind::Acme) {
            eprintln!(
                "acme feature requires building with --features http3,acme"
            );
            return ExitCode::from(1);
        }

        // Build the virtual-host registry (multi-tenant, one shared port).
        // The --root server is the default vhost; each --vhost spec adds an
        // isolated site selected by the request Host header.
        // Capture the Panel flag before `server` is moved into the vhost list.
        let panel_enabled = server.feature_enabled(larva_php::FeatureKind::Panel);
        server.name = args.server_name.clone();
        // Per-vhost feature deltas from --vhost-features DOMAIN=LIST.
        let mut vhost_feature_deltas: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for vf in &args.vhost_features {
            if let Some((domain, list)) = vf.split_once('=') {
                vhost_feature_deltas.insert(domain.trim().to_string(), list.to_string());
            } else {
                eprintln!("Invalid --vhost-features '{}' (expected DOMAIN=LIST)", vf);
                return ExitCode::from(1);
            }
        }
        let mut vhosts: Vec<std::sync::Arc<larva_php::server::Server>> =
            vec![std::sync::Arc::new(server)];
        for spec in &args.vhost {
            match larva_php::server::parse_vhost_spec(spec) {
                Ok((domain, vroot, aliases)) => match larva_php::server::Server::new(&vroot) {
                    Ok(mut v) => {
                        v.set_domains(domain.clone(), aliases);
                        // Per-vhost features: default all, apply delta if present.
                        if let Some(delta) = vhost_feature_deltas.get(&domain) {
                            let mut f = larva_php::VhostFeatures::all();
                            f.apply_delta(delta);
                            v.set_features(f);
                        }
                        v.activate();
                        tracing::info!("vhost '{}' -> {}", v.domain(), vroot);
                        vhosts.push(std::sync::Arc::new(v));
                    }
                    Err(e) => {
                        eprintln!("Invalid vhost root '{}': {}", vroot, e);
                        return ExitCode::from(1);
                    }
                },
                Err(e) => {
                    eprintln!("{}", e);
                    return ExitCode::from(1);
                }
            }
        }
        let router = std::sync::Arc::new(larva_php::server::VHostRouter::new(vhosts));

        // Start the web control panel if --panel is specified.
        // Runs in a COMPLETELY SEPARATE OS thread with its own Tokio runtime.
        // If the panel panics or crashes, it cannot affect the main PHP server.
        // The panel hot-reloads the vhost router above when websites change.
        if args.panel.is_some() && panel_enabled {
            #[cfg(feature = "panel")]
            {
                let panel_port = args.panel.as_deref().unwrap_or("9000");
                let port = panel_port.parse::<u16>().unwrap_or(9000);
                let panel_addr = format!("0.0.0.0:{}", port);
                tracing::info!("Starting isolated web control panel on http://{}", panel_addr);

                let reloader: std::sync::Arc<dyn larva_php::panel::VHostReloader> =
                    std::sync::Arc::new(RouterReloader {
                        router: router.clone(),
                    });

                std::thread::spawn(move || {
                    // Each panel thread gets its own Tokio runtime — fully isolated
                    // from the main server's runtime. A panic here is contained.
                    let rt = match tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(2)
                        .thread_name("larva-panel")
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt,
                        Err(e) => {
                            tracing::error!("failed to create panel tokio runtime: {}", e);
                            return;
                        }
                    };

                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let wp = std::sync::Arc::new(WorkerProviderImpl);
                        rt.block_on(larva_php::panel::start_with_reloader_and_workers(
                            &panel_addr,
                            Some(reloader),
                            Some(wp),
                        ))
                    }));

                    match result {
                        Ok(Err(e)) => tracing::error!("Panel server error: {}", e),
                        Err(panic_err) => {
                            tracing::error!("Panel thread panicked: {:?}", panic_err);
                        }
                        Ok(Ok(())) => {
                            tracing::info!("Panel server stopped");
                        }
                    }
                });

                // Give the panel a moment to bind before continuing.
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            #[cfg(not(feature = "panel"))]
            {
                eprintln!("--panel requires building with --features panel");
                return ExitCode::from(1);
            }
        }

        // Start HTTP server
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = larva_php::server::Server::serve(addr, router).await {
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
) -> larva_php::errors::Result<()> {
    Err(larva_php::errors::Error::Acme(
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