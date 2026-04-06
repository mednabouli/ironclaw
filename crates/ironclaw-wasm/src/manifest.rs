//! Plugin registry manifest — JSON index of available and installed plugins.
//!
//! The registry manifest is a JSON file that describes available plugins:
//! - Remote registries serve `registry.json` listing downloadable plugins
//! - Locally installed plugins have a `plugin.json` manifest next to the `.wasm` file

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::capability::Capability;

/// A single plugin entry in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin name (matches the tool name).
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// Human-readable description shown to the LLM.
    pub description: String,
    /// Author or maintainer.
    pub author: String,
    /// License identifier (e.g. "MIT", "Apache-2.0").
    pub license: String,
    /// Capabilities this plugin requires.
    pub capabilities: Vec<Capability>,
    /// URL allowlist for the HTTP capability (empty = unrestricted if http granted).
    #[serde(default)]
    pub allowed_urls: Vec<String>,
    /// Environment variable names this plugin may read.
    #[serde(default)]
    pub allowed_env_vars: Vec<String>,
    /// JSON Schema for the tool's parameters (inline).
    pub parameters: serde_json::Value,
    /// Download URL for the `.wasm` file (used by remote registries).
    #[serde(default)]
    pub download_url: Option<String>,
    /// SHA-256 hex digest of the `.wasm` file for integrity verification.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// The remote registry index — a collection of available plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRegistry {
    /// Registry format version.
    pub version: String,
    /// Base URL for relative download URLs.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Available plugins.
    pub plugins: Vec<PluginManifest>,
}

impl PluginRegistry {
    /// Load a registry from a JSON file.
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Cannot read registry file: {e}"))?;
        let registry: Self = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Invalid registry JSON: {e}"))?;
        Ok(registry)
    }

    /// Parse a registry from a JSON string.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let registry: Self = serde_json::from_str(json)
            .map_err(|e| anyhow::anyhow!("Invalid registry JSON: {e}"))?;
        Ok(registry)
    }

    /// Find a plugin by name.
    pub fn find(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.iter().find(|p| p.name == name)
    }
}

impl PluginManifest {
    /// Load a manifest from a JSON file (typically `plugin.json` next to the `.wasm`).
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Cannot read plugin manifest: {e}"))?;
        let manifest: Self = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Invalid plugin manifest: {e}"))?;
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> PluginManifest {
        PluginManifest {
            name: "weather".into(),
            version: "0.1.0".into(),
            description: "Get current weather for a city using wttr.in".into(),
            author: "IronClaw Contributors".into(),
            license: "MIT".into(),
            capabilities: vec![Capability::Http],
            allowed_urls: vec!["https://wttr.in/".into()],
            allowed_env_vars: vec![],
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "city": {
                        "type": "string",
                        "description": "City name (e.g. London, Tokyo)"
                    }
                },
                "required": ["city"]
            }),
            download_url: Some("https://plugins.ironclaw.dev/weather/0.1.0/weather.wasm".into()),
            sha256: Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into()),
        }
    }

    #[test]
    fn manifest_roundtrip() {
        let manifest = sample_manifest();
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let parsed: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "weather");
        assert_eq!(parsed.capabilities, vec![Capability::Http]);
    }

    #[test]
    fn registry_roundtrip() {
        let registry = PluginRegistry {
            version: "1".into(),
            base_url: Some("https://plugins.ironclaw.dev".into()),
            plugins: vec![sample_manifest()],
        };
        let json = serde_json::to_string_pretty(&registry).unwrap();
        let parsed = PluginRegistry::from_json(&json).unwrap();
        assert_eq!(parsed.plugins.len(), 1);
        assert_eq!(parsed.find("weather").unwrap().name, "weather");
        assert!(parsed.find("nonexistent").is_none());
    }

    #[test]
    fn manifest_deserializes_with_defaults() {
        let json = r#"{
            "name": "test",
            "version": "0.1.0",
            "description": "A test plugin",
            "author": "test",
            "license": "MIT",
            "capabilities": [],
            "parameters": {"type": "object"}
        }"#;
        let m: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(m.allowed_urls.is_empty());
        assert!(m.allowed_env_vars.is_empty());
        assert!(m.download_url.is_none());
        assert!(m.sha256.is_none());
    }
}
