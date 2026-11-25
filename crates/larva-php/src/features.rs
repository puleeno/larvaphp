//! Per-virtual-host feature management with lazy initialization.
//!
//! # Concept
//!
//! All Cargo features are compiled in by default, but each virtual host
//! declares which features it actually wants through [`VhostFeatures`]. A
//! per-vhost [`FeatureManager`] then **lazily** initializes a feature the
//! first time it is needed (Strategy pattern: the `init` closure is the
//! strategy), and never touches disabled features at all — so a vhost that
//! does not use HTTP/3, ACME, or the embedded runtime pays zero
//! initialization cost (no thread spawn, no runtime creation, no cert I/O).
//!
//! # Design patterns
//!
//! - **Strategy**: [`FeatureManager::ensure`] takes an `init` closure that
//!   encapsulates how to load a specific feature. The manager decides *when*
//!   (lazily, once) and *whether* (only if enabled) to run it.
//! - **Registry**: [`FeatureKind`] is the registry key; [`VhostFeatures`] is
//!   the declarative set of enabled kinds.
//! - **Lazy initialization**: [`FeatureManager`] tracks loaded kinds in a
//!   `Mutex<HashSet>` and only runs `init` once per kind.

use std::collections::HashSet;
use std::sync::Mutex;

/// Runtime-toggleable feature kinds.
///
/// These mirror the Cargo features but are controllable per virtual host at
/// runtime. A kind is only initialized when a vhost enables it *and* the
/// feature is first used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureKind {
    /// HTTP/3 (QUIC/UDP) listener.
    Http3,
    /// Let's Encrypt / ACME automatic certificate provisioning.
    Acme,
    /// Embedded in-process PHP runtime (no php-cgi spawn).
    Embedded,
    /// Multi-process worker pool (fault-isolated PHP).
    Workers,
    /// Mercure real-time capabilities.
    Mercure,
    /// File watching / hot reload.
    Watcher,
    /// FastCGI client.
    Fcgi,
    /// Web control panel.
    Panel,
}

impl FeatureKind {
    /// All feature kinds, in declaration order.
    pub const ALL: &[FeatureKind] = &[
        FeatureKind::Http3,
        FeatureKind::Acme,
        FeatureKind::Embedded,
        FeatureKind::Workers,
        FeatureKind::Mercure,
        FeatureKind::Watcher,
        FeatureKind::Fcgi,
        FeatureKind::Panel,
    ];

    /// Canonical lowercase name (used in CLI parsing).
    pub fn name(&self) -> &'static str {
        match self {
            FeatureKind::Http3 => "http3",
            FeatureKind::Acme => "acme",
            FeatureKind::Embedded => "embedded",
            FeatureKind::Workers => "workers",
            FeatureKind::Mercure => "mercure",
            FeatureKind::Watcher => "watcher",
            FeatureKind::Fcgi => "fcgi",
            FeatureKind::Panel => "panel",
        }
    }

    /// Parse a feature name (case-insensitive).
    pub fn from_name(s: &str) -> Option<FeatureKind> {
        let s = s.trim().to_ascii_lowercase();
        FeatureKind::ALL.iter().copied().find(|k| k.name() == s)
    }
}

/// Per-virtual-host declaration of which features are enabled.
///
/// Defaults to **all** features enabled (matching the "enable everything by
/// default" requirement); a vhost opts out by disabling specific kinds.
#[derive(Debug, Clone)]
pub struct VhostFeatures {
    enabled: HashSet<FeatureKind>,
}

impl Default for VhostFeatures {
    fn default() -> Self {
        Self::all()
    }
}

impl VhostFeatures {
    /// All features enabled.
    pub fn all() -> Self {
        Self {
            enabled: FeatureKind::ALL.iter().copied().collect(),
        }
    }

    /// No features enabled.
    pub fn none() -> Self {
        Self {
            enabled: HashSet::new(),
        }
    }

    /// Enable a feature.
    pub fn enable(&mut self, kind: FeatureKind) -> &mut Self {
        self.enabled.insert(kind);
        self
    }

    /// Disable a feature.
    pub fn disable(&mut self, kind: FeatureKind) -> &mut Self {
        self.enabled.remove(&kind);
        self
    }

    /// Whether a feature is enabled for this vhost.
    pub fn is_enabled(&self, kind: FeatureKind) -> bool {
        self.enabled.contains(&kind)
    }

    /// Parse a comma-separated feature list (e.g. `"http3,acme,embedded"`).
    /// Unknown names are ignored. An empty string yields no features.
    pub fn parse(spec: &str) -> Self {
        let mut f = Self::none();
        for part in spec.split(',') {
            if let Some(kind) = FeatureKind::from_name(part) {
                f.enable(kind);
            }
        }
        f
    }

    /// Parse a `+`/`-` delta spec against a base set (e.g. `"+http3,-acme"`).
    /// `+` enables, `-` disables; a bare name enables.
    pub fn apply_delta(&mut self, spec: &str) -> &mut Self {
        for part in spec.split(',') {
            let part = part.trim();
            if let Some(rest) = part.strip_prefix('+') {
                if let Some(k) = FeatureKind::from_name(rest) {
                    self.enable(k);
                }
            } else if let Some(rest) = part.strip_prefix('-') {
                if let Some(k) = FeatureKind::from_name(rest) {
                    self.disable(k);
                }
            } else if let Some(k) = FeatureKind::from_name(part) {
                self.enable(k);
            }
        }
        self
    }

