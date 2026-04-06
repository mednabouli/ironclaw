//! Wasmtime Component Model runtime for executing WASM plugins.
//!
//! This module provides the actual execution engine that loads `.wasm`
//! Component Model binaries, links host imports (HTTP, filesystem, env, log),
//! and invokes the plugin's `tool.invoke` export.
//!
//! Requires the `runtime` feature flag (which pulls in `wasmtime`).

use std::path::Path;

use anyhow::Context;
use tracing::{debug, error, info, trace, warn};
use wasmtime::component::{types, Component, Linker, Val};
use wasmtime::{Config, Engine, Store};

use crate::capability::{Capability, CapabilityGrant};

/// Default fuel limit per plugin invocation (~1 million instructions).
const DEFAULT_FUEL: u64 = 1_000_000;

/// Maximum memory a plugin may allocate (64 MiB).
const MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;

/// Shared WASM runtime engine.
///
/// Creating an `Engine` is expensive (compiles internal codegen infrastructure).
/// Share a single `WasmRuntime` across all plugins.
pub struct WasmRuntime {
    engine: Engine,
}

impl WasmRuntime {
    /// Create a new WASM runtime with sensible defaults.
    ///
    /// Enables fuel metering and the Component Model.
    pub fn new() -> anyhow::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        // Memory limits are enforced per-store, not in the engine config

        let engine = Engine::new(&config).context("Failed to create wasmtime engine")?;

        Ok(Self { engine })
    }

    /// Compile a `.wasm` component from disk.
    pub fn load_component(&self, path: &Path) -> anyhow::Result<CompiledPlugin> {
        info!(path = %path.display(), "Compiling WASM plugin");
        let bytes = std::fs::read(path)
            .with_context(|| format!("Cannot read WASM file: {}", path.display()))?;

        let component = Component::new(&self.engine, &bytes)
            .with_context(|| format!("Failed to compile WASM component: {}", path.display()))?;

        // Read schema by instantiating once with a temporary store
        let schema = read_plugin_schema(&self.engine, &component)?;

        Ok(CompiledPlugin {
            engine: self.engine.clone(),
            component,
            schema,
        })
    }

    /// Return a reference to the underlying engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

/// A compiled WASM plugin ready to be invoked.
pub struct CompiledPlugin {
    engine: Engine,
    component: Component,
    schema: PluginSchema,
}

/// Schema extracted from the plugin's `get-schema` export.
#[derive(Debug, Clone)]
pub struct PluginSchema {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// JSON Schema string for parameters.
    pub parameters_json: String,
}

/// Host state carried in the wasmtime `Store`.
struct HostState {
    capabilities: CapabilityGrant,
    #[allow(dead_code)]
    fuel: u64,
    limiter: StoreLimiter,
}

impl CompiledPlugin {
    /// Return the schema extracted from the plugin.
    pub fn schema(&self) -> &PluginSchema {
        &self.schema
    }

