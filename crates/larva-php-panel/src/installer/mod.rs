//! Cross-platform OS package manager + binary download fallback.

pub mod download;
pub mod os;
pub mod services;

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OsKind {
    Windows,
    Macos,
    Linux,
    Unknown,
}

impl OsKind {
    pub fn detect() -> Self {
        match std::env::consts::OS {
            "windows" => Self::Windows,
            "macos" => Self::Macos,
            "linux" => Self::Linux,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    pub name: String,
    pub description: String,
    pub installed: bool,
    pub version: Option<String>,
    pub install_cmd: String,
}

pub trait PackageManager: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn install(&self, pkg: &str) -> Result<String, String>;
    fn uninstall(&self, pkg: &str) -> Result<String, String>;
    fn is_installed(&self, pkg: &str) -> bool;
}

pub struct OsPackageManager {
    pub os: OsKind,
}

impl OsPackageManager {
    pub fn new() -> Self {
        Self { os: OsKind::detect() }
    }

    pub fn command(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let out = Command::new(program).args(args).output().map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        if out.status.success() {
            Ok(stdout)
        } else {
            Err(format!("{}: {}", stderr, stdout))
        }
    }
}

impl PackageManager for OsPackageManager {
    fn name(&self) -> &'static str {
        match self.os {
            OsKind::Windows => "winget/choco",
            OsKind::Macos => "brew",
            OsKind::Linux => "apt/dnf/pacman",
            OsKind::Unknown => "none",
        }
    }
    fn is_available(&self) -> bool {
        match self.os {
            OsKind::Windows => which::which("winget").is_ok() || which::which("choco").is_ok(),
            OsKind::Macos => which::which("brew").is_ok(),
            OsKind::Linux => {
                which::which("apt").is_ok()
                    || which::which("dnf").is_ok()
                    || which::which("pacman").is_ok()
                    || which::which("apk").is_ok()
            }
            OsKind::Unknown => false,
        }
    }
    fn install(&self, pkg: &str) -> Result<String, String> {
        match self.os {
            OsKind::Windows => {
                if which::which("winget").is_ok() {
                    self.command("winget", &["install", "-e", "--id", pkg, "--silent", "--accept-package-agreements", "--accept-source-agreements"])
                } else if which::which("choco").is_ok() {
                    self.command("choco", &["install", pkg, "-y"])
                } else {
                    Err("no package manager available (winget/choco)".into())
                }
            }
            OsKind::Macos => self.command("brew", &["install", pkg]),
            OsKind::Linux => {
                if which::which("apt").is_ok() {
                    self.command("sudo", &["apt", "install", "-y", pkg])
                } else if which::which("dnf").is_ok() {
                    self.command("sudo", &["dnf", "install", "-y", pkg])
                } else if which::which("pacman").is_ok() {
                    self.command("sudo", &["pacman", "-S", "--noconfirm", pkg])
                } else if which::which("apk").is_ok() {
                    self.command("sudo", &["apk", "add", pkg])
                } else {
                    Err("no package manager available".into())
                }
            }
            OsKind::Unknown => Err("unsupported OS".into()),
        }
    }
    fn uninstall(&self, pkg: &str) -> Result<String, String> {
        match self.os {
            OsKind::Windows => {
                if which::which("winget").is_ok() {
                    self.command("winget", &["uninstall", "-e", "--id", pkg, "--silent"])
                } else if which::which("choco").is_ok() {
                    self.command("choco", &["uninstall", pkg, "-y"])
                } else {
                    Err("no package manager".into())
                }
            }
            OsKind::Macos => self.command("brew", &["uninstall", pkg]),
            OsKind::Linux => {
                if which::which("apt").is_ok() {
                    self.command("sudo", &["apt", "remove", "-y", pkg])
                } else if which::which("dnf").is_ok() {
                    self.command("sudo", &["dnf", "remove", "-y", pkg])
                } else if which::which("pacman").is_ok() {
                    self.command("sudo", &["pacman", "-R", "--noconfirm", pkg])
                } else if which::which("apk").is_ok() {
                    self.command("sudo", &["apk", "del", pkg])
                } else {
                    Err("no package manager".into())
                }
            }
            OsKind::Unknown => Err("unsupported OS".into()),
        }
    }
    fn is_installed(&self, pkg: &str) -> bool {
        which::which(pkg).is_ok()
    }
}