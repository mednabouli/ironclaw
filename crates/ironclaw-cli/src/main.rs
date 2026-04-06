use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use clap::{Parser, Subcommand};
use ironclaw_agents::{AgentContext, AgentHandler};
use ironclaw_channels::{CliChannel, RestChannel};
use ironclaw_config::IronClawConfig;
use ironclaw_core::{Channel, CompletionRequest, Message};
use tracing::{error, info, warn};

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

    /// List all stored sessions
    Sessions,

    /// Search across all stored messages
    Search {
        /// Text to search for
        query: String,
        /// Maximum number of results
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },

    /// Clear all messages for a session
    ClearSession {
        /// The session ID to clear
        session_id: String,
    },

    /// Manage WASM plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

/// Plugin management subcommands.
#[derive(Subcommand)]
enum PluginAction {
    /// Install a plugin from a URL
    Install {
        /// URL to the .wasm file
        url: String,
    },
    /// List installed plugins
    List,
    /// Show details of an installed plugin
    Info {
        /// Plugin name
        name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load config (use defaults if file missing)
    let cfg = if cli.config.exists() {
        IronClawConfig::from_file(&cli.config)
            .with_context(|| format!("Failed to load config: {}", cli.config.display()))?
    } else {
        warn!(
            "Config file not found at '{}', using defaults",
            cli.config.display()
        );
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
        Commands::Start => cmd_start(cfg).await?,
        Commands::Chat { model } => cmd_chat(cfg, model).await?,
        Commands::Run { prompt, json } => cmd_run(cfg, prompt, json).await?,
        Commands::Health => cmd_health(cfg).await?,
        Commands::List => cmd_list(cfg).await?,
        Commands::Sessions => cmd_sessions(cfg).await?,
        Commands::Search { query, limit } => cmd_search(cfg, query, limit).await?,
        Commands::ClearSession { session_id } => cmd_clear_session(cfg, session_id).await?,
        Commands::Plugin { action } => cmd_plugin(action).await?,
    }

    Ok(())
}

// ── start ─────────────────────────────────────────────────────────────────
async fn cmd_start(cfg: IronClawConfig) -> anyhow::Result<()> {
    let ctx = AgentContext::from_config(cfg.clone()).await?;
    let handler = Arc::new(AgentHandler::new(ctx));

    let mut handles = vec![];

    for name in &cfg.channels.enabled {
        match name.as_str() {
            "rest" => {
                let ch = RestChannel::new(cfg.channels.rest.clone());
                let handler = Arc::clone(&handler);
                let h = tokio::spawn(async move {
                    if let Err(e) = ch.start(handler).await {
                        error!(channel = "rest", error = %e, "Channel error");
                    }
                });
                handles.push(h);
            }
            "cli" => {
                let ch = CliChannel::default();
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
    let ctx = AgentContext::from_config(cfg).await?;
    let handler = Arc::new(AgentHandler::new(ctx));
    let ch = CliChannel::default();
    ch.start(handler).await
}

// ── run ───────────────────────────────────────────────────────────────────
async fn cmd_run(cfg: IronClawConfig, prompt: String, as_json: bool) -> anyhow::Result<()> {
    let ctx = AgentContext::from_config(cfg.clone()).await?;
    let provider = ctx
        .providers
        .resolve()
        .await
        .context("No provider available. Is Ollama running?")?;

    let req = CompletionRequest {
        messages: vec![
            Message::system(&cfg.agent.system_prompt),
            Message::user(&prompt),
        ],
        tools: vec![],
        max_tokens: Some(cfg.agent.max_tokens),
        temperature: Some(cfg.agent.temperature),
        stream: false,
        model: None,
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

    println!(
        "🦀 IronClaw v{}  —  Health Check",
        env!("CARGO_PKG_VERSION")
    );
    println!("{}", "─".repeat(40));

    let reg = ProviderRegistry::from_config(&cfg);

    let provider_names = vec![cfg.providers.primary.clone()]
        .into_iter()
        .chain(cfg.providers.fallback.clone())
        .collect::<std::collections::HashSet<_>>();

    for name in provider_names {
        if let Some(p) = reg.get(&name) {
            match p.health_check().await {
                Ok(_) => println!("  ✅ {name}"),
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

    let primary = &cfg.providers.primary;
    let fallback = cfg.providers.fallback.join(", ");
    println!("\n🤖 Providers: {primary} (primary)");
    if !fallback.is_empty() {
        println!("   Fallback: {fallback}");
    }

    println!("\n🔧 Tools: {}", cfg.tools.enabled.join(", "));
    println!(
        "\n💾 Memory: {} (max_history={})",
        cfg.memory.backend, cfg.memory.max_history
    );
    Ok(())
}

// ── sessions ──────────────────────────────────────────────────────────────
async fn cmd_sessions(cfg: IronClawConfig) -> anyhow::Result<()> {
    let memory = ironclaw_memory::from_config(&cfg).await?;
    let ids = memory.sessions().await?;
    if ids.is_empty() {
        println!("No sessions found.");
    } else {
        println!("📋 Sessions ({}):", ids.len());
        for id in &ids {
            println!("  • {id}");
        }
    }
    Ok(())
}

// ── search ────────────────────────────────────────────────────────────────
async fn cmd_search(cfg: IronClawConfig, query: String, limit: usize) -> anyhow::Result<()> {
    let memory = ironclaw_memory::from_config(&cfg).await?;
    let hits = memory.search(&query, limit).await?;
    if hits.is_empty() {
        println!("No results for '{query}'.");
    } else {
        println!("🔍 Results for '{query}' ({} hits):", hits.len());
        for hit in &hits {
            let role = format!("{:?}", hit.message.role).to_lowercase();
            let ts = hit.message.timestamp.format("%Y-%m-%d %H:%M:%S");
            let text = if hit.message.content.len() > 80 {
                format!("{}…", &hit.message.content[..80])
            } else {
                hit.message.content.clone()
            };
            println!("  [{ts}] ({}) {role}: {text}", hit.session_id);
        }
    }
    Ok(())
}

// ── clear-session ─────────────────────────────────────────────────────────
async fn cmd_clear_session(cfg: IronClawConfig, session_id: String) -> anyhow::Result<()> {
    let memory = ironclaw_memory::from_config(&cfg).await?;
    memory.clear(&session_id).await?;
    println!("Cleared session '{session_id}'.");
    Ok(())
}

// ── plugin ────────────────────────────────────────────────────────────────
async fn cmd_plugin(action: PluginAction) -> anyhow::Result<()> {
    let plugin_dir = ironclaw_wasm::installer::default_plugin_dir();

    match action {
        PluginAction::Install { url } => {
            println!("📦 Installing plugin from {url}");
            let result =
                ironclaw_wasm::installer::install_from_url(&url, &plugin_dir, None).await?;
            println!(
                "  ✅ Installed '{}' → {}",
                result.name,
                result.wasm_path.display()
            );
        }
        PluginAction::List => {
            let plugins = ironclaw_wasm::installer::list_installed(&plugin_dir);
            if plugins.is_empty() {
                println!("No plugins installed. Install one with: ironclaw plugin install <url>");
            } else {
                println!("🔌 Installed plugins ({}):", plugins.len());
                for p in &plugins {
                    let caps = p
                        .capabilities
                        .iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!(
                        "  • {} v{} — {} [{}]",
                        p.name, p.version, p.description, caps
                    );
                }
            }
        }
        PluginAction::Info { name } => {
            let manifest_path = plugin_dir.join(&name).join("plugin.json");
            if !manifest_path.exists() {
                anyhow::bail!("Plugin '{name}' not found in {}", plugin_dir.display());
            }
            let manifest = ironclaw_wasm::manifest::PluginManifest::from_file(&manifest_path)?;
            println!("🔌 Plugin: {}", manifest.name);
            println!("   Version:     {}", manifest.version);
            println!("   Description: {}", manifest.description);
            println!("   Author:      {}", manifest.author);
            println!("   License:     {}", manifest.license);
            let caps = manifest
                .capabilities
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            println!("   Capabilities: [{}]", caps);
            if !manifest.allowed_urls.is_empty() {
                println!("   Allowed URLs: {}", manifest.allowed_urls.join(", "));
            }
            if !manifest.allowed_env_vars.is_empty() {
                println!("   Allowed Env:  {}", manifest.allowed_env_vars.join(", "));
            }
        }
    }

    Ok(())
}
