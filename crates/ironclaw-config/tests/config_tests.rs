use ironclaw_config::IronClawConfig;

#[test]
fn default_config_is_valid() {
    let cfg = IronClawConfig::default();
    assert_eq!(cfg.providers.primary, "ollama");
    assert_eq!(cfg.channels.enabled, vec!["cli"]);
    assert_eq!(cfg.memory.backend, "memory");
    assert_eq!(cfg.memory.max_history, 50);
    assert_eq!(cfg.agent.max_tokens, 4096);
}

#[test]
fn toml_round_trip() {
    let cfg = IronClawConfig::default();
    let s = toml::to_string(&cfg).unwrap();
    let cfg2: IronClawConfig = toml::from_str(&s).unwrap();
    assert_eq!(cfg.providers.primary, cfg2.providers.primary);
}

#[test]
fn env_var_expansion_works() {
    std::env::set_var("IC_TEST_KEY", "sk-abc");
    let raw = r#"[providers.claude]\napi_key = "${IC_TEST_KEY}""#;
    let expanded = ironclaw_config::expand_env_vars(raw);
    assert!(expanded.contains("sk-abc"));
}

#[test]
fn missing_file_returns_error() {
    assert!(IronClawConfig::from_file("/no/such/file.toml").is_err());
}
