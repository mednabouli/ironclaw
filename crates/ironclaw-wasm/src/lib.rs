//! WASM Plugin sandbox — Phase 6 (wasmtime Component Model)
//! This is a type-safe stub. Implement with `wasmtime` in Phase 6.

use std::path::PathBuf;

use async_trait::async_trait;
use ironclaw_core::{Tool, ToolSchema};
use serde_json::{json, Value};
use tracing::warn;

/// A tool loaded from a .wasm plugin file.
/// Phase 6 will implement real wasmtime sandboxing.
pub struct WasmTool {
    #[allow(dead_code)] // will be used by wasmtime in Phase 6
    path: PathBuf,
    schema: ToolSchema,
}

impl WasmTool {
    pub fn from_file(path: impl Into<PathBuf>, schema: ToolSchema) -> Self {
        Self {
            path: path.into(),
            schema,
        }
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
            "WASM plugin '{}' called but wasmtime is not yet wired (Phase 6)",
            self.schema.name
        );
        Ok(json!({ "error": "WASM plugins available in Phase 6" }))
    }
}

/// Scan a directory and load all .wasm files as WasmTool stubs.
pub fn scan_plugins(dir: &std::path::Path) -> Vec<WasmTool> {
    if !dir.exists() {
        return vec![];
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "wasm").unwrap_or(false))
        .map(|e| {
            let name = e
                .path()
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let schema = ToolSchema {
                name: name.clone(),
                description: format!("WASM plugin: {name}"),
                parameters: json!({"type":"object","properties":{}}),
            };
            WasmTool::from_file(e.path(), schema)
        })
        .collect()
}
