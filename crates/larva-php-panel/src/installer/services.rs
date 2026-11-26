//! Service catalog: known third-party services/apps the panel can manage.

use crate::installer::{OsKind, PackageManager, Service};

pub fn catalog() -> Vec<Service> {
    let os = OsKind::detect();
    let pm = crate::installer::OsPackageManager { os };
    let pkgs: &[(&str, &str, &str, &str)] = &[
        ("php", "PHP", "PHP runtime", "php"),
        ("mysql", "MySQL", "MySQL database server", "mysql"),
        ("postgresql", "PostgreSQL", "PostgreSQL database server", "psql"),
        ("ssh", "OpenSSH", "SSH client/server", "ssh"),
        ("ftp", "vsftpd", "FTP server", "ftp"),
        ("curl", "curl", "HTTP client utility", "curl"),
        ("git", "Git", "Version control", "git"),
        ("nginx", "Nginx", "Web server (optional)", "nginx"),
    ];
    pkgs
        .iter()
        .map(|(id, name, desc, bin)| Service {
            id: id.to_string(),
            name: name.to_string(),
            description: desc.to_string(),
            installed: pm.is_installed(bin),
            version: None,
            install_cmd: format!("{}", id),
        })
        .collect()
}