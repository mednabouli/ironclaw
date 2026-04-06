//! WASM Plugin sandbox — Phase 6 (wasmtime Component Model)
//!
//! Provides the plugin system for IronClaw:
//! - WIT interface definition for the Component Model ([`wit/plugin.wit`])
//! - Capability-based sandboxing ([`capability`])
//! - Plugin manifest and registry ([`manifest`])
//! - Plugin installer ([`installer`])
//! - `WasmTool` — loads `.wasm` plugins as `Tool` implementations

pub mod capability;
pub mod installer;
pub mod manifest;

use std::path::PathBuf;

use async_trait::async_trait;
use ironclaw_core::{Tool, ToolSchema};
use serde_json::{json, Value};
use tracing::warn;

use crate::capability::CapabilityGrant;
use crate::manifest::PluginManifest;

/// A tool loaded from a .wasm plugin file.
///
/// Currently a type-safe stub that resolves the schema from the plugin manifest.
/// Full wasmtime Component Model execution will be wired in a future release.
pub struct WasmTool {
    /// Path to the `.wasm` file.
    #[allow(dead_code)]
    path: PathBuf,
    /// Tool schema (from plugin.json or inferred from filename).
    schema: ToolSchema,
    /// Capabilities granted to this plugin.
    #[allow(dead_code)]
    capabilities: CapabilityGrant,
}

impl WasmTool {
    /// Create a WASM tool from a file path and schema.
    pub fn from_file(path: impl Into<PathBuf>, schema: ToolSchema) -> Self {
        Self {
            path: path.into(),
            schema,
            capabilities: CapabilityGrant::default(),
        }
    }

    /// Create a WASM tool from a plugin manifest.
    pub fn from_manifest(wasm_path: impl Into<PathBuf>, manifest: &PluginManifest) -> Self {
        let schema = ToolSchema {
            name: manifest.name.clone(),
            description: manifest.description.clone(),
            parameters: manifest.parameters.clone(),
        };
        let capabilities = CapabilityGrant {
            capabilities: manifest.capabilities.clone(),
            allowed_urls: manifest.allowed_urls.clone(),
            allowed_env_vars: manifest.allowed_env_vars.clone(),
            sandbox_dir: None,
        };
        Self {
            path: wasm_path.into(),
            schema,
            capabilities,
        }
    }

    /// Set the capability grant for this plugin.
    pub fn with_capabilities(mut self, grant: CapabilityGrant) -> Self {
        self.capabilities = grant;
        self
    }

    /// Return the capabilities granted to this plugin.
    pub fn capabilities(&self) -> &CapabilityGrant {
        &self.capabilities
    }

    /// Return the path to the `.wasm` file.
    pub fn wasm_path(&self) -> &std::path::Path {
        &self.path
    }
}

#[async_trait]
impl Tool for WasmTool {
    fn name(&self) -> &str {
        &self.schema.name
    }
    fn description(&self) -> &str {
        &self.schema.description
    }
    fn schema(&self) -> ToolSchema {
        self.schema.clone()
    }

    async fn invoke(&self, _params: Value) -> anyhow::Result<Value> {
        warn!(
            plugin = %self.schema.name,
            path = %self.path.display(),
            "WASM plugin invoked but wasmtime runtime is not yet wired"
        );
        Ok(json!({
            "error": "WASM runtime not yet available — wasmtime Component Model pending",
            "plugin": self.schema.name,
        }))
    }
}

