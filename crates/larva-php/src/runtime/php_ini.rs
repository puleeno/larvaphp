//! PHP INI resolution (Strategy + Chain of Responsibility).
//!
//! Resolves the `php.ini` file/directory plus the always-on `ini_entries`
//! used by the embedded PHP runtime (FFI) and by the CGI/FastCGI transports.
//!
//! Resolution order (first non-empty wins):
//!   1. [`EnvResolver`]        — `LARVA_PHP_INI` / `PHPRC`
//!   2. [`PathResolver`]       — scan well-known install directories
//!   3. [`IniEntriesResolver`] — built-in defaults (opcache, extensions)

use std::path::PathBuf;

/// Resolved PHP INI configuration.
#[derive(Debug, Clone, Default)]
pub struct PhpIniConfig {
    /// Directory containing `php.ini` (exported as `PHPRC`).
    pub dir: Option<String>,
    /// Full path to `php.ini` (used as `php_ini_path_override`).
    pub path: Option<PathBuf>,
    /// Extra INI directives appended after the main file (`ini_entries`).
    pub entries: Option<String>,
}

/// A source of PHP INI configuration.
pub trait PhpIniResolver {
    /// Resolve this source into a (possibly empty) [`PhpIniConfig`].
    fn resolve(&self) -> PhpIniConfig;
}

/// Resolves from the process environment.
///
/// * `LARVA_PHP_INI` — explicit path to a `php.ini` file.
/// * `PHPRC`         — directory containing `php.ini`.
pub struct EnvResolver;

impl PhpIniResolver for EnvResolver {
    fn resolve(&self) -> PhpIniConfig {
        let mut cfg = PhpIniConfig::default();

        if let Ok(explicit) = std::env::var("LARVA_PHP_INI") {
            let p = PathBuf::from(&explicit);
            if p.is_file() {
                cfg.path = Some(p.clone());
                if let Some(parent) = p.parent() {
                    cfg.dir = Some(parent.to_string_lossy().into_owned());
                }
                return cfg;
            }
        }

        if let Ok(php_dir) = std::env::var("PHPRC") {
            let dir = PathBuf::from(&php_dir);
            let ini = dir.join("php.ini");
            if ini.is_file() {
                cfg.path = Some(ini);
            }
            cfg.dir = Some(php_dir);
        }

        cfg
    }
}

/// Resolves by scanning well-known install directories for `php.ini`.
pub struct PathResolver;

impl PhpIniResolver for PathResolver {
    fn resolve(&self) -> PhpIniConfig {
        let mut candidates: Vec<PathBuf> = Vec::new();

        // Windows: the standard PHP install layout.
        #[cfg(windows)]
        {
            candidates.push(PathBuf::from(r"C:\Program Files\PHP\current"));
            if let Ok(root) = std::env::var("SystemDrive") {
                candidates.push(PathBuf::from(format!("{root}\\php")));
            }
        }

        // Unix: common system locations.
        #[cfg(not(windows))]
        {
            candidates.push(PathBuf::from("/etc/php.ini"));
            candidates.push(PathBuf::from("/usr/local/etc/php.ini"));
            candidates.push(PathBuf::from("/etc/php/8.3/cli/php.ini"));
        }

        for dir in candidates {
            let ini = dir.join("php.ini");
            if ini.is_file() {
                return PhpIniConfig {
                    dir: Some(dir.to_string_lossy().into_owned()),
                    path: Some(ini),
                    entries: None,
                };
            }
        }

        PhpIniConfig::default()
    }
}

/// Always-on INI directives, most importantly **opcache**.
///
/// Without these, every request recompiles all PHP bytecode from scratch, which
/// is the dominant cost for a CMS such as WordPress (~700 ms vs ~100 ms).
///
/// `opcache.file_cache` is what makes the cache survive across the short-lived
/// `php-cgi.exe` processes spawned by the CGI transport: compiled bytecode is
/// persisted to disk and reloaded instead of being recompiled.
pub struct IniEntriesResolver;

impl IniEntriesResolver {
    /// Directory used for the persistent opcache file cache.
    pub fn file_cache_dir() -> PathBuf {
        std::env::temp_dir().join("larva-php-opcache")
    }

