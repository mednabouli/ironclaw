
pub mod registry;

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

pub use registry::ProviderRegistry;
