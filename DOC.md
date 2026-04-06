# IronClaw — Usage Documentation

***

## Installation

### Option A — Build from Source (Recommended)
```bash
git clone https://github.com/mednabouli/ironclaw
cd ironclaw
cargo build --release
# Binary is at ./target/release/ironclaw
```

### Option B — Cross-compile for Raspberry Pi / ARM
```bash
cargo install cross
cross build --release --target aarch64-unknown-linux-musl
# Copy binary to your device — no runtime dependencies needed
```

***

## Prerequisites

IronClaw uses **Ollama by default** — no API key required.

```bash
# Install Ollama
curl -fsSL https://ollama.ai/install.sh | sh

# Pull a model
ollama pull llama3.2          # lightweight (2GB)
ollama pull llama3.3:70b      # best quality (40GB)
ollama pull phi4              # fast, efficient (9GB)
```

To use cloud providers instead, just set environment variables:
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
export GROQ_API_KEY="gsk_..."
```

***

## Configuration

IronClaw reads `ironclaw.toml` from the current directory. All `${ENV_VAR}` references are expanded at startup.

```toml
# ironclaw.toml

[agent]
name          = "IronClaw"
system_prompt = "You are a helpful and concise AI assistant."
max_tokens    = 4096
temperature   = 0.7

[providers]
primary  = "ollama"              # First choice
fallback = ["claude", "groq"]   # Automatic failover

[providers.ollama]
base_url = "http://localhost:11434"
model    = "llama3.2"

[providers.claude]
api_key = "${ANTHROPIC_API_KEY}"
model   = "claude-3-5-sonnet-20241022"

[providers.groq]
api_key = "${GROQ_API_KEY}"
model   = "llama-3.3-70b-versatile"

[channels]
enabled = ["cli"]   # "cli" | "rest" | "telegram" | "discord"

[channels.rest]
host = "127.0.0.1"
port = 8080

[memory]
backend     = "memory"  # in-process session history
max_history = 50        # messages kept per session

[tools]
enabled = ["datetime", "shell"]

[tools.shell]
allowlist    = ["ls", "echo", "cat", "pwd", "date"]
timeout_secs = 30
```

***

## CLI Commands

### `start` — Launch all configured channels
```bash
ironclaw start
ironclaw --config /etc/ironclaw/prod.toml start
```
Starts every channel listed in `channels.enabled`. Channels run concurrently — you can have REST and CLI active at the same time.

### `chat` — Interactive conversation
```bash
ironclaw chat
```
Opens a colored ANSI terminal session with full conversation memory. Type `/quit` to exit.

```
🦀 IronClaw  (type /quit to exit)
──────────────────────────────────────────────────
You: What's the capital of Canada?
IronClaw: Ottawa is the capital of Canada.

You: What time is it?
IronClaw: [calls get_datetime tool]
          The current time is 5:21 PM UTC.

You: /quit

Goodbye! 👋
```

### `run` — One-shot prompt, returns and exits
```bash
# Print response to stdout
ironclaw run "Summarize the Rust ownership model in 3 bullet points"

# Output as JSON (includes usage stats, latency, model name)
ironclaw run --json "What is 2 + 2?"

# Pipe into other tools
ironclaw run "List 5 project names for a Rust web framework" | fzf
```

### `health` — Check all providers
```bash
ironclaw health
```
```
🦀 IronClaw v0.1.0  —  Health Check
────────────────────────────────────────
  ✅ ollama
  ❌ claude  (api_key not set)
  ⚪ openai  (not configured)
```

### `list` — Show active configuration
```bash
ironclaw list
```
```
🦀 IronClaw v0.1.0

📡 Channels: cli
🤖 Providers: ollama (primary)
   Fallback: claude, groq
🔧 Tools: datetime, shell
💾 Memory: memory (max_history=50)
```

***

## REST API

Enable by adding `"rest"` to your channels:
```toml
[channels]
enabled = ["rest"]

[channels.rest]
host = "0.0.0.0"
port = 8080
```

### `POST /v1/chat`
```bash
curl -X POST http://localhost:8080/v1/chat \
  -H "Content-Type: application/json" \
  -d '{
    "message":    "What is the boiling point of water?",
    "session_id": "user-abc"
  }'
