# ironclaw-core

Core traits and types for the [IronClaw](https://github.com/mednabouli/ironclaw) AI agent framework.

This crate defines the foundational abstractions that all IronClaw implementation crates depend on:

- **`Provider`** — LLM completion and streaming
- **`Channel`** — inbound/outbound messaging (REST, Telegram, Discord, etc.)
- **`Tool`** — function calling interface for agents
- **`MemoryStore`** — session history and vector search
- **`Agent`** / **`AgentBus`** — multi-agent orchestration

## Usage

```rust
use ironclaw_core::{Provider, CompletionRequest, CompletionResponse};

// Implement the Provider trait for your LLM backend
```

## Design Principles

- **Zero business logic** — only traits, types, and type aliases
- **Minimal dependencies** — serde, async-trait, tokio-stream, uuid, chrono
- **Send + Sync + 'static** — all trait objects are safe to share across tasks
- **`anyhow::Result`** — for all fallible public APIs

## License

MIT OR Apache-2.0
