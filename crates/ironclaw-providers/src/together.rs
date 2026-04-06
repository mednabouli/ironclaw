use ironclaw_core::{Provider, ProviderError};

use crate::openai::OpenAIProvider;

/// Together AI provider (meta-llama/Llama-3-70b, etc.).
///
/// Thin wrapper over `OpenAIProvider` pointed at `api.together.xyz`.
/// API key is never included in `Debug` output.
pub struct TogetherProvider(OpenAIProvider);

impl std::fmt::Debug for TogetherProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TogetherProvider").field(&self.0).finish()
    }
}

impl TogetherProvider {
    /// Create a new Together AI provider with the given API key and model.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self(OpenAIProvider::new(
            api_key,
            model,
            "https://api.together.xyz",
        ))
    }
    /// Create a new Together provider with a shared HTTP client.
    pub fn with_client(
        client: reqwest::Client,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self(OpenAIProvider::with_client(
            client,
            api_key,
            model,
            "https://api.together.xyz",
        ))
    }
}

#[async_trait::async_trait]
impl Provider for TogetherProvider {
    fn name(&self) -> &'static str {
        "together"
    }

    fn supports_vision(&self) -> bool {
        false
    }

    async fn complete(
        &self,
        req: ironclaw_core::CompletionRequest,
    ) -> Result<ironclaw_core::CompletionResponse, ProviderError> {
        self.0.complete(req).await
    }

    async fn stream(
        &self,
        req: ironclaw_core::CompletionRequest,
    ) -> Result<ironclaw_core::BoxStream<ironclaw_core::StreamChunk>, ProviderError> {
        self.0.stream(req).await
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        self.0.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_name_is_together() {
        let p = TogetherProvider::new("fake-key", "meta-llama/Llama-3-70b");
        assert_eq!(p.name(), "together");
    }

    #[test]
    fn does_not_support_vision() {
        let p = TogetherProvider::new("fake-key", "meta-llama/Llama-3-70b");
        assert!(!p.supports_vision());
    }

    #[test]
    fn debug_does_not_leak_api_key() {
        let p = TogetherProvider::new("sk-secret", "meta-llama/Llama-3-70b");
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
}