```
```json
{
  "session_id": "user-abc",
  "response":   "Water boils at 100°C (212°F) at standard atmospheric pressure."
}
```

**Session memory is automatic** — send the same `session_id` across requests and IronClaw remembers the conversation history (up to `max_history` messages).

### `GET /health`
```bash
curl http://localhost:8080/health
# {"status":"ok","version":"0.1.0"}
```

***

## Tools

### Built-in: `get_datetime`
The agent automatically calls this when asked about the current time or date. No configuration needed.

### Built-in: `shell`
Runs allowlisted shell commands. The agent uses this for file inspection, directory listings, etc.

```toml
[tools.shell]
allowlist    = ["ls", "cat", "echo", "pwd", "git", "cargo"]
timeout_secs = 30
```

> **Security:** Only commands explicitly listed in `allowlist` will execute. Any other command is rejected with an error returned to the LLM.

### Adding a Custom Tool (Library usage)
```rust
use async_trait::async_trait;
use ironclaw_core::{Tool, ToolSchema};
use serde_json::{json, Value};

struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self)        -> &str { "get_weather" }
    fn description(&self) -> &str { "Get current weather for a city." }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name:        self.name().to_string(),
            description: self.description().to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string", "description": "City name" }
                },
                "required": ["city"]
            }),
        }
    }

    async fn invoke(&self, params: Value) -> anyhow::Result<Value> {
        let city = params["city"].as_str().unwrap_or("unknown");
        // Call your weather API here
        Ok(json!({ "city": city, "temp_c": 22, "condition": "sunny" }))
    }
}
```

Then register it before starting:
```rust
ctx.tools.register(Arc::new(WeatherTool));
```

***

## Using as a Library

Add to your `Cargo.toml`:
```toml
ironclaw-core      = "0.1"
ironclaw-agents    = "0.1"
ironclaw-providers = "0.1"
ironclaw-memory    = "0.1"
ironclaw-tools     = "0.1"
```

### Minimal embed example
```rust
use ironclaw_agents::{AgentContext, ReActAgent};
use ironclaw_config::IronClawConfig;
use ironclaw_core::{Agent, AgentTask};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg   = IronClawConfig::from_file("ironclaw.toml")?;
    let ctx   = AgentContext::from_config(cfg);
    let agent = ReActAgent::new(ctx);

    let task   = AgentTask::new("What is the current time?");
    let output = agent.run(task).await?;

    println!("{}", output.text);
    println!("Tokens used: {}", output.usage.total_tokens);
    Ok(())
}
```

### With conversation history
```rust
// AgentHandler automatically loads/saves history per session_id
use ironclaw_agents::{AgentContext, AgentHandler};
use ironclaw_core::{MessageHandler, InboundMessage};

let ctx     = AgentContext::from_config(cfg);
let handler = AgentHandler::new(ctx);

// First turn
let msg1 = InboundMessage::cli("My name is Med.");
let r1   = handler.handle(msg1).await?.unwrap();

// Second turn — agent remembers "Med" from history
let msg2 = InboundMessage::cli("What's my name?");
let r2   = handler.handle(msg2).await?.unwrap();

println!("{}", r2.as_str()); // "Your name is Med."
```

***

## Provider Fallback Chain

IronClaw automatically tries providers in order. If Ollama is down, it falls back to Claude, then Groq — with zero code changes.

```toml
[providers]
primary  = "ollama"
fallback = ["claude", "groq", "openai"]
```

**Health check flow:**
1. Try `ollama` → if `GET /api/tags` fails → move on
2. Try `claude` → if `api_key` is empty → skip
3. Try `groq` → returns first healthy one
4. If all fail → clear error: `"No healthy provider. Is Ollama running?"`

***

## Environment Variables Reference

| Variable | Used By | Example |
|---|---|---|
| `ANTHROPIC_API_KEY` | Claude provider | `sk-ant-...` |
| `OPENAI_API_KEY` | OpenAI provider | `sk-...` |
| `GROQ_API_KEY` | Groq provider | `gsk_...` |
| `TELEGRAM_TOKEN` | Telegram channel | `123456:ABC...` |
| `REST_AUTH_TOKEN` | REST channel | any secret string |
| `RUST_LOG` | Tracing filter override | `debug`, `ironclaw=trace` |

***

## Deployment

### systemd service
```ini
# /etc/systemd/system/ironclaw.service
[Unit]
Description=IronClaw AI Agent
After=network.target

[Service]
Type=simple
User=ironclaw
WorkingDirectory=/opt/ironclaw
EnvironmentFile=/opt/ironclaw/.env
ExecStart=/opt/ironclaw/ironclaw start
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```
```bash
sudo systemctl enable --now ironclaw
sudo journalctl -fu ironclaw
```

### Docker
```dockerfile
FROM scratch
COPY ironclaw /ironclaw
COPY ironclaw.toml /ironclaw.toml
EXPOSE 8080
CMD ["/ironclaw", "start"]
```
```bash
# Build static binary first
cross build --release --target x86_64-unknown-linux-musl

# Image is ~2MB
docker build -t ironclaw .
docker run -e ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY -p 8080:8080 ironclaw
```