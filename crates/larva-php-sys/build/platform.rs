//! Build-time platform strategies for the `larva-php-sys` C bridge.
//!
//! # Pattern: Strategy
//!
//! Platform-specific compilation is encapsulated behind [`PlatformBuild`].
//! `build.rs` only talks to the trait; adding a new OS means adding one
//! implementation plus one branch in [`select`], without touching the build
//! logic or sprinkling `#[cfg(target_os = ...)]` across the crate.
//!
//! * [`WindowsPlatform`] keeps the original behaviour: compile the Win32 PHP
//!   shim and link against `php8.lib`.
//! * [`UnixPlatform`] is shared by Linux and macOS. It compiles the real PHP
//!   SAPI shim against the PHP development headers and links the PHP embed
//!   library. No no-op stubs are used in this path. Each OS still gets its own
//!   named strategy so the two builds can diverge later without code conflicts.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A platform-specific build strategy.
pub trait PlatformBuild {
    /// Short name used in diagnostics.
    fn name(&self) -> &'static str;

    /// C sources that implement the FFI bridge on this platform.
    fn c_sources(&self, src: &Path) -> Vec<PathBuf>;

    /// Extra preprocessor defines / include paths for the C compiler.
    fn configure_c(&self, _build: &mut cc::Build) {}

    /// Linker directives emitted to Cargo.
    fn emit_link_directives(&self, _php_lib_dir: &Path) {}

    /// Default value of `PHP_LIB_DIR`.
    fn php_lib_dir_default(&self) -> String {
        String::new()
    }
}

/// PHP header directories from `PHP_INCLUDES` or `php-config --includes`.
fn php_include_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(value) = std::env::var("PHP_INCLUDES") {
        for token in value.split_whitespace() {
            if let Some(dir) = token.strip_prefix("-I") {
                dirs.push(PathBuf::from(dir));
            }
        }
    }

    if dirs.is_empty() {
        if let Ok(output) = Command::new("php-config").arg("--includes").output() {
            for token in String::from_utf8_lossy(&output.stdout).split_whitespace() {
                if let Some(dir) = token.strip_prefix("-I") {
                    dirs.push(PathBuf::from(dir));
                }
            }
        }
    }

    dirs
}

/// Name of the PHP embed shared library, e.g. `php8.3`.
fn php_embed_lib() -> String {
    std::env::var("PHP_EMBED_LIB").unwrap_or_else(|_| "php8.3".to_string())
}

// ---------------------------------------------------------------------------
// Windows strategy
// ---------------------------------------------------------------------------

/// Original Windows build: real embedded PHP via `php8.dll`.
pub struct WindowsPlatform;
static WINDOWS: WindowsPlatform = WindowsPlatform;

impl PlatformBuild for WindowsPlatform {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn c_sources(&self, src: &Path) -> Vec<PathBuf> {
        vec![src.join("stubs.c"), src.join("larva_embed.c")]
    }

    fn configure_c(&self, build: &mut cc::Build) {
        let vc_include = std::env::var("INCLUDE").unwrap_or_default();
        for inc in vc_include.split(';') {
            let inc = inc.trim();
            if !inc.is_empty() {
                build.include(PathBuf::from(inc));
            }
        }

        // Windows SDK UCRT provides stddef.h and friends.
        let win_ucrt =
            PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt");
        if win_ucrt.exists() {
            build.include(win_ucrt);
        }

        build.define("_WINDOWS", None);
        build.define("WINDOWS", Some("1"));
        build.define("ZEND_WIN32", Some("1"));
        build.define("PHP_WIN32", Some("1"));
        build.define("WIN32", None);
        build.define("_CRT_SECURE_NO_WARNINGS", None);
        build.define("_CRT_NONSTDC_NO_DEPRECATE", None);
        // Do NOT force build.compiler("cl.exe"): that bypasses the cc crate's
        // vswhere-based MSVC detection and fails in shells without the MSVC env.
    }

    fn emit_link_directives(&self, php_lib_dir: &Path) {
        println!("cargo:rustc-link-search={}", php_lib_dir.display());
        println!("cargo:rustc-link-lib=php8");

        let sdk_lib_base = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\Lib");
        if let Ok(versions) = sdk_lib_base.read_dir() {
            let ver_dirs: Vec<_> = versions
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("10.0."))
                .collect();
            if let Some(latest) = ver_dirs.iter().max_by_key(|e| e.file_name()) {
                println!(
                    "cargo:rustc-link-search={}",
                    latest.path().join("um").join("x64").display()
                );
            }
        }
        for lib in &["shell32", "user32", "advapi32", "ws2_32", "bcrypt", "crypt32"] {
            println!("cargo:rustc-link-lib={}", lib);
        }
    }

    fn php_lib_dir_default(&self) -> String {
        r"C:\Program Files\PHP\current\dev".to_string()
    }
}

// ---------------------------------------------------------------------------
// Unix strategies (Linux / macOS / other Unix)
// ---------------------------------------------------------------------------

/// Shared Unix implementation, instantiated with a distinct name per OS.
pub struct UnixPlatform {
    name: &'static str,
}

pub static LINUX: UnixPlatform = UnixPlatform { name: "linux" };
pub static MACOS: UnixPlatform = UnixPlatform { name: "macos" };
pub static UNIX: UnixPlatform = UnixPlatform { name: "unix" };

impl PlatformBuild for UnixPlatform {
    fn name(&self) -> &'static str {
        self.name
    }

    fn c_sources(&self, src: &Path) -> Vec<PathBuf> {
        // Real PHP SAPI shim; no no-op stubs in release builds.
        vec![src.join("stubs.c"), src.join("larva_embed_unix.c")]
    }

    fn configure_c(&self, build: &mut cc::Build) {
        let dirs = php_include_dirs();
        if dirs.is_empty() {
            println!(
                "cargo:warning=PHP headers not found; set PHP_INCLUDES (e.g. `php-config --includes`)"
            );
        }
        for dir in dirs {
            build.include(dir);
        }
    }

    fn emit_link_directives(&self, php_lib_dir: &Path) {
        let lib = php_embed_lib();
        println!("cargo:rustc-link-search=native={}", php_lib_dir.display());
        println!("cargo:rustc-link-lib=dylib={}", lib);
        // Ship a runnable binary even when PHP lives outside the system paths.
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", php_lib_dir.display());
    }

    fn php_lib_dir_default(&self) -> String {
        if cfg!(target_os = "macos") {
            let lib = php_embed_lib();
            for dir in ["/opt/homebrew/lib", "/usr/local/lib"] {
                if Path::new(dir).join(format!("lib{}.dylib", lib)).exists() {
                    return dir.to_string();
                }
            }
        }
        "/usr/lib".to_string()
    }
}

// ---------------------------------------------------------------------------
// Strategy selection (compile-time, so conflicting platforms never mix)
// ---------------------------------------------------------------------------

/// Return the strategy for the host OS the build script is running on.
pub fn select() -> &'static dyn PlatformBuild {
    if cfg!(target_os = "windows") {
        &WINDOWS
    } else if cfg!(target_os = "linux") {
        &LINUX
    } else if cfg!(target_os = "macos") {
        &MACOS
    } else {
        &UNIX
    }
}
