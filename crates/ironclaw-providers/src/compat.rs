use ironclaw_core::{Provider, ProviderError};

use crate::openai::OpenAIProvider;

/// Generic OpenAI-compatible provider for any endpoint (DeepSeek, LM Studio,
/// Together AI, Fireworks, vLLM, etc.).
///
/// Instantiated from `[providers.extra.<name>]` config entries. The provider
/// name is taken from the config key, so multiple instances can coexist
/// (e.g. "deepseek", "lmstudio", "together").
///
/// API key is never included in `Debug` output.
pub struct CompatProvider {
    inner: OpenAIProvider,
    /// Leaked &'static str from the config key — lives for program lifetime.
    provider_name: &'static str,
}

impl std::fmt::Debug for CompatProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompatProvider")
            .field("name", &self.provider_name)
            .field("inner", &self.inner)
            .finish()
    }
}

impl CompatProvider {
    /// Create a new OpenAI-compatible provider with the given name, API key,
    /// model, and base URL.
    ///
    /// The `name` is leaked to a `&'static str` since providers are created
    /// once at startup and live for the program lifetime.
    pub fn new(
        name: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        let name_string = name.into();
        let provider_name: &'static str = Box::leak(name_string.into_boxed_str());
        Self {
            inner: OpenAIProvider::new(api_key, model, base_url),
            provider_name,
        }
    }

    /// Create a new OpenAI-compatible provider with a shared HTTP client.
    pub fn with_client(
        client: reqwest::Client,
        name: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        let name_string = name.into();
        let provider_name: &'static str = Box::leak(name_string.into_boxed_str());
        Self {
            inner: OpenAIProvider::with_client(client, api_key, model, base_url),
            provider_name,
        }
    }
}

#[async_trait::async_trait]
impl Provider for CompatProvider {
    fn name(&self) -> &'static str {
        self.provider_name
    }

    fn supports_vision(&self) -> bool {
        false // unknown — conservative default for generic endpoints
    }

    async fn complete(
        &self,
        req: ironclaw_core::CompletionRequest,
    ) -> Result<ironclaw_core::CompletionResponse, ProviderError> {
        self.inner.complete(req).await
    }

    async fn stream(
        &self,
        req: ironclaw_core::CompletionRequest,
    ) -> Result<ironclaw_core::BoxStream<ironclaw_core::StreamChunk>, ProviderError> {
        self.inner.stream(req).await
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        self.inner.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_api_key() {
        let p = CompatProvider::new(
            "deepseek",
            "sk-secret",
            "deepseek-chat",
            "https://api.deepseek.com",
        );
        let debug_str = format!("{:?}", p);
        assert!(
            !debug_str.contains("sk-secret"),
            "api_key must not appear in Debug output"
        );
        assert!(
            debug_str.contains("[REDACTED]"),
            "Debug output must contain [REDACTED]"
        );
    }

    #[test]
    fn provider_name_matches_config_key() {
        let p = CompatProvider::new("lmstudio", "key", "model", "http://localhost:1234");
        assert_eq!(p.name(), "lmstudio");
    }

    #[test]
    fn provider_name_deepseek() {
        let p = CompatProvider::new(
            "deepseek",
            "key",
            "deepseek-chat",
            "https://api.deepseek.com",
        );
        assert_eq!(p.name(), "deepseek");
    }

    #[test]
    fn does_not_support_vision_by_default() {
        let p = CompatProvider::new("test", "key", "model", "http://localhost");
        assert!(!p.supports_vision());
    }
}
