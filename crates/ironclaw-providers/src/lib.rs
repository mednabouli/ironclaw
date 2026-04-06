pub mod circuit_breaker;
pub mod compat;
pub mod cost;
pub mod registry;
pub mod retry;
pub mod sse;

#[cfg(feature = "ollama")]
pub mod ollama;
#[cfg(feature = "ollama")]
pub use ollama::OllamaProvider;

#[cfg(feature = "anthropic")]
pub mod anthropic;
#[cfg(feature = "anthropic")]
pub use anthropic::AnthropicProvider;

#[cfg(feature = "openai")]
pub mod openai;
#[cfg(feature = "openai")]
pub use openai::OpenAIProvider;

#[cfg(feature = "groq")]
pub mod groq;
#[cfg(feature = "groq")]
pub use groq::GroqProvider;

#[cfg(feature = "openrouter")]
pub mod openrouter;
#[cfg(feature = "openrouter")]
pub use openrouter::OpenRouterProvider;

#[cfg(feature = "mistral")]
pub mod mistral;
#[cfg(feature = "mistral")]
pub use mistral::MistralProvider;

#[cfg(feature = "together")]
pub mod together;
#[cfg(feature = "together")]
pub use together::TogetherProvider;

#[cfg(feature = "cohere")]
pub mod cohere;
#[cfg(feature = "cohere")]
pub use cohere::CohereProvider;

pub use circuit_breaker::{CircuitBreakerConfig, CircuitBreakerProvider};
pub use cost::{CostBreakdown, CostCalculator, ModelPricing};
pub use registry::ProviderRegistry;
pub use retry::{RetryConfig, RetryProvider};
