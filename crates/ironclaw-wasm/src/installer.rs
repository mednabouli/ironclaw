//! Plugin installer — download and install WASM plugins from URLs or registries.

use std::path::{Path, PathBuf};

use tracing::{info, warn};

use crate::manifest::{PluginManifest, PluginRegistry};

/// Default plugin directory under the user's home.
pub fn default_plugin_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".ironclaw").join("plugins")
}

/// Result of a plugin installation.
#[derive(Debug)]
pub struct InstallResult {
    /// Plugin name.
    pub name: String,
    /// Where the .wasm file was saved.
    pub wasm_path: PathBuf,
    /// Where the plugin.json manifest was saved.
    pub manifest_path: PathBuf,
}

/// Install a plugin from a direct URL.
///
/// Downloads the `.wasm` file and saves it to `plugin_dir/{name}.wasm`.
/// If `manifest` is provided, writes `plugin_dir/{name}/plugin.json`.
pub async fn install_from_url(
    url: &str,
    plugin_dir: &Path,
    manifest: Option<&PluginManifest>,
) -> anyhow::Result<InstallResult> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    info!(url = %url, "Downloading plugin");

    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;

    // Derive name from manifest or URL filename
    let name = if let Some(m) = manifest {
        m.name.clone()
    } else {
        url_to_plugin_name(url)
    };

    // Create plugin subdirectory
    let sub_dir = plugin_dir.join(&name);
    tokio::fs::create_dir_all(&sub_dir).await?;

    // Write .wasm file
    let wasm_path = sub_dir.join(format!("{name}.wasm"));
    tokio::fs::write(&wasm_path, &bytes).await?;
    info!(path = %wasm_path.display(), size = bytes.len(), "Plugin saved");

    // Verify SHA-256 if manifest provides one
    if let Some(m) = manifest {
        if let Some(expected_hash) = &m.sha256 {
            let actual_hash = sha256_hex(&bytes);
            if actual_hash != *expected_hash {
                // Clean up the downloaded file
                let _ = tokio::fs::remove_file(&wasm_path).await;
                anyhow::bail!(
                    "SHA-256 mismatch for '{}': expected {}, got {}",
                    name,
                    expected_hash,
                    actual_hash
                );
            }
            info!("SHA-256 verified");
        }
    }

    // Write manifest
    let manifest_path = sub_dir.join("plugin.json");
    if let Some(m) = manifest {
        let json = serde_json::to_string_pretty(m)?;
        tokio::fs::write(&manifest_path, json).await?;
    } else {
        // Write a minimal manifest derived from the URL
        let minimal = PluginManifest {
            name: name.clone(),
            version: "0.0.0".into(),
            description: format!("Plugin installed from {url}"),
            author: "unknown".into(),
            license: "unknown".into(),
            capabilities: vec![],
            allowed_urls: vec![],
            allowed_env_vars: vec![],
            parameters: serde_json::json!({"type": "object", "properties": {}}),
            download_url: Some(url.into()),
            sha256: None,
        };
        let json = serde_json::to_string_pretty(&minimal)?;
        tokio::fs::write(&manifest_path, json).await?;
    }

    Ok(InstallResult {
        name,
        wasm_path,
        manifest_path,
    })
}

/// Install a plugin by name from a registry.
///
/// Looks up the plugin in the registry, resolves the download URL,
/// and installs it.
pub async fn install_from_registry(
    name: &str,
    registry: &PluginRegistry,
    plugin_dir: &Path,
) -> anyhow::Result<InstallResult> {
    let manifest = registry
        .find(name)
        .ok_or_else(|| anyhow::anyhow!("Plugin '{name}' not found in registry"))?;

    let download_url = if let Some(ref u) = manifest.download_url {
        if u.starts_with("http://") || u.starts_with("https://") {
            u.clone()
        } else if let Some(ref base) = registry.base_url {
            format!("{}/{}", base.trim_end_matches('/'), u)
        } else {
            anyhow::bail!("Plugin '{name}' has a relative URL but registry has no base_url");
        }
    } else if let Some(ref base) = registry.base_url {
        format!(
            "{}/{}/{}/{}.wasm",
            base.trim_end_matches('/'),
            name,
            manifest.version,
            name
        )
    } else {
        anyhow::bail!("Plugin '{name}' has no download URL and registry has no base_url");
    };

    install_from_url(&download_url, plugin_dir, Some(manifest)).await
}

/// List installed plugins in a directory.
pub fn list_installed(plugin_dir: &Path) -> Vec<PluginManifest> {
    let Ok(entries) = std::fs::read_dir(plugin_dir) else {
        return vec![];
    };

    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let manifest_path = e.path().join("plugin.json");
            PluginManifest::from_file(&manifest_path).ok()
        })
        .collect()
}

/// Extract a plugin name from a URL.
fn url_to_plugin_name(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("unknown")
        .trim_end_matches(".wasm")
        .to_string()
}

/// Compute SHA-256 hex digest of bytes.
///
/// Stub implementation — returns an empty string and logs a warning.
/// A full implementation would use the `sha2` crate or `ring`.
fn sha256_hex(data: &[u8]) -> String {
    let _ = data;
    warn!("SHA-256 verification requires the sha2 crate; skipping in stub mode");
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_to_name() {
        assert_eq!(
            url_to_plugin_name("https://example.com/plugins/weather.wasm"),
            "weather"
        );
        assert_eq!(
            url_to_plugin_name("https://example.com/crypto-price.wasm"),
            "crypto-price"
        );
        assert_eq!(url_to_plugin_name("https://example.com/"), "");
    }

    #[test]
    fn default_dir_exists() {
        let dir = default_plugin_dir();
        // Should be something like /home/user/.ironclaw/plugins
        assert!(dir.to_string_lossy().contains(".ironclaw"));
        assert!(dir.to_string_lossy().contains("plugins"));
    }

    #[test]
    fn list_installed_empty_dir() {
        let tmp = std::env::temp_dir().join("ironclaw-test-empty-plugins");
        let _ = std::fs::create_dir_all(&tmp);
        let plugins = list_installed(&tmp);
        assert!(plugins.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
