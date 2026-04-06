# Changelog

All notable changes to IronClaw are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)  
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

### Added
- `SECURITY.md` — vulnerability disclosure policy and threat model
- `CONTRIBUTING.md` — full contributor guide with PR checklist, branch naming, commit format
- `CHANGELOG.md` — this file
- `.gitignore` — excludes `target/`, `.env`, `*.db`, `*.wasm`, `.DS_Store`
- `.env.example` — all environment variables documented with example values

### Fixed
- REST `chat_handler` now enforces `Authorization: Bearer <token>` when `auth_token` is configured
- API keys no longer appear in `Debug` output for `AnthropicProvider`, `OpenAIProvider`, `GroqProvider`
- `AgentContext::try_from_config` replaces panicking `from_config`
- Fixed duplicate `use std::io::Write` in `ironclaw-channels/src/cli.rs`
- Fixed unused variable warning in `ironclaw-providers/src/registry.rs`

### Changed
- `[workspace.lints]` added to `Cargo.toml` — `unsafe_code = "forbid"`, `unwrap_used = "deny"`
- Added `rustfmt.toml` — `max_width = 100`, `imports_granularity = "Crate"`

---

## [0.1.0] — 2026-04-05

> Initial public release. Foundation milestone.

### Added

#### Core (`ironclaw-core`)
- `Provider` trait — LLM provider abstraction with `complete`, `stream`, `health_check`
- `Channel` trait — messaging channel abstraction (`start`, `send`, `stop`)
- `MessageHandler` trait — processes `InboundMessage`, returns optional `OutboundMessage`
- `Tool` trait — callable LLM tool with `name`, `description`, `schema`, `invoke`
- `MemoryStore` trait — session memory with `push`, `history`, `clear`
- `Agent` trait — autonomous agent with `run(AgentTask) -> AgentOutput`
- `AgentBus` trait — inter-agent message bus
- `CompletionRequest` / `CompletionResponse` unified types
- `BoxStream<StreamChunk>` for provider-level streaming
- `Message` with `Role::{System, User, Assistant, Tool}`
- `AgentTask`, `AgentOutput`, `AgentRole`, `TokenUsage`, `StopReason`
- `InboundMessage`, `OutboundMessage`, `ChannelId`
- `ToolSchema`, `ToolCall`, `ToolResult`

#### Config (`ironclaw-config`)
- `IronClawConfig` — full TOML configuration struct with serde
- `${ENV_VAR}` expansion in all string config values
- `IronClawConfig::from_file(path)` and `IronClawConfig::default()`
- Provider configs: `OllamaConfig`, `ClaudeConfig`, `OpenAIConfig`, `GroqConfig`
- Channel configs: `RestConfig`, `TelegramConfig`
- Memory config: `MemoryConfig` (backend, max_history, path)
- Tools config: `ToolsConfig`, `ShellConfig` (allowlist, timeout)
- Agent config: `AgentConfig` (system_prompt, max_tokens, temperature, max_iterations)

#### Providers (`ironclaw-providers`)
- `OllamaProvider` — NDJSON streaming, tool calling, health via `/api/tags`
- `AnthropicProvider` — SSE streaming, vision support, tool use (Claude 3.5 Sonnet)
- `OpenAIProvider` — SSE streaming, function calling (GPT-4o, GPT-4o-mini)
- `GroqProvider` — OpenAI-compatible wrapper (Llama 3.3 70B)
- `ProviderRegistry` — dynamic registration + automatic fallback chain resolution
- Per-provider health checks with timeout

#### Memory (`ironclaw-memory`)
- `InMemoryStore` — `DashMap<SessionId, VecDeque<Message>>` with `max_history` cap
- Thread-safe, zero-persistence, suitable for development and testing

#### Tools (`ironclaw-tools`)
- `DateTimeTool` — returns UTC datetime, Unix timestamp, day of week; supports IANA timezones
- `ShellTool` — executes allowlisted shell commands with timeout enforcement
- `ToolRegistry` — dynamic tool registration, `all_schemas()`, `filtered_schemas(allowlist)`

#### Channels (`ironclaw-channels`)
- `CliChannel` — colored ANSI terminal, tokio stdin loop, `/quit` command
- `RestChannel` — axum 0.7, `POST /v1/chat`, `GET /health`, CORS headers

#### Agents (`ironclaw-agents`)
- `ReActAgent` — reason + act loop, up to 10 iterations, tool call parsing + execution
- `AgentHandler` — `MessageHandler` implementation with session memory integration
- `LocalBus` — in-process `AgentBus` with `DashMap<AgentId, Arc<dyn Agent>>`
- `AgentContext` — dependency injection root (config, provider registry, memory, tools)

#### CLI (`ironclaw-cli`)
- `ironclaw start` — launch all configured channels concurrently
- `ironclaw chat` — interactive CLI conversation
- `ironclaw run <prompt>` — one-shot prompt, print response, exit
- `ironclaw run --json <prompt>` — JSON output with metadata
- `ironclaw health` — check all provider health status
- `ironclaw list` — display active configuration summary
- `--config <path>` global flag — override config file location

#### WASM (`ironclaw-wasm`)
- Type-safe plugin trait stub (Phase 6 placeholder, `wasmtime` not yet wired)

#### Project
- Cargo workspace monorepo with 9 crates
- Release profile: `opt-level = "z"`, `lto = true`, `strip = true` → ≤ 2MB binary
- Cross-compilation support: `aarch64-unknown-linux-musl`, `armv7-unknown-linux-musleabihf`, `riscv64gc-unknown-linux-gnu`
- `Cross.toml` for Docker-based cross-compilation
- Unit test suite across all crates
- `ARCHITECTURE.md` — crate dependency graph + design decisions
- `REQUIREMENTS.md` — full functional + non-functional requirements
- `README.md` — quick start guide
- `GITHUB_SETUP.md` — repository setup instructions
- GitHub Actions: CI workflow + Release workflow with binary builds

---

[Unreleased]: https://github.com/mednabouli/ironclaw/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mednabouli/ironclaw/releases/tag/v0.1.0
