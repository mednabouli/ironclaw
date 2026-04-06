//! Property-based tests for ironclaw-config TOML parsing.

use ironclaw_config::IronClawConfig;
use proptest::prelude::*;

proptest! {
    /// Arbitrary TOML strings must never cause a panic; they either
    /// parse successfully or return an error.
    #[test]
    fn config_parse_never_panics(data in ".*") {
        let _ = toml::from_str::<IronClawConfig>(&data);
    }

    /// Valid TOML with an [agent] table always parses without panic.
    #[test]
    fn config_agent_section_roundtrips(
        name in "[a-zA-Z0-9_]{1,30}",
        max_tok in 1..65_536u32,
        temp in 0.0f32..2.0,
    ) {
        let toml_str = format!(
            r#"
[agent]
name = "{name}"
system_prompt = "You are a test bot."
max_tokens = {max_tok}
temperature = {temp}
"#
        );
        let cfg: IronClawConfig = toml::from_str(&toml_str).unwrap();
        prop_assert_eq!(&cfg.agent.name, &name);
        prop_assert_eq!(cfg.agent.max_tokens, max_tok);
    }

    /// Config default round-trips through TOML serialize/deserialize.
    #[test]
    fn config_default_serde_roundtrips(_dummy in 0..1u32) {
        let cfg = IronClawConfig::default();
        let toml_str = toml::to_string(&cfg).unwrap();
        let cfg2: IronClawConfig = toml::from_str(&toml_str).unwrap();
        prop_assert_eq!(&cfg.agent.name, &cfg2.agent.name);
        prop_assert_eq!(cfg.agent.max_tokens, cfg2.agent.max_tokens);
        prop_assert_eq!(&cfg.providers.primary, &cfg2.providers.primary);
    }

    /// Arbitrary provider name and fallback list are handled correctly.
    #[test]
    fn config_providers_section(
        primary in "[a-z]{1,20}",
        fallback_count in 0..5usize,
    ) {
        let fallbacks: Vec<String> = (0..fallback_count)
            .map(|i| format!("provider_{i}"))
            .collect();
        let fb_toml = fallbacks
            .iter()
            .map(|f| format!("\"{f}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let toml_str = format!(
            r#"
[providers]
primary = "{primary}"
fallback = [{fb_toml}]
"#
        );
        let cfg: IronClawConfig = toml::from_str(&toml_str).unwrap();
        prop_assert_eq!(&cfg.providers.primary, &primary);
        prop_assert_eq!(cfg.providers.fallback.len(), fallback_count);
    }

    /// Memory config with arbitrary capacity round-trips.
    #[test]
    fn config_memory_capacity(cap in 1..100_000usize) {
        let toml_str = format!(
            r#"
[memory]
max_history = {cap}
"#
        );
        let cfg: IronClawConfig = toml::from_str(&toml_str).unwrap();
        prop_assert_eq!(cfg.memory.max_history, cap);
    }
}
