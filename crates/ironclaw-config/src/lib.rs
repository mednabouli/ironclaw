
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::collections::HashMap;

/// Master config struct — parsed from ironclaw.toml
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IronClawConfig {
    pub agent:     AgentConfig,
    pub providers: ProvidersConfig,
    pub channels:  ChannelsConfig,
    pub memory:    MemoryConfig,
    pub tools:     ToolsConfig,
    pub telemetry: TelemetryConfig,
}


impl IronClawConfig {
    pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow::anyhow!("Cannot read config: {e}"))?;
        let expanded = expand_env_vars(&raw);
        let cfg: Self = toml::from_str(&expanded)
            .map_err(|e| anyhow::anyhow!("Config parse error: {e}"))?;
        Ok(cfg)
    }

    pub fn from_default() -> Self { Self::default() }
}

/// Expand `${VAR_NAME}` placeholders in a config string using environment variables.
pub fn expand_env_vars(s: &str) -> String {
    let mut result = s.to_string();
    // Replace ${VAR_NAME} with env value or empty string
    let mut start = 0;
    while let Some(i) = result[start..].find("${") {
        let abs_i = start + i;
        if let Some(j) = result[abs_i..].find('}') {
            let var_name = &result[abs_i + 2..abs_i + j];
            let value    = std::env::var(var_name).unwrap_or_default();
            result.replace_range(abs_i..abs_i + j + 1, &value);
            start = abs_i + value.len();
        } else {
            break;
        }
    }
    result
}

// ── Sub-configs ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub name:          String,
    pub system_prompt: String,
    pub max_tokens:    u32,
    pub temperature:   f32,
}
impl Default for AgentConfig {
    fn default() -> Self {
        Self { name: "IronClaw".into(),
               system_prompt: "You are IronClaw, a helpful AI assistant.".into(),
               max_tokens: 4096, temperature: 0.7 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub primary:  String,
    pub fallback: Vec<String>,
    pub ollama:   OllamaConfig,
    pub claude:   AnthropicConfig,
    pub openai:   OpenAIConfig,
    pub groq:     GroqConfig,
    pub extra:    HashMap<String, ExtraProviderConfig>,
}
impl Default for ProvidersConfig {
    fn default() -> Self {
        Self { primary: "ollama".into(), fallback: vec![],
               ollama: Default::default(), claude: Default::default(),
               openai: Default::default(), groq: Default::default(),
               extra: HashMap::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model:    String,
}
impl Default for OllamaConfig {
    fn default() -> Self { Self { base_url: "http://localhost:11434".into(), model: "llama3.2".into() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub model:   String,
    pub base_url: String,
}
impl Default for AnthropicConfig {
    fn default() -> Self { Self { api_key: String::new(), model: "claude-3-5-sonnet-20241022".into(), base_url: "https://api.anthropic.com".into() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenAIConfig {
    pub api_key:  String,
    pub model:    String,
    pub base_url: String,
}
impl Default for OpenAIConfig {
    fn default() -> Self { Self { api_key: String::new(), model: "gpt-4o-mini".into(), base_url: "https://api.openai.com".into() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GroqConfig {
    pub api_key: String,
    pub model:   String,
}
impl Default for GroqConfig {
    fn default() -> Self { Self { api_key: String::new(), model: "llama-3.3-70b-versatile".into() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraProviderConfig {
    pub base_url: String,
    pub api_key:  String,
    pub model:    String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelsConfig {
    pub enabled:   Vec<String>,
    pub telegram:  TelegramConfig,
    pub rest:      RestConfig,
    pub discord:   DiscordConfig,
}
impl Default for ChannelsConfig {
    fn default() -> Self {
        Self { enabled: vec!["cli".into()], telegram: Default::default(),
               rest: Default::default(), discord: Default::default() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelegramConfig { pub token: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RestConfig { pub host: String, pub port: u16, pub auth_token: String }
impl Default for RestConfig { fn default() -> Self { Self { host: "127.0.0.1".into(), port: 8080, auth_token: String::new() } } }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscordConfig { pub token: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig { pub backend: String, pub max_history: usize, pub path: String }
impl Default for MemoryConfig { fn default() -> Self { Self { backend: "memory".into(), max_history: 50, path: "~/.ironclaw/memory.db".into() } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsConfig { pub enabled: Vec<String>, pub shell: ShellToolConfig }
impl Default for ToolsConfig { fn default() -> Self { Self { enabled: vec!["datetime".into()], shell: Default::default() } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellToolConfig { pub allowlist: Vec<String>, pub timeout_secs: u64 }
impl Default for ShellToolConfig { fn default() -> Self { Self { allowlist: vec!["ls".into(),"echo".into(),"cat".into()], timeout_secs: 30 } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TelemetryConfig { pub level: String, pub format: String }
impl Default for TelemetryConfig { fn default() -> Self { Self { level: "info".into(), format: "pretty".into() } } }
