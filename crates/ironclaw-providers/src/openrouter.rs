use ironclaw_core::{Provider, ProviderError};

use crate::openai::OpenAIProvider;

/// OpenRouter provider — 200+ models via a single OpenAI-compatible endpoint.
///
/// Wraps `OpenAIProvider` pointed at `https://openrouter.ai/api`.
/// Supports model routing, fallback, vision, and tool calling depending
/// on the selected model.
///
/// API key is never included in `Debug` output.
pub struct OpenRouterProvider(OpenAIProvider);

impl std::fmt::Debug for OpenRouterProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("OpenRouterProvider").field(&self.0).finish()
    }
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider with the given API key and default model.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self(OpenAIProvider::new(
            api_key,
            model,
            "https://openrouter.ai/api",
        ))
    }
}

#[async_trait::async_trait]
impl Provider for OpenRouterProvider {
    fn name(&self) -> &'static str {
        "openrouter"
    }

    fn supports_vision(&self) -> bool {
        true // many OpenRouter models support vision; model-dependent
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
    fn debug_does_not_leak_api_key() {
        let p = OpenRouterProvider::new("sk-or-secret", "openai/gpt-4o");
        let debug_str = format!("{:?}", p);
        assert!(
            !debug_str.contains("sk-or-secret"),
            "api_key must not appear in Debug output"
        );
        assert!(
            debug_str.contains("[REDACTED]"),
            "Debug output must contain [REDACTED]"
        );
    }

    #[test]
    fn provider_name_is_openrouter() {
        let p = OpenRouterProvider::new("key", "openai/gpt-4o");
        assert_eq!(p.name(), "openrouter");
    }

    #[test]
    fn supports_vision() {
        let p = OpenRouterProvider::new("key", "openai/gpt-4o");
        assert!(p.supports_vision());
    }
}
