//! LarvaPHP - a modern PHP application server written in Rust.
//!
//! This is a Rust port of [FrankenPHP](https://frankenphp.dev), embedding PHP
//! via the Zend Engine C API and providing an HTTP server built on Hyper/Tokio.
//!
//! # Architecture
//!
//! - [`ffi`] - Raw FFI bindings to the Zend Engine (via `larva-php-sys`)
//! - [`types`] - Safe wrappers for zval conversion (PHP ↔ Rust)
//! - [`env`] - Environment variable management
//! - [`options`] - Configuration options
//! - [`context`] - Per-request context
//! - [`errors`] - Error types
//! - [`phpinfo`] - phpinfo() integration
//! - [`thread`] - PHP thread management and state machine
//! - [`worker`] - Worker mode (persistent PHP scripts)
//! - [`server`] - Server blocks (scoped request handling)
//! - [`cgi`] - CGI variable registration and path splitting
//! - [`metrics`] - Metrics collection (Prometheus-compatible)
//! - [`scaling`] - Auto-scaling of PHP threads
//! - [`mercure`] - Mercure real-time capabilities
//! - [`watcher`] - File watching and hot reload
//! - [`embed`] - Embedded PHP application support
//! - [`cli`] - PHP CLI execution

pub mod cgi;
pub mod cli;
pub mod completions;
pub mod config;
pub mod context;
pub mod embed;
pub mod embed_php;
pub mod env;
pub mod edge_cache;
pub mod errors;
pub mod fastcgi;
pub mod features;
pub mod http3;
pub mod listener;
pub mod metrics;
pub mod options;
pub mod path_resolver;
pub mod phpinfo;
pub mod runtime;
pub mod scaling;
pub mod server;
pub mod thread;
#[cfg(feature = "tls")]
pub mod tls;
pub mod types;
pub mod worker;

#[cfg(feature = "acme")]
pub mod acme;

#[cfg(feature = "panel")]
pub use larva_php_panel as panel;

#[cfg(feature = "mercure")]
pub mod mercure;

#[cfg(feature = "watcher")]
pub mod watcher;

pub use config::hot_reload::HotReloadManager;
pub use config::{
    ConfigLoader, ConfigParser, Middleware, MiddlewareChain, RewriteMiddleware, RewriteRule,
    ServerConfig, ServerConfigBuilder,
};
pub use completions::Shell;
pub use errors::{Error, Result};
pub use features::{FeatureKind, FeatureManager, VhostFeatures};
pub use options::{Option, WorkerOption, ServerOption, RequestOption};
pub use path_resolver::{
    default_resolver, make_native, resolve_root, PathResolver, UnixPathResolver,
    WindowsPathResolver,
};
pub use server::Server;

pub use larva_php_sys as ffi;

/// Version of LarvaPHP.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Server header value sent in responses.
pub const SERVER_HEADER: &str = "LarvaPHP";

/// Default maximum consecutive worker failures before panicking.
pub const DEFAULT_MAX_CONSECUTIVE_FAILURES: i32 = 6;

/// Default document root.
pub const DEFAULT_DOCUMENT_ROOT: &str = "public";

/// Default watch pattern.
pub const DEFAULT_WATCH_PATTERN: &str = "./**/*.{env,php,twig,yaml,yml}";

/// Default hot reload pattern.
pub const DEFAULT_HOT_RELOAD_PATTERN: &str = "./**/*.{css,env,gif,htm,html,jpg,jpeg,js,mjs,php,png,svg,twig,webp,xml,yaml,yml}";

/// Initialize the PHP runtime and start workers.
///
/// This is the main entry point. It mirrors the `frankenphp.Init()` function
/// in the Go source.
pub fn init(options: Vec<Box<dyn Option>>) -> Result<()> {
    crate::thread::init(options)
}

/// Shutdown the PHP runtime.
///
/// This mirrors `frankenphp.Shutdown()`.
pub fn shutdown() {
    crate::thread::shutdown();
}

/// Validate options without starting the runtime.
///
/// This mirrors `frankenphp.Validate()`.
pub fn validate(options: &[Box<dyn Option>]) -> Result<()> {
    crate::thread::validate(options)
}

/// Get the PHP version information.
pub fn version() -> ffi::frankenphp_version {
    unsafe { ffi::frankenphp_get_version() }
}

/// Get the PHP configuration.
pub fn config() -> ffi::frankenphp_config {
    unsafe { ffi::frankenphp_get_config() }
}

/// Execute a PHP script via the CLI SAPI.
///
/// This mirrors `frankenphp.ExecuteScriptCLI()`.
pub fn execute_script_cli(script: &str, args: &[String], options: Vec<Box<dyn Option>>) -> i32 {
    crate::cli::execute_script_cli(script, args, options)
}