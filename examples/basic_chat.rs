//! Basic chat example — one-shot prompt to Ollama.
//!
//! Run:
//!   cargo run --example basic_chat
//!
//! Requires Ollama running at localhost:11434.

use ironclaw_config::IronClawConfig;
use ironclaw_core::{CompletionRequest, Message};
use ironclaw_providers::ProviderRegistry;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    let cfg = IronClawConfig::default();
    let registry = ProviderRegistry::from_config(&cfg);

    let provider = registry
        .resolve()
        .await
        .expect("No healthy provider — is Ollama running?");

    info!(provider = provider.name(), "Using provider");

    let req = CompletionRequest {
        messages: vec![
            Message::system("You are a helpful assistant."),
            Message::user("What is the capital of France? Answer in one word."),
        ],
        tools: vec![],
        max_tokens: Some(100),
        temperature: Some(0.3),
        stream: false,
        model: None,
        response_format: Default::default(),
    };

    let resp = provider.complete(req).await?;
    println!("Response: {}", resp.text());
    println!("Tokens:   {}", resp.usage.total_tokens);
    println!("Latency:  {}ms", resp.latency_ms);

    Ok(())
}
