//! Tool use example — ReAct agent with built-in tools.
//!
//! Run:
//!   cargo run --example tool_use
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

    let mut cfg = IronClawConfig::default();
    cfg.tools.enabled = vec![
        "datetime".into(),
        "calculator".into(),
        "shell".into(),
    ];
    cfg.tools.shell.allowlist = vec!["echo".into(), "date".into()];

    let ctx = AgentContext::from_config(cfg).await?;

    // Create a ReAct agent that can use tools
    let agent = ironclaw_agents::ReActAgent::new(
        "tool-demo".to_string(),
        Arc::clone(&ctx.providers),
        Arc::clone(&ctx.tools),
        5, // max iterations
    );

    info!("Sending task with tool use...");

    let task = AgentTask::new("What time is it in UTC? Also compute 142 * 37.");
    let output = agent.run(task).await?;

    println!("Agent output:\n{}", output.text);
    println!("Iterations: {}", output.iterations);

    Ok(())
}
