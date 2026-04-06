use ironclaw_core::Provider;

use crate::openai::OpenAIProvider;

/// Mistral AI provider (mistral-large-latest, mistral-small-latest, etc.).
///
/// Thin wrapper over `OpenAIProvider` pointed at `api.mistral.ai`.
/// API key is never included in `Debug` output.
pub struct MistralProvider(OpenAIProvider);

impl std::fmt::Debug for MistralProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("MistralProvider").field(&self.0).finish()
    }
}

impl MistralProvider {
    /// Create a new Mistral provider with the given API key and model.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self(OpenAIProvider::new(
            api_key,
            model,
            "https://api.mistral.ai",
        ))
    }
}

#[async_trait::async_trait]
impl Provider for MistralProvider {
    fn name(&self) -> &'static str {
        "mistral"
    }

    fn supports_vision(&self) -> bool {
        false
    }

    async fn complete(
        &self,
        req: ironclaw_core::CompletionRequest,
    ) -> anyhow::Result<ironclaw_core::CompletionResponse> {
        self.0.complete(req).await
    }

    async fn stream(
        &self,
        req: ironclaw_core::CompletionRequest,
    ) -> anyhow::Result<ironclaw_core::BoxStream<ironclaw_core::StreamChunk>> {
        self.0.stream(req).await
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        self.0.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_name_is_mistral() {
        let p = MistralProvider::new("fake-key", "mistral-large-latest");
        assert_eq!(p.name(), "mistral");
    }

    #[test]
    fn does_not_support_vision() {
        let p = MistralProvider::new("fake-key", "mistral-large-latest");
        assert!(!p.supports_vision());
    }

    #[test]
    fn debug_does_not_leak_api_key() {
        let p = MistralProvider::new("sk-secret", "mistral-large-latest");
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