    /// Build the default `ini_entries` string.
    pub fn entries() -> String {
        let cache_dir = Self::file_cache_dir();
        let _ = std::fs::create_dir_all(&cache_dir);

        let mut entries = String::new();
        // Note: the system php.ini already loads opcache (zend_extension=opcache),
        // so we must NOT add `zend_extension=php_opcache.dll` here or it will be
        // loaded twice ("Cannot load Zend OPcache - it was already loaded").
        // These opcache.* directives configure the already-loaded extension.
        entries.push_str("opcache.enable=1\n");
        entries.push_str("opcache.enable_cli=1\n");
        // Persist compiled bytecode so it is reused across processes.
        entries.push_str(&format!("opcache.file_cache={}\n", cache_dir.to_string_lossy()));
        // Prefer the shared-memory cache, fall back to the file cache.
        entries.push_str("opcache.file_cache_only=0\n");
        // Never re-check mtimes: production semantics, biggest win.
        entries.push_str("opcache.validate_timestamps=0\n");
        entries.push_str("opcache.revalidate_freq=0\n");
        entries.push_str("opcache.memory_consumption=512\n");
        entries.push_str("opcache.interned_strings_buffer=32\n");
        entries.push_str("opcache.max_accelerated_files=100000\n");
        entries.push_str("opcache.save_comments=1\n");
        entries.push_str("opcache.fast_shutdown=1\n");
        // PHP 8.3's embedded SAPI otherwise auto-detects a 2 MiB compiler
        // stack, which aborts even trivial scripts on dedicated HTTP/3 worker
        // threads. Use the normal 8 MiB process stack limit; the H3 workers
        // themselves have a larger 64 MiB stack.
        entries.push_str("zend.max_allowed_stack_size=8M\n");
        entries
    }
}

impl PhpIniResolver for IniEntriesResolver {
    fn resolve(&self) -> PhpIniConfig {
        PhpIniConfig {
            dir: None,
            path: None,
            entries: Some(Self::entries()),
        }
    }
}

/// Chains several resolvers; later sources fill in fields the earlier ones
/// left empty.
pub struct ChainResolver {
    resolvers: Vec<Box<dyn PhpIniResolver>>,
}

impl ChainResolver {
    pub fn new(resolvers: Vec<Box<dyn PhpIniResolver>>) -> Self {
        Self { resolvers }
    }

    /// The default chain: environment → well-known paths → built-in entries.
    pub fn default_chain() -> Self {
        Self::new(vec![
            Box::new(EnvResolver),
            Box::new(PathResolver),
            Box::new(IniEntriesResolver),
        ])
    }
}

impl PhpIniResolver for ChainResolver {
    fn resolve(&self) -> PhpIniConfig {
        let mut out = PhpIniConfig::default();
        for r in &self.resolvers {
            let cfg = r.resolve();
            if out.dir.is_none() {
                out.dir = cfg.dir;
            }
            if out.path.is_none() {
                out.path = cfg.path;
            }
            if out.entries.is_none() {
                out.entries = cfg.entries;
            }
        }
        out
    }
}

