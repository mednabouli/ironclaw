#![no_main]
//! Fuzz target: Tool parameter JSON handling.
//!
//! Feeds arbitrary JSON to tool schema validation and
//! ToolSchema construction, checking for panics.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) {
        // Try constructing a ToolSchema with arbitrary parameters
        let _ = ironclaw_core::ToolSchema::new("fuzz_tool", "fuzz description", value.clone());

        // Try parsing as ToolCall
        let _ = serde_json::from_value::<ironclaw_core::ToolCall>(value.clone());

        // Try parsing as ToolResult
        let _ = serde_json::from_value::<ironclaw_core::ToolResult>(value);
    }
});
