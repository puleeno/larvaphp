use std::env;
use std::path::PathBuf;

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

fn main() {
    #[cfg(windows)]
    ensure_msvc_env();

    // ---- PHP library directory ----
    let php_lib_dir = PathBuf::from(env::var("PHP_LIB_DIR").unwrap_or_else(|_| {
        r"C:\Program Files\PHP\current\dev".to_string()
    }));

    // ---- Stub C source directory ----
    let stub_src = PathBuf::from(env::var("STUB_SRC_DIR").unwrap_or_else(|_| {
        r"C:\Users\Administrator\Projects\LarvaPHP-Web-Server\larva-php\crates\larva-php-sys\src".to_string()
    }));

    println!("cargo:rerun-if-changed={}", stub_src.join("stubs.c").display());
    println!("cargo:rerun-if-changed={}", stub_src.join("larva_embed.c").display());
    println!("cargo:rerun-if-changed={}", stub_src.join("larva_embed.h").display());

    // ---- Link against php8.lib ----
    println!(
        "cargo:rustc-link-search={}",
        php_lib_dir.display()
    );
    println!("cargo:rustc-link-lib=php8");

    // ---- Windows system libraries ----
    if cfg!(target_os = "windows") {
        let sdk_lib_base = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\Lib");
        if let Ok(versions) = sdk_lib_base.read_dir() {
            let ver_dirs: Vec<_> = versions
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("10.0."))
                .collect();
            if let Some(latest) = ver_dirs.iter().max_by_key(|e| e.file_name()) {
                let sdk_lib = latest.path().join("um").join("x64");
                println!("cargo:rustc-link-search={}", sdk_lib.display());
            }
        }
        for lib in &["shell32", "user32", "advapi32", "ws2_32", "bcrypt", "crypt32"] {
            println!("cargo:rustc-link-lib={}", lib);
        }
    }

    // ---- Compile stubs.c using cc crate ----
    let stub_path = stub_src.join("stubs.c");
    let embed_path = stub_src.join("larva_embed.c");
    let mut build = cc::Build::new();
    if stub_path.exists() {
        build.file(stub_path);
    }
    if embed_path.exists() {
        build.file(embed_path);
    }

    // Add VC include directory explicitly
    if cfg!(target_os = "windows") {
        let vc_include = PathBuf::from(env::var("INCLUDE").unwrap_or_default());
        for inc in vc_include.to_string_lossy().split(';') {
            let inc = inc.trim();
            if !inc.is_empty() {
                build.include(PathBuf::from(inc));
            }
        }
        // Add Windows SDK UCRT for stddef.h, etc.
        let win_ucrt = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt");
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
        // cc auto-detects cl.exe via vswhere and sets up INCLUDE/LIB itself.
    }

    build.compile("larva_php_bridge");

    // ---- Bindgen (optional, skipped - using manual ffi.rs) ----
    let php_includes = env::var("PHP_INCLUDES").unwrap_or_default();
    if !php_includes.is_empty() {
        println!("cargo:warning=PHP_INCLUDES set but bindgen skipped; using manual ffi.rs.");
    }
}
