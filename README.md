
# 🦀 IronClaw

[![CI](https://github.com/mednabouli/ironclaw/actions/workflows/ci.yml/badge.svg)](https://github.com/mednabouli/ironclaw/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> **Ultra-lightweight AI agent framework in Rust.**
> One binary. 8 LLM providers. 8 channels. Agent swarms. WASM plugins. ~4.4 MB.

| Metric | Value |
|--------|-------|
| Binary size | **4.4 MB** (default features, `--release`) |
| Cold startup | **~57 ms** (macOS arm64, measured) |
| RAM at idle | **~7.5 MB** RSS |

## Quick Start

```bash
# Install Ollama and pull a model
curl -fsSL https://ollama.ai/install.sh | sh
ollama pull llama3.2

# Clone and build
git clone https://github.com/mednabouli/ironclaw
cd ironclaw
cargo build --release

# Run interactive chat (uses Ollama by default)
./target/release/ironclaw chat

# One-shot prompt
./target/release/ironclaw run "What is Rust?"

# Start REST API on :8080
./target/release/ironclaw start

# Check all providers
./target/release/ironclaw health
```

## REST API Usage

```bash
# Start REST channel: channels.enabled = ["rest"] in ironclaw.toml
curl -X POST http://localhost:8080/v1/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello!", "session_id": "my-session"}'
```

## Workspace Crates

| Crate | Description |
|-------|-------------|
| `ironclaw-core`      | Traits + types (zero business logic) |
| `ironclaw-config`    | TOML config parser with env var expansion |
| `ironclaw-providers` | 8 LLM providers: Ollama, Anthropic, OpenAI, Groq, Mistral, Together, Cohere, OpenRouter |
| `ironclaw-memory`    | In-memory + SQLite persistent memory, vector store |
| `ironclaw-tools`     | Shell, DateTime built-in tools + registry |
| `ironclaw-wasm`      | WASM plugin system: manifest, installer, capability sandbox |
| `ironclaw-channels`  | CLI, REST/SSE, Telegram, Discord, Slack, WebSocket, Matrix, Webhook |
| `ironclaw-agents`    | ReAct agent loop, AgentContext, event bus |
| `ironclaw-cli`       | Binary entry point (clap CLI) |

## Feature Flags

IronClaw uses Cargo feature flags to keep the default binary small. Only what you need gets compiled in.

### CLI binary (`ironclaw-cli`)

| Feature | Default | Description |
|---------|---------|-------------|
| `rest` | **yes** | Enables the REST/SSE HTTP channel |
| `all-providers` | no | Compiles all 8 providers (default: Ollama + Anthropic + OpenAI) |
| `otel` | no | OpenTelemetry tracing export |

```bash
# Default build — Ollama/Anthropic/OpenAI + REST + CLI channels
cargo build --release -p ironclaw-cli

# Minimal build — CLI channel only, no REST server (~4.2 MB)
cargo build --release -p ironclaw-cli --no-default-features

# Full build — all 8 providers + REST
cargo build --release -p ironclaw-cli --features all-providers
```

### Providers (`ironclaw-providers`)

| Feature | Default | Provider |
|---------|---------|----------|
| `ollama` | **yes** | Ollama (local models) |
| `anthropic` | **yes** | Anthropic Claude |
| `openai` | **yes** | OpenAI GPT |
| `groq` | no | Groq |
| `mistral` | no | Mistral |
| `together` | no | Together AI |
| `cohere` | no | Cohere |
| `openrouter` | no | OpenRouter |
| `all` | no | All of the above |

### Channels (`ironclaw-channels`)

| Feature | Default | Channel |
|---------|---------|---------|
| `cli` | **yes** | Interactive terminal |
| `rest` | **yes** | REST/SSE HTTP API (axum) |
| `prometheus` | **yes** | Prometheus metrics endpoint |
| `telegram` | no | Telegram bot (teloxide) |
| `discord` | no | Discord bot (serenity) |
| `slack` | no | Slack bot |
| `websocket` | no | WebSocket server |
| `webhook` | no | Incoming webhook handler |
| `matrix` | no | Matrix chat |

### Memory (`ironclaw-memory`)

| Feature | Default | Backend |
|---------|---------|---------|
| `sqlite` | **yes** | SQLite persistent storage + vector store |
| `redis-backend` | no | Redis memory backend |

## Architecture

All providers share a single `reqwest::Client` instance, saving ~300 KB RAM and ~2 ms startup per additional provider. The SQLite connection pool is set to 1 connection to minimize idle memory.

For the full architecture diagram, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Cross-Compile

```bash
cargo install cross
cross build --release --target aarch64-unknown-linux-musl
cross build --release --target armv7-unknown-linux-gnueabihf
```

## License

Licensed under MIT OR Apache-2.0.
