use std::{io::IsTerminal, path::PathBuf, sync::Arc};

use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use ironclaw_agents::{AgentContext, AgentHandler};
use ironclaw_channels::{CliChannel, RestChannel};
use ironclaw_config::{ConfigWatcher, IronClawConfig};
use ironclaw_core::{Channel, CompletionRequest, Message};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

#[cfg(feature = "otel")]
mod otel;

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
        /// Show what would be cleared without actually clearing
        #[arg(long)]
        dry_run: bool,
    },

    /// Process a batch of prompts from JSONL input
    Batch {
        /// Path to a JSONL file (each line: {"prompt": "..."}). Reads stdin if omitted.
        #[arg(short, long)]
        input: Option<PathBuf>,
        /// Maximum number of concurrent requests
        #[arg(short = 'j', long, default_value_t = 4)]
        concurrency: usize,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },

    /// Manage WASM plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },

    /// Diagnose environment: check Ollama, config, providers, system info
    Doctor,

    /// Generate a default ironclaw.toml config file
    Init {
        /// Output path (default: ./ironclaw.toml)
        #[arg(short, long, default_value = "ironclaw.toml")]
        output: PathBuf,
        /// Overwrite existing file
        #[arg(long)]
        force: bool,
    },

    /// Benchmark providers side-by-side
    Bench {
        /// Prompt to benchmark with
        #[arg(default_value = "Explain the concept of recursion in 2 sentences.")]
        prompt: String,
        /// Number of iterations per provider
        #[arg(short = 'n', long, default_value_t = 3)]
        iterations: usize,
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

    // When the otel feature is enabled, bridge tracing → OTLP.
    // The guard must be held alive for the duration of main().
    #[cfg(feature = "otel")]
    let _otel_guard = otel::init_otel_tracing(&log_level)?;

    #[cfg(not(feature = "otel"))]
    {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_new(&log_level)
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .compact()
            .init();
    }

    info!(version = env!("CARGO_PKG_VERSION"), "IronClaw starting");

    match cli.command {
        Commands::Start => cmd_start(cfg, &cli.config).await?,
        Commands::Chat { model } => cmd_chat(cfg, model).await?,
        Commands::Run { prompt, json } => cmd_run(cfg, prompt, json).await?,
        Commands::Health => cmd_health(cfg).await?,
        Commands::List => cmd_list(cfg).await?,
        Commands::Sessions => cmd_sessions(cfg).await?,
        Commands::Search { query, limit } => cmd_search(cfg, query, limit).await?,
        Commands::ClearSession {
            session_id,
            dry_run,
        } => cmd_clear_session(cfg, session_id, dry_run).await?,
        Commands::Batch { input, concurrency } => cmd_batch(cfg, input, concurrency).await?,
        Commands::Completions { shell } => {
            cmd_completions(shell);
            return Ok(());
        }
        Commands::Plugin { action } => cmd_plugin(action).await?,
        Commands::Doctor => cmd_doctor(cfg).await?,
        Commands::Init { output, force } => cmd_init(output, force)?,
        Commands::Bench { prompt, iterations } => cmd_bench(cfg, prompt, iterations).await?,
    }

    Ok(())
}

// ── start ─────────────────────────────────────────────────────────────────
async fn cmd_start(cfg: IronClawConfig, config_path: &std::path::Path) -> anyhow::Result<()> {
    let ctx = AgentContext::from_config(cfg.clone()).await?;

    // Start hot-reload watcher — keeps previous config on parse errors.
    let _watcher = ConfigWatcher::start(config_path, ctx.config.clone())?;

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
    ch.start(handler).await.map_err(|e| anyhow::anyhow!(e))
}

