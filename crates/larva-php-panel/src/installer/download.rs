//! Binary download fallback (direct from official sources).

use std::path::{Path, PathBuf};

pub async fn download(url: &str, dest: &Path) -> Result<PathBuf, String> {
    let resp = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::write(dest, &bytes).await.map_err(|e| e.to_string())?;
    Ok(dest.to_path_buf())
}

pub fn install_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".larva-php").join("packages")
}