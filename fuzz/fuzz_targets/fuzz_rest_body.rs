#![no_main]
//! Fuzz the REST API /chat request body JSON deserialization.
//!
//! Mirrors the `ChatRequest` struct from ironclaw-channels::rest
//! to ensure arbitrary JSON payloads never cause panics during parsing.

use libfuzzer_sys::fuzz_target;

/// Mirrors the REST channel's ChatRequest structure.
#[derive(serde::Deserialize)]
struct ChatRequest {
    session_id: Option<String>,
    message: String,
}

/// Mirrors the REST channel's StreamChatRequest structure.
#[derive(serde::Deserialize)]
struct StreamChatRequest {
    session_id: Option<String>,
    message: String,
}

fuzz_target!(|data: &[u8]| {
    // Attempt to parse as ChatRequest — must never panic
    let _ = serde_json::from_slice::<ChatRequest>(data);

    // Attempt to parse as StreamChatRequest
    let _ = serde_json::from_slice::<StreamChatRequest>(data);

    // Also try as a generic serde_json::Value (the fallback path)
    let _ = serde_json::from_slice::<serde_json::Value>(data);
});
