use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};

pub mod watcher;
pub use watcher::ConfigWatcher;

/// Master config struct — parsed from ironclaw.toml
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IronClawConfig {
    pub agent: AgentConfig,
    pub providers: ProvidersConfig,
    pub channels: ChannelsConfig,
    pub memory: MemoryConfig,
    pub tools: ToolsConfig,
    pub telemetry: TelemetryConfig,
}

impl IronClawConfig {
    pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow::anyhow!("Cannot read config: {e}"))?;
        let expanded = expand_env_vars(&raw);
        let cfg: Self =
            toml::from_str(&expanded).map_err(|e| anyhow::anyhow!("Config parse error: {e}"))?;
        Ok(cfg)
    }

    pub fn from_default() -> Self {
        Self::default()
    }
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
            let value = std::env::var(var_name).unwrap_or_default();
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
    pub name: String,
    pub system_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "IronClaw".into(),
            system_prompt: "You are IronClaw, a helpful AI assistant.".into(),
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub primary: String,
    pub fallback: Vec<String>,
    pub retry: ProviderRetryConfig,
    pub ollama: OllamaConfig,
    pub claude: AnthropicConfig,
    pub openai: OpenAIConfig,
    pub groq: GroqConfig,
    pub openrouter: OpenRouterConfig,
    pub mistral: MistralConfig,
    pub together: TogetherConfig,
    pub cohere: CohereConfig,
    pub extra: HashMap<String, ExtraProviderConfig>,
}
impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            primary: "ollama".into(),
            fallback: vec![],
            retry: Default::default(),
            ollama: Default::default(),
            claude: Default::default(),
            openai: Default::default(),
            groq: Default::default(),
            openrouter: Default::default(),
            mistral: Default::default(),
            together: Default::default(),
            cohere: Default::default(),
            extra: HashMap::new(),
        }
    }
}

/// Retry configuration for provider API calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderRetryConfig {
    /// Whether retry is enabled.
    pub enabled: bool,
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Base delay between retries in milliseconds.
    pub base_delay_ms: u64,
    /// Maximum delay cap in milliseconds.
    pub max_delay_ms: u64,
}
impl Default for ProviderRetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 3,
            base_delay_ms: 500,
            max_delay_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model: String,
}
impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".into(),
            model: "llama3.2".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}
impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "claude-3-5-sonnet-20241022".into(),
            base_url: "https://api.anthropic.com".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}
impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "gpt-4o-mini".into(),
            base_url: "https://api.openai.com".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GroqConfig {
    pub api_key: String,
    pub model: String,
}
impl Default for GroqConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "llama-3.3-70b-versatile".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenRouterConfig {
    pub api_key: String,
    pub model: String,
}
impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "openai/gpt-4o".into(),
        }
    }
}

/// Mistral AI provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MistralConfig {
    /// API key for Mistral AI.
    pub api_key: String,
    /// Model name (e.g. "mistral-large-latest").
    pub model: String,
}
impl Default for MistralConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "mistral-large-latest".into(),
        }
    }
}

/// Together AI provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TogetherConfig {
    /// API key for Together AI.
    pub api_key: String,
    /// Model name (e.g. "meta-llama/Llama-3-70b").
    pub model: String,
}
impl Default for TogetherConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "meta-llama/Llama-3-70b".into(),
        }
    }
}

/// Cohere provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CohereConfig {
    /// API key for Cohere.
    pub api_key: String,
    /// Model name (e.g. "command-r-plus").
    pub model: String,
}
impl Default for CohereConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "command-r-plus".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelsConfig {
    pub enabled: Vec<String>,
    pub rate_limit: RateLimitConfigToml,
    pub telegram: TelegramConfig,
    pub rest: RestConfig,
    pub discord: DiscordConfig,
    pub slack: SlackConfig,
    pub websocket: WebSocketConfig,
    pub webhook: WebhookConfig,
    pub matrix: MatrixConfig,
}
impl Default for ChannelsConfig {
    fn default() -> Self {
        Self {
            enabled: vec!["cli".into()],
            rate_limit: Default::default(),
            telegram: Default::default(),
            rest: Default::default(),
            discord: Default::default(),
            slack: Default::default(),
            websocket: Default::default(),
            webhook: Default::default(),
            matrix: Default::default(),
        }
    }
}

/// Per-user rate-limit configuration (token bucket).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfigToml {
    /// Whether rate limiting is enabled.
    pub enabled: bool,
    /// Maximum burst size (tokens in a full bucket).
    pub capacity: u32,
    /// Tokens added per refill interval.
    pub refill_tokens: u32,
    /// Refill interval in seconds.
    pub refill_interval_secs: u64,
}
impl Default for RateLimitConfigToml {
    fn default() -> Self {
        Self {
            enabled: false,
            capacity: 20,
            refill_tokens: 1,
            refill_interval_secs: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelegramConfig {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RestConfig {
    pub host: String,
    pub port: u16,
    pub auth_token: String,
}
impl Default for RestConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8080,
            auth_token: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscordConfig {
    pub token: String,
}

/// Slack Events API channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SlackConfig {
    pub bot_token: String,
    pub signing_secret: String,
    pub host: String,
    pub port: u16,
}
impl Default for SlackConfig {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            signing_secret: String::new(),
            host: "127.0.0.1".into(),
            port: 3000,
        }
    }
}

/// WebSocket channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebSocketConfig {
    pub host: String,
    pub port: u16,
    pub auth_token: String,
}
impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8081,
            auth_token: String::new(),
        }
    }
}

/// Generic inbound webhook channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebhookConfig {
    pub host: String,
    pub port: u16,
    pub path: String,
    pub auth_token: String,
}
impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 9000,
            path: "/webhook".into(),
            auth_token: String::new(),
        }
    }
}

/// Matrix channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatrixConfig {
    pub homeserver_url: String,
    pub access_token: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub backend: String,
    pub max_history: usize,
    pub path: String,
    pub redis: RedisConfig,
    /// Dimensions for stored embedding vectors (must match your embedding model).
    pub embedding_dimensions: usize,
}
impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            backend: "memory".into(),
            max_history: 50,
            path: "~/.ironclaw/memory.db".into(),
            redis: Default::default(),
            embedding_dimensions: 384,
        }
    }
}

/// Redis backend configuration for distributed memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RedisConfig {
    /// Redis connection URL (e.g. `redis://127.0.0.1:6379`).
    pub url: String,
    /// Key prefix for all IronClaw keys, to avoid collisions.
    pub key_prefix: String,
    /// Maximum number of messages per session before trimming.
    pub max_history: usize,
}
impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".into(),
            key_prefix: "ironclaw:".into(),
            max_history: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsConfig {
    pub enabled: Vec<String>,
    pub shell: ShellToolConfig,
    /// Directories that FileReadTool and FileWriteTool are allowed to access.
    pub file_allowed_dirs: Vec<String>,
}
impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enabled: vec!["datetime".into()],
            shell: Default::default(),
            file_allowed_dirs: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellToolConfig {
    pub allowlist: Vec<String>,
    pub timeout_secs: u64,
}
impl Default for ShellToolConfig {
    fn default() -> Self {
        Self {
            allowlist: vec!["ls".into(), "echo".into(), "cat".into()],
            timeout_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TelemetryConfig {
    pub level: String,
    pub format: String,
}
impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: "pretty".into(),
        }
    }
}