// ── run ───────────────────────────────────────────────────────────────────
async fn cmd_run(cfg: IronClawConfig, prompt: String, as_json: bool) -> anyhow::Result<()> {
    let ctx = AgentContext::from_config(cfg.clone()).await?;
    let provider = ctx
        .providers
        .resolve()
        .await
        .context("No provider available. Is Ollama running?")?;

    let req = CompletionRequest::builder(vec![
        Message::system(&cfg.agent.system_prompt),
        Message::user(&prompt),
    ])
    .max_tokens(cfg.agent.max_tokens)
    .temperature(cfg.agent.temperature)
    .build();

    // Show spinner while waiting for model response (only when not piped)
    let spinner = if std::io::stderr().is_terminal() && !as_json {
        let sp = indicatif::ProgressBar::new_spinner();
        sp.set_style(
            indicatif::ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .expect("valid template")
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        sp.set_message("Thinking...");
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        Some(sp)
    } else {
        None
    };

    let resp = provider.complete(req).await?;

    if let Some(sp) = spinner {
        sp.finish_and_clear();
    }

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
async fn cmd_clear_session(
    cfg: IronClawConfig,
    session_id: String,
    dry_run: bool,
) -> anyhow::Result<()> {
    if dry_run {
        let memory = ironclaw_memory::from_config(&cfg).await?;
        let sid = ironclaw_core::SessionId::from(session_id.as_str());
        let history = memory.history(&sid, 1000).await?;
        println!(
            "Would clear session '{session_id}' ({} messages). Use without --dry-run to proceed.",
            history.len()
        );
        return Ok(());
    }
    let memory = ironclaw_memory::from_config(&cfg).await?;
    let sid = ironclaw_core::SessionId::from(session_id.as_str());
    memory.clear(&sid).await?;
    println!("Cleared session '{session_id}'.");
    Ok(())
}

// ── batch ─────────────────────────────────────────────────────────────────

/// Input line for batch mode.
#[derive(Deserialize)]
struct BatchInput {
    prompt: String,
    #[serde(default)]
    id: Option<String>,
}

/// Output line for batch mode.
#[derive(Serialize)]
struct BatchOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    prompt: String,
    response: String,
    model: String,
    tokens_total: u32,
    latency_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

async fn cmd_batch(
    cfg: IronClawConfig,
    input: Option<PathBuf>,
    concurrency: usize,
) -> anyhow::Result<()> {
    use tokio::sync::Semaphore;

    // Read all JSONL lines
    let raw = match &input {
        Some(path) => tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Cannot read {}", path.display()))?,
        None => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("Failed to read stdin")?;
            buf
        }
    };

    let lines: Vec<BatchInput> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| {
            serde_json::from_str(l).with_context(|| format!("Invalid JSON on line {}", i + 1))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    if lines.is_empty() {
        info!("Empty batch input — nothing to do");
        return Ok(());
    }

    info!(count = lines.len(), concurrency, "Processing batch");

    let ctx = AgentContext::from_config(cfg.clone()).await?;
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let system_prompt = cfg.agent.system_prompt.clone();
    let max_tokens = cfg.agent.max_tokens;
    let temperature = cfg.agent.temperature;

    let mut handles = Vec::with_capacity(lines.len());

    for item in lines {
        let permit = semaphore.clone().acquire_owned().await?;
        let ctx = ctx.clone();
        let system_prompt = system_prompt.clone();

        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let result = async {
                let provider = ctx
                    .providers
                    .resolve()
                    .await
                    .context("No provider available")?;
                let req = CompletionRequest::builder(vec![
                    Message::system(&system_prompt),
                    Message::user(&item.prompt),
                ])
                .max_tokens(max_tokens)
                .temperature(temperature)
                .build();
                provider.complete(req).await
            }
            .await;

            match result {
                Ok(resp) => BatchOutput {
                    id: item.id,
                    prompt: item.prompt,
                    response: resp.text().to_string(),
                    model: resp.model.clone(),
                    tokens_total: resp.usage.total_tokens,
                    latency_ms: resp.latency_ms,
                    error: None,
                },
                Err(e) => BatchOutput {
                    id: item.id,
                    prompt: item.prompt,
                    response: String::new(),
                    model: String::new(),
                    tokens_total: 0,
                    latency_ms: 0,
                    error: Some(e.to_string()),
                },
            }
        }));
    }

    for handle in handles {
        let output = handle.await?;
        println!("{}", serde_json::to_string(&output)?);
    }

    Ok(())
}

