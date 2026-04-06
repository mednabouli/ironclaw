#![no_main]
//! Fuzz target: SSE line parsing.
//!
//! Feeds arbitrary bytes as SSE data lines, simulating the format:
//!   `data: {...}\n\n`
//!   `data: [DONE]\n\n`
//!
//! Checks that parsing never panics on malformed input.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Simulate SSE line-by-line parsing
        for line in s.lines() {
            // Strip "data: " prefix like a real SSE parser
            let payload = line.strip_prefix("data: ").unwrap_or(line);

            // Skip [DONE] marker
            if payload.trim() == "[DONE]" {
                continue;
            }

            // Skip empty lines and comments
            if payload.is_empty() || payload.starts_with(':') {
                continue;
            }

            // Try to parse as JSON (this is what providers do with SSE data)
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) {
                // Extract fields that providers typically access
                let _ = value.get("choices");
                let _ = value.get("delta");
                let _ = value.get("content");
                let _ = value.get("type");
                let _ = value.get("error");

                // Try to extract as StreamChunk
                let _ = serde_json::from_value::<ironclaw_core::StreamChunk>(value);
            }
        }
    }
});
