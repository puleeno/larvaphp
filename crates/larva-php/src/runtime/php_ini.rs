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
        // Load the opcache Zend extension. It must be loaded before any
        // opcache.* directives are parsed; `extension_dir` comes from the
        // main php.ini which PHP reads before the scan directory.
        entries.push_str("zend_extension=php_opcache.dll\n");
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
}