// ── completions ───────────────────────────────────────────────────────────
fn cmd_completions(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "ironclaw", &mut std::io::stdout());
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

// ── doctor ────────────────────────────────────────────────────────────────
async fn cmd_doctor(cfg: IronClawConfig) -> anyhow::Result<()> {
    println!("🩺 IronClaw Doctor v{}", env!("CARGO_PKG_VERSION"));
    println!("{}", "─".repeat(50));

    // System info
    println!("\n📋 System:");
    println!(
        "  OS:       {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("  Rust:     {}", env!("CARGO_PKG_RUST_VERSION", "unknown"));
    println!("  Version:  {}", env!("CARGO_PKG_VERSION"));

    // Config check
    println!("\n⚙️  Config:");
    println!("  Primary provider: {}", cfg.providers.primary);
    println!("  Fallback chain:   {:?}", cfg.providers.fallback);
    println!("  Channels:         {:?}", cfg.channels.enabled);
    println!("  Memory backend:   {}", cfg.memory.backend);
    println!("  Tools enabled:    {:?}", cfg.tools.enabled);

    // Ollama check
    println!("\n🦙 Ollama:");
    let ollama_url = format!("{}/api/tags", cfg.providers.ollama.base_url);
    match reqwest::get(&ollama_url).await {
        Ok(resp) if resp.status().is_success() => {
            println!(
                "  ✅ Ollama is running at {}",
                cfg.providers.ollama.base_url
            );
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if let Some(models) = body.get("models").and_then(|m| m.as_array()) {
                    let names: Vec<&str> = models
                        .iter()
                        .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                        .collect();
                    if names.is_empty() {
                        println!("  ⚠️  No models installed. Run: ollama pull llama3.2");
                    } else {
                        println!("  Models: {}", names.join(", "));
                    }
                }
            }
        }
        Ok(resp) => {
            println!("  ❌ Ollama responded with status {}", resp.status());
        }
        Err(_) => {
            println!(
                "  ❌ Ollama not reachable at {}",
                cfg.providers.ollama.base_url
            );
            println!("     Install: https://ollama.com  then run: ollama serve");
        }
    }

    // Provider health
    println!("\n🤖 Providers:");
    let reg = ironclaw_providers::ProviderRegistry::from_config(&cfg);
    let all_names: std::collections::HashSet<String> =
        std::iter::once(cfg.providers.primary.clone())
            .chain(cfg.providers.fallback.clone())
            .collect();
    for name in &all_names {
        if let Some(p) = reg.get(name) {
            match p.health_check().await {
                Ok(_) => println!("  ✅ {name}"),
                Err(e) => println!("  ❌ {name}: {e}"),
            }
        } else {
            println!("  ⚪ {name} (not configured)");
        }
    }

    // Memory backend check
    println!("\n💾 Memory:");
    match ironclaw_memory::from_config(&cfg).await {
        Ok(mem) => {
            let sessions = mem.sessions().await.unwrap_or_default();
            println!(
                "  ✅ {} backend OK ({} sessions)",
                cfg.memory.backend,
                sessions.len()
            );
        }
        Err(e) => println!("  ❌ {}: {e}", cfg.memory.backend),
    }

    // Plugins check
    println!("\n🔌 Plugins:");
    let plugin_dir = ironclaw_wasm::installer::default_plugin_dir();
    let plugins = ironclaw_wasm::installer::list_installed(&plugin_dir);
    if plugins.is_empty() {
        println!("  No plugins installed.");
    } else {
        println!("  {} plugins installed", plugins.len());
        for p in &plugins {
            println!("    • {} v{}", p.name, p.version);
        }
    }

    println!("\n{}", "─".repeat(50));
    println!("✅ Doctor check complete.");
    Ok(())
}

