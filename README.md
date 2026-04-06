
# 🦀 IronClaw

[![CI](https://github.com/mednabouli/ironclaw/actions/workflows/ci.yml/badge.svg)](https://github.com/mednabouli/ironclaw/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

> **Ultra-lightweight AI agent framework in Rust.**  
> One binary. Any LLM. Every channel. Agent swarms. WASM plugins.

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
| `ironclaw-providers` | Ollama, Claude, OpenAI, Groq providers |
| `ironclaw-memory`    | In-memory session store (SQLite in Phase 4) |
| `ironclaw-tools`     | Shell, DateTime built-in tools + registry |
| `ironclaw-wasm`      | WASM plugin sandbox (stub for Phase 6) |
| `ironclaw-channels`  | REST/SSE + CLI channels |
| `ironclaw-agents`    | ReAct agent, AgentContext, MessageHandler |
| `ironclaw-cli`       | Binary entry point (clap CLI) |

## Cross-Compile

```bash
cargo install cross
cross build --release --target aarch64-unknown-linux-musl
cross build --release --target armv7-unknown-linux-gnueabihf
```

## License

Licensed under MIT OR Apache-2.0.