    /// Invoke the plugin's `tool.invoke` export with JSON parameters.
    ///
    /// Enforces capability checks and fuel metering for sandboxing.
    pub fn invoke(
        &self,
        params_json: &str,
        capabilities: &CapabilityGrant,
    ) -> anyhow::Result<String> {
        let host_state = HostState {
            capabilities: capabilities.clone(),
            fuel: DEFAULT_FUEL,
            limiter: StoreLimiter {
                memory_remaining: MAX_MEMORY_BYTES,
            },
        };

        let mut store = Store::new(&self.engine, host_state);
        store.set_fuel(DEFAULT_FUEL)?;

        // Set memory limits via store limiter
        store.limiter(|state| &mut state.limiter);

        let mut linker = Linker::<HostState>::new(&self.engine);
        link_host_imports(&mut linker)?;

        let instance = linker
            .instantiate(&mut store, &self.component)
            .context("Failed to instantiate WASM component")?;

        // Look up the tool.invoke export
        // In the Component Model, interface exports are namespaced:
        //   ironclaw:plugin/tool.invoke
        let invoke_func = instance
            .get_func(&mut store, "ironclaw:plugin/tool@0.1.0#invoke")
            .or_else(|| instance.get_func(&mut store, "invoke"))
            .context("Plugin does not export 'invoke' function")?;

        // Call: invoke(params-json: string) -> result<string, string>
        let mut results = [Val::Bool(false)]; // placeholder, will be overwritten
        invoke_func.call(&mut store, &[Val::String(params_json.into())], &mut results)?;
        invoke_func.post_return(&mut store)?;

        // Parse the result<string, string> return value
        match &results[0] {
            Val::Result(result_val) => match result_val {
                Ok(Some(boxed)) => {
                    if let Val::String(s) = boxed.as_ref() {
                        Ok(s.clone())
                    } else {
                        Ok(String::new())
                    }
                }
                Ok(None) => Ok(String::new()),
                Err(Some(boxed)) => {
                    if let Val::String(e) = boxed.as_ref() {
                        Err(anyhow::anyhow!("Plugin returned error: {e}"))
                    } else {
                        Err(anyhow::anyhow!("Plugin returned non-string error"))
                    }
                }
                Err(None) => Err(anyhow::anyhow!("Plugin returned unspecified error")),
            },
            Val::String(s) => Ok(s.clone()),
            other => Err(anyhow::anyhow!(
                "Plugin invoke returned unexpected type: {other:?}"
            )),
        }
    }
}

/// Read the plugin schema by calling `get-schema` once.
fn read_plugin_schema(engine: &Engine, component: &Component) -> anyhow::Result<PluginSchema> {
    let mut store = Store::new(
        engine,
        HostState {
            capabilities: CapabilityGrant::default(),
            fuel: DEFAULT_FUEL,
            limiter: StoreLimiter {
                memory_remaining: MAX_MEMORY_BYTES,
            },
        },
    );
    store.set_fuel(DEFAULT_FUEL)?;

    let mut linker = Linker::<HostState>::new(engine);
    link_host_imports(&mut linker)?;

    let instance = linker
        .instantiate(&mut store, component)
        .context("Failed to instantiate plugin for schema extraction")?;

    let get_schema = instance
        .get_func(&mut store, "ironclaw:plugin/tool@0.1.0#get-schema")
        .or_else(|| instance.get_func(&mut store, "get-schema"))
        .context("Plugin does not export 'get-schema' function")?;

    // get-schema() -> tool-schema { name, description, parameters-json }
    let mut results = [
        Val::String(String::new()),
        Val::String(String::new()),
        Val::String(String::new()),
    ];
    get_schema.call(&mut store, &[], &mut results)?;
    get_schema.post_return(&mut store)?;

    // The component model returns a record as multiple flat values
    let name = match &results[0] {
        Val::String(s) => s.to_string(),
        Val::Record(fields) => extract_record_field(fields, "name")?,
        other => anyhow::bail!("Unexpected get-schema result[0]: {other:?}"),
    };

    let description = if results.len() > 1 {
        match &results[1] {
            Val::String(s) => s.to_string(),
            _ => String::new(),
        }
    } else {
        String::new()
    };

    let parameters_json = if results.len() > 2 {
        match &results[2] {
            Val::String(s) => s.to_string(),
            _ => "{}".to_string(),
        }
    } else {
        "{}".to_string()
    };

    Ok(PluginSchema {
        name,
        description,
        parameters_json,
    })
}

/// Extract a string field from a component model record.
fn extract_record_field(fields: &[(String, Val)], name: &str) -> anyhow::Result<String> {
    for (k, v) in fields {
        if k == name {
            if let Val::String(s) = v {
                return Ok(s.to_string());
            }
        }
    }
    anyhow::bail!("Record field '{name}' not found or not a string")
}

