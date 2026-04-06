
use anyhow::Context;
use clap::{Parser, Subcommand};
use std::{path::PathBuf, sync::Arc};
use tracing::{error, info, warn};

use ironclaw_agents::{AgentContext, AgentHandler};
use ironclaw_channels::{CliChannel, RestChannel};
use ironclaw_config::IronClawConfig;
use ironclaw_core::{Channel, CompletionRequest, Message};

#[derive(Parser)]
#[command(
    name    = "ironclaw",
    version = env!("CARGO_PKG_VERSION"),
    about   = "🦀 IronClaw — ultra-lightweight AI agent framework",
    long_about = None
)]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "ironclaw.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start all configured channels
    Start,

    /// Interactive CLI chat session
    Chat {
        /// Override the default model
        #[arg(long)]
        model: Option<String>,
    },

    /// One-shot prompt, print response and exit
    Run {
        prompt: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Check health of all configured providers
    Health,

    /// List configured providers, channels, and tools
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load config (use defaults if file missing)
    let cfg = if cli.config.exists() {
        IronClawConfig::from_file(&cli.config)
            .with_context(|| format!("Failed to load config: {}", cli.config.display()))?
    } else {
        warn!("Config file not found at '{}', using defaults", cli.config.display());
        IronClawConfig::default()
    };

    // Init tracing
    let log_level = cfg.telemetry.level.clone();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_new(&log_level)
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .compact()
        .init();

    info!(version = env!("CARGO_PKG_VERSION"), "IronClaw starting");

    match cli.command {
        Commands::Start   => cmd_start(cfg).await?,
        Commands::Chat { model } => cmd_chat(cfg, model).await?,
        Commands::Run { prompt, json } => cmd_run(cfg, prompt, json).await?,
        Commands::Health  => cmd_health(cfg).await?,
        Commands::List    => cmd_list(cfg).await?,
    }

    Ok(())
}

// ── start ─────────────────────────────────────────────────────────────────
async fn cmd_start(cfg: IronClawConfig) -> anyhow::Result<()> {
    let ctx     = AgentContext::from_config(cfg.clone());
    let handler = Arc::new(AgentHandler::new(ctx));

    let mut handles = vec![];

    for name in &cfg.channels.enabled {
        match name.as_str() {
            "rest" => {
                let ch      = RestChannel::new(cfg.channels.rest.clone());
                let handler = Arc::clone(&handler);
                let h = tokio::spawn(async move {
                    if let Err(e) = ch.start(handler).await {
                        error!(channel = "rest", error = %e, "Channel error");
                    }
                });
                handles.push(h);
            }
            "cli" => {
                let ch      = CliChannel::default();
                let handler = Arc::clone(&handler);
                let h = tokio::spawn(async move {
                    if let Err(e) = ch.start(handler).await {
                        error!(channel = "cli", error = %e, "Channel error");
                    }
                });
                handles.push(h);
            }
            other => warn!("Unknown channel: {other}"),
        }
    }

    if handles.is_empty() {
        warn!("No channels configured. Add channels.enabled = [\"cli\"] to ironclaw.toml");
    }

    for h in handles {
        h.await?;
    }
    Ok(())
}

// ── chat ──────────────────────────────────────────────────────────────────
async fn cmd_chat(cfg: IronClawConfig, _model: Option<String>) -> anyhow::Result<()> {
    let ctx     = AgentContext::from_config(cfg);
    let handler = Arc::new(AgentHandler::new(ctx));
    let ch      = CliChannel::default();
    ch.start(handler).await
}

// ── run ───────────────────────────────────────────────────────────────────
async fn cmd_run(cfg: IronClawConfig, prompt: String, as_json: bool) -> anyhow::Result<()> {
    let ctx      = AgentContext::from_config(cfg.clone());
    let provider = ctx.providers.resolve().await
        .context("No provider available. Is Ollama running?")?;

    let req = CompletionRequest {
        messages:    vec![
            Message::system(&cfg.agent.system_prompt),
            Message::user(&prompt),
        ],
        tools:       vec![],
        max_tokens:  Some(cfg.agent.max_tokens),
        temperature: Some(cfg.agent.temperature),
        stream:      false,
        model:       None,
    };

    let resp = provider.complete(req).await?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        print!("{}", resp.text());
    }
    Ok(())
}

// ── health ────────────────────────────────────────────────────────────────
async fn cmd_health(cfg: IronClawConfig) -> anyhow::Result<()> {
    use ironclaw_providers::ProviderRegistry;

    println!("🦀 IronClaw v{}  —  Health Check", env!("CARGO_PKG_VERSION"));
    println!("{}", "─".repeat(40));

    let reg = ProviderRegistry::from_config(&cfg);

    let provider_names = vec![
        cfg.providers.primary.clone()
    ].into_iter()
     .chain(cfg.providers.fallback.clone())
     .collect::<std::collections::HashSet<_>>();

    for name in provider_names {
        if let Some(p) = reg.get(&name) {
            match p.health_check().await {
                Ok(_)  => println!("  ✅ {name}"),
                Err(e) => println!("  ❌ {name}  ({e})"),
            }
        } else {
            println!("  ⚪ {name}  (not configured)");
        }
    }
    println!("{}", "─".repeat(40));
    Ok(())
}

// ── list ──────────────────────────────────────────────────────────────────
async fn cmd_list(cfg: IronClawConfig) -> anyhow::Result<()> {
    println!("🦀 IronClaw v{}", env!("CARGO_PKG_VERSION"));
    println!("\n📡 Channels: {}", cfg.channels.enabled.join(", "));

    let primary  = &cfg.providers.primary;
    let fallback = cfg.providers.fallback.join(", ");
    println!("\n🤖 Providers: {primary} (primary)");
    if !fallback.is_empty() { println!("   Fallback: {fallback}"); }

    println!("\n🔧 Tools: {}", cfg.tools.enabled.join(", "));
    println!("\n💾 Memory: {} (max_history={})", cfg.memory.backend, cfg.memory.max_history);
    Ok(())
}
