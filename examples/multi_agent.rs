//! Multi-agent example — Router dispatches to specialist agents.
//!
//! Run:
//!   cargo run --example multi_agent
//!
//! Requires Ollama running at localhost:11434.

use std::sync::Arc;

use ironclaw_agents::AgentContext;
use ironclaw_config::IronClawConfig;
use ironclaw_core::{AgentTask, Agent};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    let cfg = IronClawConfig::default();
    let ctx = AgentContext::from_config(cfg).await?;

    // Create a chain-of-thought agent that forces step-by-step reasoning
    let cot_agent = ironclaw_agents::ChainOfThoughtAgent::new(
        "cot-agent".to_string(),
        Arc::clone(&ctx.providers),
    );

    info!("Running Chain-of-Thought agent...");

    let task = AgentTask::new(
        "A farmer has 17 sheep. All but 9 run away. How many sheep does he have left?"
    );
    let output = cot_agent.run(task).await?;

    println!("CoT output:\n{}", output.text);

    Ok(())
}