/// Link all host imports into the component linker.
fn link_host_imports(linker: &mut Linker<HostState>) -> anyhow::Result<()> {
    let mut host_instance = linker.instance("ironclaw:plugin/host@0.1.0")?;

    // ── http-fetch ──────────────────────────────────────────────────────
    host_instance.func_new(
        "http-fetch",
        |caller: wasmtime::StoreContextMut<'_, HostState>,
         _ty: types::ComponentFunc,
         args: &[Val],
         results: &mut [Val]| {
            // args[0] is http-request record: { method, url, headers, body }
            let (method, url, headers, body) = parse_http_request(args)?;

            let caps = &caller.data().capabilities;
            if !caps.has(&Capability::Http) {
                results[0] = make_result_err("HTTP capability not granted to this plugin");
                return Ok(());
            }
            if !caps.check_url(&url) {
                results[0] = make_result_err(&format!("URL not in allowlist: {url}"));
                return Ok(());
            }

            debug!(method = %method, url = %url, "Plugin HTTP request");

            // Execute HTTP request synchronously using tokio Handle
            let result = tokio::runtime::Handle::try_current()
                .map_err(|e| anyhow::anyhow!("No tokio runtime: {e}"))
                .and_then(|handle| {
                    handle.block_on(async {
                        let client = reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(30))
                            .build()?;

                        let mut req = match method.as_str() {
                            "GET" => client.get(&url),
                            "POST" => client.post(&url),
                            "PUT" => client.put(&url),
                            "DELETE" => client.delete(&url),
                            "PATCH" => client.patch(&url),
                            "HEAD" => client.head(&url),
                            _ => anyhow::bail!("Unsupported HTTP method: {method}"),
                        };

                        for (k, v) in &headers {
                            req = req.header(k.as_str(), v.as_str());
                        }

                        if let Some(ref b) = body {
                            req = req.body(b.clone());
                        }

                        let resp = req.send().await?;
                        let status = resp.status().as_u16();
                        let resp_headers: Vec<(String, String)> = resp
                            .headers()
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                            .collect();
                        let resp_body = resp.text().await?;

                        Ok((status, resp_headers, resp_body))
                    })
                });

            match result {
                Ok((status, resp_headers, resp_body)) => {
                    results[0] = make_http_response_ok(status, &resp_headers, &resp_body);
                }
                Err(e) => {
                    results[0] = make_result_err(&format!("HTTP request failed: {e}"));
                }
            }
            Ok(())
        },
    )?;

    // ── fs-read ─────────────────────────────────────────────────────────
    host_instance.func_new(
        "fs-read",
        |caller: wasmtime::StoreContextMut<'_, HostState>,
         _ty: types::ComponentFunc,
         args: &[Val],
         results: &mut [Val]| {
            let path = val_as_str(&args[0])?;
            let caps = &caller.data().capabilities;

            if !caps.has(&Capability::Filesystem) {
                results[0] = make_result_err("Filesystem capability not granted");
                return Ok(());
            }
            if !caps.check_path(&path) {
                results[0] = make_result_err(&format!("Path not in sandbox: {path}"));
                return Ok(());
            }

            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    results[0] = make_result_ok_string(&content);
                }
                Err(e) => {
                    results[0] = make_result_err(&format!("fs-read failed: {e}"));
                }
            }
            Ok(())
        },
    )?;

    // ── fs-write ────────────────────────────────────────────────────────
    host_instance.func_new(
        "fs-write",
        |caller: wasmtime::StoreContextMut<'_, HostState>,
         _ty: types::ComponentFunc,
         args: &[Val],
         results: &mut [Val]| {
            let path = val_as_str(&args[0])?;
            let content = val_as_str(&args[1])?;
            let caps = &caller.data().capabilities;

            if !caps.has(&Capability::Filesystem) {
                results[0] = make_result_err("Filesystem capability not granted");
                return Ok(());
            }
            if !caps.check_path(&path) {
                results[0] = make_result_err(&format!("Path not in sandbox: {path}"));
                return Ok(());
            }

            match std::fs::write(&path, &content) {
                Ok(()) => {
                    results[0] = make_result_ok_unit();
                }
                Err(e) => {
                    results[0] = make_result_err(&format!("fs-write failed: {e}"));
                }
            }
            Ok(())
        },
    )?;

    // ── fs-list ─────────────────────────────────────────────────────────
    host_instance.func_new(
        "fs-list",
        |caller: wasmtime::StoreContextMut<'_, HostState>,
         _ty: types::ComponentFunc,
         args: &[Val],
         results: &mut [Val]| {
            let path = val_as_str(&args[0])?;
            let caps = &caller.data().capabilities;

            if !caps.has(&Capability::Filesystem) {
                results[0] = make_result_err("Filesystem capability not granted");
                return Ok(());
            }
            if !caps.check_path(&path) {
                results[0] = make_result_err(&format!("Path not in sandbox: {path}"));
                return Ok(());
            }

            match std::fs::read_dir(&path) {
                Ok(entries) => {
                    let names: Vec<Val> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| Val::String(e.file_name().to_string_lossy().into_owned()))
                        .collect();
                    results[0] = Val::Result(Ok(Some(Box::new(Val::List(names)))));
                }
                Err(e) => {
                    results[0] = make_result_err(&format!("fs-list failed: {e}"));
                }
            }
            Ok(())
        },
    )?;

    // ── env-get ─────────────────────────────────────────────────────────
    host_instance.func_new(
        "env-get",
        |caller: wasmtime::StoreContextMut<'_, HostState>,
         _ty: types::ComponentFunc,
         args: &[Val],
         results: &mut [Val]| {
            let name = val_as_str(&args[0])?;
            let caps = &caller.data().capabilities;

            if !caps.has(&Capability::Env) || !caps.check_env_var(&name) {
                results[0] = Val::Option(None);
                return Ok(());
            }

            match std::env::var(&name) {
                Ok(val) => {
                    results[0] = Val::Option(Some(Box::new(Val::String(val))));
                }
                Err(_) => {
                    results[0] = Val::Option(None);
                }
            }
            Ok(())
        },
    )?;

    // ── log ─────────────────────────────────────────────────────────────
    host_instance.func_new(
        "log",
        |_caller: wasmtime::StoreContextMut<'_, HostState>,
         _ty: types::ComponentFunc,
         args: &[Val],
         _results: &mut [Val]| {
            let level = val_as_str(&args[0])?;
            let message = val_as_str(&args[1])?;

            match level.as_str() {
                "trace" => trace!(target: "wasm_plugin", "{message}"),
                "debug" => debug!(target: "wasm_plugin", "{message}"),
                "info" => info!(target: "wasm_plugin", "{message}"),
                "warn" => warn!(target: "wasm_plugin", "{message}"),
                "error" => error!(target: "wasm_plugin", "{message}"),
                _ => debug!(target: "wasm_plugin", level = %level, "{message}"),
            }
            Ok(())
        },
    )?;

    Ok(())
}

