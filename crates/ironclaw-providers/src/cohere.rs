use ironclaw_core::{Provider, ProviderError};

use crate::openai::OpenAIProvider;

/// Cohere provider (command-r-plus, command-r, etc.).
///
/// Thin wrapper over `OpenAIProvider` pointed at the Cohere OpenAI-compatible
/// endpoint at `api.cohere.com/compatibility/v1`.
/// API key is never included in `Debug` output.
pub struct CohereProvider(OpenAIProvider);

impl std::fmt::Debug for CohereProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CohereProvider").field(&self.0).finish()
    }
}

impl CohereProvider {
    /// Create a new Cohere provider with the given API key and model.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self(OpenAIProvider::new(
            api_key,
            model,
            "https://api.cohere.com/compatibility/v1",
        ))
    }
}

#[async_trait::async_trait]
impl Provider for CohereProvider {
    fn name(&self) -> &'static str {
        "cohere"
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
    fn provider_name_is_cohere() {
        let p = CohereProvider::new("fake-key", "command-r-plus");
        assert_eq!(p.name(), "cohere");
    }

    #[test]
    fn does_not_support_vision() {
        let p = CohereProvider::new("fake-key", "command-r-plus");
        assert!(!p.supports_vision());
    }

    #[test]
    fn debug_does_not_leak_api_key() {
        let p = CohereProvider::new("sk-secret", "command-r-plus");
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