/// Scan a directory and load all `.wasm` files as `WasmTool` instances.
///
/// For each `.wasm` file, looks for a sibling `plugin.json` manifest.
/// If found, uses the manifest to populate the schema and capabilities.
/// Otherwise, creates a stub tool from the filename.
pub fn scan_plugins(dir: &std::path::Path) -> Vec<WasmTool> {
    if !dir.exists() {
        return vec![];
    }

    let mut tools = Vec::new();

    // First, check subdirectories (structured plugin layout: name/name.wasm + plugin.json)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("plugin.json");
                if manifest_path.exists() {
                    if let Ok(manifest) = PluginManifest::from_file(&manifest_path) {
                        let wasm_path = path.join(format!("{}.wasm", manifest.name));
                        if wasm_path.exists() {
                            tools.push(WasmTool::from_manifest(&wasm_path, &manifest));
                            continue;
                        }
                    }
                }
                // Check for any .wasm file in the subdirectory
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub_entry in sub_entries.filter_map(|e| e.ok()) {
                        if sub_entry
                            .path()
                            .extension()
                            .map(|x| x == "wasm")
                            .unwrap_or(false)
                        {
                            let tool = wasm_tool_from_path(sub_entry.path());
                            tools.push(tool);
                        }
                    }
                }
            } else if path.extension().map(|x| x == "wasm").unwrap_or(false) {
                // Flat layout: .wasm files directly in the plugin dir
                let manifest_path = path.with_extension("json");
                if manifest_path.exists() {
                    if let Ok(manifest) = PluginManifest::from_file(&manifest_path) {
                        tools.push(WasmTool::from_manifest(&path, &manifest));
                        continue;
                    }
                }
                tools.push(wasm_tool_from_path(path));
            }
        }
    }

    tools
}

/// Create a stub `WasmTool` from just a `.wasm` file path (no manifest).
fn wasm_tool_from_path(path: PathBuf) -> WasmTool {
    let name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let schema = ToolSchema {
        name: name.clone(),
        description: format!("WASM plugin: {name}"),
        parameters: json!({"type": "object", "properties": {}}),
    };
    WasmTool::from_file(path, schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn wasm_tool_from_file_has_correct_schema() {
        let schema = ToolSchema {
            name: "test-plugin".into(),
            description: "A test plugin".into(),
            parameters: json!({"type": "object"}),
        };
        let tool = WasmTool::from_file("/tmp/test.wasm", schema);
        assert_eq!(tool.name(), "test-plugin");
        assert_eq!(tool.description(), "A test plugin");
    }

    #[test]
    fn wasm_tool_from_manifest() {
        let manifest = PluginManifest {
            name: "weather".into(),
            version: "0.1.0".into(),
            description: "Get weather".into(),
            author: "test".into(),
            license: "MIT".into(),
            capabilities: vec![crate::capability::Capability::Http],
            allowed_urls: vec!["https://wttr.in/".into()],
            allowed_env_vars: vec![],
            parameters: json!({"type": "object", "properties": {"city": {"type": "string"}}}),
            download_url: None,
            sha256: None,
        };
        let tool = WasmTool::from_manifest("/tmp/weather.wasm", &manifest);
        assert_eq!(tool.name(), "weather");
        assert!(tool
            .capabilities()
            .has(&crate::capability::Capability::Http));
        assert!(!tool.capabilities().has(&crate::capability::Capability::Env));
    }

    #[test]
    fn scan_empty_dir() {
        let tmp = std::env::temp_dir().join("ironclaw-scan-empty");
        let _ = std::fs::create_dir_all(&tmp);
        let tools = scan_plugins(&tmp);
        assert!(tools.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn scan_nonexistent_dir() {
        let tools = scan_plugins(std::path::Path::new("/nonexistent/path"));
        assert!(tools.is_empty());
    }

    #[test]
    fn scan_flat_wasm_files() {
        let tmp = std::env::temp_dir().join("ironclaw-scan-flat");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Create a dummy .wasm file
        let wasm_path = tmp.join("myplugin.wasm");
        std::fs::File::create(&wasm_path)
            .unwrap()
            .write_all(b"\0asm")
            .unwrap();

        let tools = scan_plugins(&tmp);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "myplugin");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn scan_structured_plugin_with_manifest() {
        let tmp = std::env::temp_dir().join("ironclaw-scan-structured");
        let _ = std::fs::remove_dir_all(&tmp);
        let plugin_dir = tmp.join("weather");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        // Create plugin.json
        let manifest = serde_json::json!({
            "name": "weather",
            "version": "0.1.0",
            "description": "Weather plugin",
            "author": "test",
            "license": "MIT",
            "capabilities": ["http"],
            "parameters": {"type": "object"}
        });
        std::fs::write(
            plugin_dir.join("plugin.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        // Create weather.wasm
        std::fs::File::create(plugin_dir.join("weather.wasm"))
            .unwrap()
            .write_all(b"\0asm")
            .unwrap();

        let tools = scan_plugins(&tmp);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "weather");
        assert_eq!(tools[0].description(), "Weather plugin");
        assert!(tools[0]
            .capabilities()
            .has(&crate::capability::Capability::Http));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