    /// Number of enabled features.
    pub fn len(&self) -> usize {
        self.enabled.len()
    }

    /// Whether no features are enabled.
    pub fn is_empty(&self) -> bool {
        self.enabled.is_empty()
    }

    /// Names of enabled features (for logging).
    pub fn enabled_names(&self) -> Vec<&'static str> {
        FeatureKind::ALL
            .iter()
            .filter(|k| self.enabled.contains(k))
            .map(|k| k.name())
            .collect()
    }
}

/// Per-virtual-host lazy feature manager.
///
/// Holds the vhost's [`VhostFeatures`] and tracks which features have already
/// been initialized. [`ensure`](FeatureManager::ensure) runs the supplied
/// `init` closure at most once per kind, and only for enabled kinds — disabled
/// features are never initialized (zero overhead).
pub struct FeatureManager {
    features: VhostFeatures,
    loaded: Mutex<HashSet<FeatureKind>>,
}

impl FeatureManager {
    /// Create a manager for the given feature set.
    pub fn new(features: VhostFeatures) -> Self {
        Self {
            features,
            loaded: Mutex::new(HashSet::new()),
        }
    }

    /// All-features manager (default).
    pub fn all() -> Self {
        Self::new(VhostFeatures::all())
    }

    /// The vhost's declared feature set.
    pub fn features(&self) -> &VhostFeatures {
        &self.features
    }

    /// Replace the feature set (e.g. after a panel hot-reload).
    pub fn set_features(&mut self, features: VhostFeatures) {
        self.features = features;
    }

    /// Whether a feature is enabled for this vhost (regardless of load state).
    pub fn is_enabled(&self, kind: FeatureKind) -> bool {
        self.features.is_enabled(kind)
    }

    /// Whether a feature has already been initialized.
    pub fn is_loaded(&self, kind: FeatureKind) -> bool {
        self.loaded.lock().unwrap().contains(&kind)
    }

    /// Lazily initialize a feature (Strategy pattern).
    ///
    /// - If the feature is **disabled** for this vhost, returns `false`
    ///   immediately without running `init` (zero overhead).
    /// - If already loaded, returns `true` without re-running `init`.
    /// - Otherwise runs `init` once; on success marks loaded and returns
    ///   `true`, on failure logs and returns `false`.
    pub fn ensure(&self, kind: FeatureKind, init: impl FnOnce() -> Result<(), String>) -> bool {
        if !self.features.is_enabled(kind) {
            return false;
        }
        let mut loaded = self.loaded.lock().unwrap();
        if loaded.contains(&kind) {
            return true;
        }
        match init() {
            Ok(()) => {
                loaded.insert(kind);
                tracing::info!("feature '{}' loaded", kind.name());
                true
            }
            Err(e) => {
                tracing::warn!("feature '{}' failed to load: {}", kind.name(), e);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all() {
        let f = VhostFeatures::default();
        for k in FeatureKind::ALL {
            assert!(f.is_enabled(*k), "{:?} should be enabled", k);
        }
    }

    #[test]
    fn parse_list() {
        let f = VhostFeatures::parse("http3, acme ,embedded");
        assert!(f.is_enabled(FeatureKind::Http3));
        assert!(f.is_enabled(FeatureKind::Acme));
        assert!(f.is_enabled(FeatureKind::Embedded));
        assert!(!f.is_enabled(FeatureKind::Panel));
        assert_eq!(f.len(), 3);
    }

    #[test]
    fn parse_empty_is_none() {
        assert!(VhostFeatures::parse("").is_empty());
    }

    #[test]
    fn apply_delta() {
        let mut f = VhostFeatures::all();
        f.apply_delta("-acme,+panel");
        assert!(!f.is_enabled(FeatureKind::Acme));
        assert!(f.is_enabled(FeatureKind::Panel));
        assert!(f.is_enabled(FeatureKind::Http3));
    }

    #[test]
    fn from_name_case_insensitive() {
        assert_eq!(FeatureKind::from_name("HTTP3"), Some(FeatureKind::Http3));
        assert_eq!(FeatureKind::from_name("  Acme "), Some(FeatureKind::Acme));
        assert_eq!(FeatureKind::from_name("nope"), None);
    }

    #[test]
    fn ensure_disabled_never_runs_init() {
        let mgr = FeatureManager::new(VhostFeatures::none());
        let mut ran = false;
        let ok = mgr.ensure(FeatureKind::Http3, || {
            ran = true;
            Ok(())
        });
        assert!(!ok);
        assert!(!ran, "init must not run for disabled feature");
        assert!(!mgr.is_loaded(FeatureKind::Http3));
    }

    #[test]
    fn ensure_runs_init_once() {
        let mgr = FeatureManager::all();
        let counter = std::sync::Arc::new(Mutex::new(0));
        let c = counter.clone();
        assert!(mgr.ensure(FeatureKind::Acme, move || {
            *c.lock().unwrap() += 1;
            Ok(())
        }));
        assert!(mgr.ensure(FeatureKind::Acme, || Ok(())));
        assert_eq!(*counter.lock().unwrap(), 1, "init must run exactly once");
        assert!(mgr.is_loaded(FeatureKind::Acme));
    }

    #[test]
    fn ensure_failure_not_marked_loaded() {
        let mgr = FeatureManager::all();
        let ok = mgr.ensure(FeatureKind::Http3, || Err("boom".into()));
        assert!(!ok);
        assert!(!mgr.is_loaded(FeatureKind::Http3));
    }
}