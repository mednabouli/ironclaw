#![no_main]
//! Fuzz target: Message round-trip serialization.
//!
//! Feeds arbitrary bytes as JSON to `serde_json::from_slice::<Message>()`,
//! then re-serializes if successful and checks for panics.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Try to parse arbitrary bytes as a Message
    if let Ok(msg) = serde_json::from_slice::<ironclaw_core::Message>(data) {
        // Re-serialize — must not panic
        let _ = serde_json::to_string(&msg);
        let _ = serde_json::to_vec(&msg);
    }

    // Also fuzz CompletionRequest
    if let Ok(req) = serde_json::from_slice::<ironclaw_core::CompletionRequest>(data) {
        let _ = serde_json::to_string(&req);
    }

    // Also fuzz StreamChunk
    if let Ok(chunk) = serde_json::from_slice::<ironclaw_core::StreamChunk>(data) {
        let _ = serde_json::to_string(&chunk);
    }

    // Also fuzz InboundMessage
    if let Ok(msg) = serde_json::from_slice::<ironclaw_core::InboundMessage>(data) {
        let _ = serde_json::to_string(&msg);
    }
});