// ── Helper functions for Val construction ───────────────────────────────────

/// Extract a string from a `Val`.
fn val_as_str(val: &Val) -> anyhow::Result<String> {
    match val {
        Val::String(s) => Ok(s.to_string()),
        other => anyhow::bail!("Expected string, got: {other:?}"),
    }
}

/// Parsed HTTP request fields.
type HttpRequestParts = (String, String, Vec<(String, String)>, Option<String>);

/// Parse an HTTP request record from component model args.
fn parse_http_request(args: &[Val]) -> anyhow::Result<HttpRequestParts> {
    // The http-request record may come as a single Record val or flattened
    match &args[0] {
        Val::Record(fields) => {
            let method = extract_record_field(fields, "method")?;
            let url = extract_record_field(fields, "url")?;

            let mut headers = Vec::new();
            for (k, v) in fields {
                if k == "headers" {
                    if let Val::List(list) = v {
                        for item in list {
                            if let Val::Tuple(pair) = item {
                                if pair.len() == 2 {
                                    let hk = val_as_str(&pair[0])?;
                                    let hv = val_as_str(&pair[1])?;
                                    headers.push((hk, hv));
                                }
                            }
                        }
                    }
                }
            }

            let body = fields
                .iter()
                .find(|(k, _)| k == "body")
                .and_then(|(_, v)| match v {
                    Val::Option(Some(inner)) => {
                        if let Val::String(s) = inner.as_ref() {
                            Some(s.to_string())
                        } else {
                            None
                        }
                    }
                    _ => None,
                });

            Ok((method, url, headers, body))
        }
        _ => anyhow::bail!("Expected http-request record argument"),
    }
}

