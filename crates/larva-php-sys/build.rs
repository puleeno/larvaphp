use std::env;
use std::path::PathBuf;

#[path = "build/platform.rs"]
mod platform;

/// On Windows, ensure the MSVC environment (cl.exe, INCLUDE, LIB) is set up so
/// `cargo build` works from a plain shell. If cl.exe is already in PATH, this is
/// a no-op. Otherwise it locates Visual Studio via vswhere and imports the
/// environment by running vcvars64.bat.
#[cfg(windows)]
fn ensure_msvc_env() {
    // Already configured?
    let has_cl = env::var("PATH")
        .map(|p| p.split(';').any(|d| PathBuf::from(d).join("cl.exe").exists()))
        .unwrap_or(false);
    if has_cl && env::var("INCLUDE").is_ok() {
        return;
    }

    let vswhere = PathBuf::from(
        r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
    );
    if !vswhere.exists() {
        return; // No VS installer; let cc emit a clear error.
    }

    let vs_path = std::process::Command::new(&vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if vs_path.is_empty() {
        return;
    }

    let vcvars = PathBuf::from(&vs_path).join(r"VC\Auxiliary\Build\vcvars64.bat");
    if !vcvars.exists() {
        return;
    }

    // `cmd /c "vcvars64.bat && set"` prints the resulting environment.
    if let Ok(out) = std::process::Command::new("cmd")
        .args(["/c", &format!("\"{}\" && set", vcvars.display())])
        .output()
    {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Some((k, v)) = line.split_once('=') {
                env::set_var(k, v);
            }
        }
    }
}

#[cfg(not(windows))]
fn ensure_msvc_env() {}

fn main() {
    // Select exactly one platform strategy for this host.
    let platform = platform::select();
    ensure_msvc_env();

    println!("cargo:rerun-if-env-changed=PHP_LIB_DIR");
    println!("cargo:rerun-if-env-changed=STUB_SRC_DIR");
    println!("cargo:rerun-if-env-changed=PHP_INCLUDES");

    let php_lib_dir = PathBuf::from(
        env::var("PHP_LIB_DIR").unwrap_or_else(|_| platform.php_lib_dir_default()),
    );

    // Default to this crate's `src` directory; STUB_SRC_DIR can override it.
    let stub_src = PathBuf::from(env::var("STUB_SRC_DIR").unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .to_string_lossy()
            .to_string()
    }));

    let sources = platform.c_sources(&stub_src);
    for source in &sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }

    let mut build = cc::Build::new();
    build.include(&stub_src);
    for source in sources {
        if source.exists() {
            build.file(source);
        }
    }
    platform.configure_c(&mut build);
    build.compile("larva_php_bridge");

    platform.emit_link_directives(&php_lib_dir);

    // Manual FFI declarations are used; bindgen generation is intentionally skipped.
    if env::var("PHP_INCLUDES").is_ok() {
        println!("cargo:warning=PHP_INCLUDES set but bindgen skipped; using manual ffi.rs.");
    }

    println!("cargo:rustc-env=LARVA_BUILD_PLATFORM={}", platform.name());
}