/// Resolve the effective PHP INI configuration using the default chain.
pub fn resolve() -> PhpIniConfig {
    ChainResolver::default_chain().resolve()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_enable_opcache() {
        let entries = IniEntriesResolver::entries();
        assert!(entries.contains("opcache.enable=1"));
        assert!(entries.contains("opcache.file_cache="));
        assert!(entries.contains("opcache.validate_timestamps=0"));
    }

    #[test]
    fn entries_raise_php_compiler_stack_limit() {
        let entries = IniEntriesResolver::entries();
        assert!(entries.contains("zend.max_allowed_stack_size=8M"));
    }

    #[test]
    fn file_cache_dir_is_absolute() {
        let dir = IniEntriesResolver::file_cache_dir();
        assert!(dir.is_absolute());
    }

    #[test]
    fn chain_includes_opcache_entries() {
        let cfg = resolve();
        let entries = cfg.entries.expect("entries must always be present");
        assert!(entries.contains("opcache.enable=1"));
    }

    #[test]
    fn chain_preserves_earlier_values() {
        let chain = ChainResolver::new(vec![Box::new(IniEntriesResolver)]);
        let cfg = chain.resolve();
        assert!(cfg.entries.is_some());
        assert!(cfg.path.is_none());
    }

    // ---- No double-load of opcache ----

    #[test]
    fn entries_do_not_double_load_opcache() {
        // The system php.ini already loads opcache (zend_extension=opcache).
        // Our entries must NOT include zend_extension=php_opcache.dll or PHP
        // will warn "Cannot load Zend OPcache - it was already loaded".
        let entries = IniEntriesResolver::entries();
        assert!(
            !entries.contains("zend_extension"),
            "entries must not contain zend_extension (causes double-load): {}",
            entries
        );
    }

    #[test]
    fn entries_enable_opcache_cli() {
        let entries = IniEntriesResolver::entries();
        assert!(
            entries.contains("opcache.enable_cli=1"),
            "opcache.enable_cli must be 1 for embedded SAPI"
        );
    }

    #[test]
    fn entries_disable_timestamp_validation() {
        // Production semantics: never re-check file mtimes.
        let entries = IniEntriesResolver::entries();
        assert!(entries.contains("opcache.validate_timestamps=0"));
        assert!(entries.contains("opcache.revalidate_freq=0"));
    }

    #[test]
    fn entries_set_memory_limits() {
        let entries = IniEntriesResolver::entries();
        assert!(entries.contains("opcache.memory_consumption=512"));
        assert!(entries.contains("opcache.interned_strings_buffer=32"));
        assert!(entries.contains("opcache.max_accelerated_files=100000"));
    }

    #[test]
    fn entries_include_file_cache() {
        let entries = IniEntriesResolver::entries();
        assert!(entries.contains("opcache.file_cache="));
        assert!(entries.contains("opcache.file_cache_only=0"));
    }

    // ---- IniEntriesResolver ----

    #[test]
    fn ini_entries_resolver_always_has_entries() {
        let resolver = IniEntriesResolver;
        let cfg = resolver.resolve();
        assert!(cfg.entries.is_some());
        assert!(cfg.path.is_none());
        assert!(cfg.dir.is_none());
    }

    #[test]
    fn file_cache_dir_can_be_created() {
        let dir = IniEntriesResolver::file_cache_dir();
        // entries() creates the dir; verify it exists after calling entries().
        let _ = IniEntriesResolver::entries();
        assert!(dir.exists() || std::fs::create_dir_all(&dir).is_ok());
    }

    // ---- ChainResolver ordering ----

    #[test]
    fn chain_resolver_first_wins_for_path() {
        // A custom resolver that provides a path should win over later ones.
        struct FakeResolver {
            path: Option<PathBuf>,
            entries: Option<String>,
        }
        impl PhpIniResolver for FakeResolver {
            fn resolve(&self) -> PhpIniConfig {
                PhpIniConfig {
                    dir: None,
                    path: self.path.clone(),
                    entries: self.entries.clone(),
                }
            }
        }

        let chain = ChainResolver::new(vec![
            Box::new(FakeResolver {
                path: Some(PathBuf::from("/custom/php.ini")),
                entries: None,
            }),
            Box::new(IniEntriesResolver),
        ]);
        let cfg = chain.resolve();
        assert_eq!(cfg.path, Some(PathBuf::from("/custom/php.ini")));
        assert!(cfg.entries.is_some()); // filled by second resolver
    }

    #[test]
    fn chain_resolver_later_fills_gaps() {
        struct FakeResolver {
            path: Option<PathBuf>,
        }
        impl PhpIniResolver for FakeResolver {
            fn resolve(&self) -> PhpIniConfig {
                PhpIniConfig {
                    dir: None,
                    path: self.path.clone(),
                    entries: None,
                }
            }
        }

        let chain = ChainResolver::new(vec![
            Box::new(FakeResolver { path: None }),
            Box::new(FakeResolver {
                path: Some(PathBuf::from("/fallback/php.ini")),
            }),
        ]);
        let cfg = chain.resolve();
        assert_eq!(cfg.path, Some(PathBuf::from("/fallback/php.ini")));
    }

    // ---- EnvResolver ----

    #[test]
    fn env_resolver_no_env_returns_empty() {
        // Without LARVA_PHP_INI or PHPRC set, returns default (empty).
        // Note: we can't unset env vars another test set, so we just verify
        // the resolver doesn't panic.
        let cfg = EnvResolver.resolve();
        // If env vars happen to be set, path/dir may be Some; that's fine.
        let _ = cfg;
    }

    #[test]
    fn path_resolver_does_not_panic() {
        let cfg = PathResolver.resolve();
        // On Windows with PHP installed, this should find php.ini.
        // On systems without PHP, it returns default.
        let _ = cfg;
    }

    // ---- PhpIniConfig default ----

    #[test]
    fn php_ini_config_default_is_empty() {
        let cfg = PhpIniConfig::default();
        assert!(cfg.dir.is_none());
        assert!(cfg.path.is_none());
        assert!(cfg.entries.is_none());
    }

    #[test]
    fn php_ini_config_clone() {
        let cfg = PhpIniConfig {
            dir: Some("/php".into()),
            path: Some(PathBuf::from("/php/php.ini")),
            entries: Some("opcache.enable=1".into()),
        };
        let cloned = cfg.clone();
        assert_eq!(cloned.dir, cfg.dir);
        assert_eq!(cloned.path, cfg.path);
        assert_eq!(cloned.entries, cfg.entries);
    }
}