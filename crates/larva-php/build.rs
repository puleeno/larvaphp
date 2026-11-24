fn main() {
    println!("cargo:rerun-if-env-changed=PHP_LIB_DIR");
    println!("cargo:rerun-if-env-changed=PHP_EMBED_LIB");

    // The PHP embed library is discovered by larva-php-sys's build script and
    // linked as a dylib (e.g. libphp.so -> libphp8.3.so). Embed its location in
    // the binary's RUNPATH so the release binary runs without LD_LIBRARY_PATH.
    // `rustc-link-arg` emitted from a dependency's build script only applies to
    // that package's targets, so it must be emitted here for our binary.
    #[cfg(target_os = "linux")]
    {
        let php_lib_dir = std::env::var("PHP_LIB_DIR").unwrap_or_else(|_| "/usr/lib".to_string());
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{}", php_lib_dir);
    }

    #[cfg(target_os = "macos")]
    {
        let php_lib_dir = std::env::var("PHP_LIB_DIR")
            .unwrap_or_else(|_| "/usr/local/opt/php/lib".to_string());
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{}", php_lib_dir);
    }
}
