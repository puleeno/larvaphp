//! OS detection helpers.

use super::OsKind;

pub fn current() -> OsKind {
    OsKind::detect()
}

pub fn os_name() -> &'static str {
    std::env::consts::OS
}

pub fn arch() -> &'static str {
    std::env::consts::ARCH
}