// ── init ──────────────────────────────────────────────────────────────────
fn cmd_init(output: PathBuf, force: bool) -> anyhow::Result<()> {
    if output.exists() && !force {
        anyhow::bail!(
            "Config file '{}' already exists. Use --force to overwrite.",
            output.display()
        );
    }

    let template = r#"# IronClaw Configuration
# Docs: https://github.com/mednabouli/ironclaw

[agent]
name = "IronClaw"
system_prompt = "You are IronClaw, a helpful AI assistant."
max_tokens = 4096
temperature = 0.7

[providers]
primary = "ollama"
fallback = []

[providers.retry]
enabled = true
max_retries = 3
base_delay_ms = 500
max_delay_ms = 30000

[providers.ollama]
base_url = "http://localhost:11434"
model = "llama3.2"

[providers.claude]
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-3-5-sonnet-20241022"

[providers.openai]
api_key = "${OPENAI_API_KEY}"
model = "gpt-4o-mini"

[providers.groq]
api_key = "${GROQ_API_KEY}"
model = "llama-3.3-70b-versatile"

[channels]
enabled = ["cli"]

[channels.rest]
host = "127.0.0.1"
port = 8080
auth_token = ""

[channels.rate_limit]
enabled = false
capacity = 20
refill_tokens = 1
refill_interval_secs = 3

[memory]
backend = "memory"
max_history = 50
path = "~/.ironclaw/memory.db"

[tools]
enabled = ["datetime", "shell", "calculator"]

[tools.shell]
allowlist = ["ls", "echo", "cat", "date", "uname"]
timeout_secs = 30

[telemetry]
level = "info"
format = "pretty"
"#;

    std::fs::write(&output, template)?;
    println!("✅ Created {}", output.display());
    println!("   Edit providers and channels, then run: ironclaw doctor");
    Ok(())
}

// ── bench ─────────────────────────────────────────────────────────────────
async fn cmd_bench(cfg: IronClawConfig, prompt: String, iterations: usize) -> anyhow::Result<()> {
    use std::time::Instant;

    println!("⏱  IronClaw Provider Benchmark");
    println!("{}", "─".repeat(60));
    println!("Prompt: \"{prompt}\"");
    println!("Iterations: {iterations}\n");

    let reg = ironclaw_providers::ProviderRegistry::from_config(&cfg);

    let all_names: Vec<String> = std::iter::once(cfg.providers.primary.clone())
        .chain(cfg.providers.fallback.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    for name in &all_names {
        let provider = match reg.get(name) {
            Some(p) => p,
            None => {
                println!("{name}: ⚪ not configured\n");
                continue;
            }
        };

        if provider.health_check().await.is_err() {
            println!("{name}: ❌ unhealthy\n");
            continue;
        }

        let mut latencies = Vec::with_capacity(iterations);
        let mut tokens_total = 0u32;
        let mut last_model = String::new();

        for _ in 0..iterations {
            let req = CompletionRequest::builder(vec![
                Message::system(&cfg.agent.system_prompt),
                Message::user(&prompt),
            ])
            .max_tokens(cfg.agent.max_tokens)
            .temperature(cfg.agent.temperature)
            .build();

            let start = Instant::now();
            match provider.complete(req).await {
                Ok(resp) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    latencies.push(elapsed);
                    tokens_total += resp.usage.total_tokens;
                    last_model = resp.model.clone();
                }
                Err(e) => {
                    println!("{name}: ❌ error: {e}\n");
                    break;
                }
            }
        }

        if latencies.is_empty() {
            continue;
        }

        let avg = latencies.iter().sum::<u64>() / latencies.len() as u64;
        let min = *latencies.iter().min().unwrap_or(&0);
        let max = *latencies.iter().max().unwrap_or(&0);
        let avg_tokens = tokens_total / latencies.len() as u32;

        println!("{name} ({last_model}):");
        println!("  Latency:  avg={avg}ms  min={min}ms  max={max}ms");
        println!("  Tokens:   avg={avg_tokens}/req  total={tokens_total}");
        println!();
    }

    println!("{}", "─".repeat(60));
    Ok(())
}
