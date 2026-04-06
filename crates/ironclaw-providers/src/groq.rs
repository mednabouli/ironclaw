
use crate::openai::OpenAIProvider;
use ironclaw_core::Provider;

/// Groq inference provider (llama-3.3-70b-versatile, mixtral-8x7b, etc.).
///
/// Thin wrapper over `OpenAIProvider` pointed at `api.groq.com`.
/// API key is never included in `Debug` output.
pub struct GroqProvider(OpenAIProvider);

impl std::fmt::Debug for GroqProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GroqProvider").field(&self.0).finish()
    }
}

impl GroqProvider {
    /// Create a new Groq provider with the given API key and model.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self(OpenAIProvider::new(api_key, model, "https://api.groq.com/openai"))
    }
}

#[async_trait::async_trait]
impl Provider for GroqProvider {
    fn name(&self) -> &'static str { "groq" }
    fn supports_vision(&self) -> bool { false }
    async fn complete(&self, req: ironclaw_core::CompletionRequest) -> anyhow::Result<ironclaw_core::CompletionResponse> { self.0.complete(req).await }
    async fn stream(&self,   req: ironclaw_core::CompletionRequest) -> anyhow::Result<ironclaw_core::BoxStream<ironclaw_core::StreamChunk>> { self.0.stream(req).await }
    async fn health_check(&self) -> anyhow::Result<()> { self.0.health_check().await }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_api_key() {
        let p = GroqProvider::new("gsk-secret", "llama-3.3-70b-versatile");
        let debug_str = format!("{:?}", p);
        assert!(!debug_str.contains("gsk-secret"), "api_key must not appear in Debug output");
        assert!(debug_str.contains("[REDACTED]"), "Debug output must contain [REDACTED]");
    }

    #[test]
    fn provider_name_is_groq() {
        let p = GroqProvider::new("key", "model");
        assert_eq!(p.name(), "groq");
    }

    #[test]
    fn does_not_support_vision() {
        let p = GroqProvider::new("key", "model");
        assert!(!p.supports_vision());
    }
}
