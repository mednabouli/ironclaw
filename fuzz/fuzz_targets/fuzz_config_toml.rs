#![no_main]
//! Fuzz target: TOML config parsing.
//!
//! Feeds arbitrary bytes as TOML to the config parser,
//! checking for panics in deserialization.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Try to parse as IronClawConfig
        let _ = toml::from_str::<ironclaw_config::IronClawConfig>(s);
    }
});