/// Construct a `result.ok(http-response)` value.
fn make_http_response_ok(status: u16, headers: &[(String, String)], body: &str) -> Val {
    let header_vals: Vec<Val> = headers
        .iter()
        .map(|(k, v)| Val::Tuple(vec![Val::String(k.clone()), Val::String(v.clone())]))
        .collect();

    let response = Val::Record(vec![
        ("status".to_string(), Val::U16(status)),
        ("headers".to_string(), Val::List(header_vals)),
        ("body".to_string(), Val::String(body.to_string())),
    ]);

    Val::Result(Ok(Some(Box::new(response))))
}

/// Construct a `result.err(string)` value.
fn make_result_err(msg: &str) -> Val {
    Val::Result(Err(Some(Box::new(Val::String(msg.to_string())))))
}

/// Construct a `result.ok(string)` value.
fn make_result_ok_string(s: &str) -> Val {
    Val::Result(Ok(Some(Box::new(Val::String(s.to_string())))))
}

/// Construct a `result.ok(_)` (unit ok) value.
fn make_result_ok_unit() -> Val {
    Val::Result(Ok(None))
}

/// Store-level resource limiter to cap plugin memory usage.
struct StoreLimiter {
    memory_remaining: usize,
}

impl wasmtime::ResourceLimiter for StoreLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        let delta = desired.saturating_sub(current);
        if delta > self.memory_remaining {
            warn!(
                current,
                desired,
                remaining = self.memory_remaining,
                "Plugin exceeded memory limit"
            );
            return Ok(false);
        }
        self.memory_remaining -= delta;
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_creates_successfully() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok());
    }

    #[test]
    fn load_nonexistent_file_fails() {
        let runtime = WasmRuntime::new().unwrap();
        let result = runtime.load_component(Path::new("/nonexistent/plugin.wasm"));
        assert!(result.is_err());
    }

    #[test]
    fn load_invalid_wasm_fails() {
        let tmp = std::env::temp_dir().join("ironclaw-invalid.wasm");
        std::fs::write(&tmp, b"not a wasm file").unwrap();
        let runtime = WasmRuntime::new().unwrap();
        let result = runtime.load_component(&tmp);
        assert!(result.is_err());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn make_result_err_is_err_variant() {
        let val = make_result_err("test error");
        match val {
            Val::Result(Err(Some(boxed))) => {
                if let Val::String(s) = *boxed {
                    assert_eq!(s.to_string(), "test error");
                } else {
                    panic!("Expected string inside Err");
                }
            }
            _ => panic!("Expected Result::Err"),
        }
    }

    #[test]
    fn make_result_ok_string_is_ok_variant() {
        let val = make_result_ok_string("hello");
        match val {
            Val::Result(Ok(Some(boxed))) => {
                if let Val::String(s) = *boxed {
                    assert_eq!(s.to_string(), "hello");
                } else {
                    panic!("Expected string inside Ok");
                }
            }
            _ => panic!("Expected Result::Ok"),
        }
    }

    #[test]
    fn val_as_str_extracts_string() {
        let val = Val::String("test".into());
        assert_eq!(val_as_str(&val).unwrap(), "test");
    }

    #[test]
    fn val_as_str_rejects_non_string() {
        let val = Val::Bool(true);
        assert!(val_as_str(&val).is_err());
    }
}
