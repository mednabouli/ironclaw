# Changelog

All notable changes to IronClaw are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)  
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

_No unreleased changes._

---

## [0.1.1] — 2026-04-12

> Hardening release — API freeze, error handling, security, testing, documentation, and release infrastructure.

### Added

#### API Freeze Sprint
- `#[non_exhaustive]` on all public enums (`Role`, `StopReason`, `AgentRole`, etc.)
- Typed error enums with `thiserror` — `ProviderError`, `ToolError`, `ChannelError`, `ConfigError`, `MemoryError`
- Newtype IDs — `SessionId`, `AgentId`, `ChannelId`, `MessageId` (Deref to inner str/String)
- Builder pattern for `CompletionRequest`, `AgentTask`, `ToolSchema`
- MSRV policy set to 1.86 and enforced in CI

#### Error Hardening Sprint
- Eliminated all `unwrap()` / `expect()` from library crates — replaced with `?` and `map_err()`
- `#[must_use]` annotations on all builder methods and pure functions
- Comprehensive error documentation with `/// # Errors` sections on all fallible public methods

#### Security Sprint
- SBOM generation (CycloneDX) in CI release workflow
- Binary signing with `cosign` in release workflow
- ShellTool audit — strict allowlist enforcement, path traversal prevention
- FileReadTool / FileWriteTool — canonicalized path checking, sandbox directory enforcement
- REST channel rate limiting via `tower::limit::RateLimitLayer`
- `cargo-vet` supply chain audit — 4 trusted imports (Google, Mozilla, Zcash, Bytecode Alliance), publisher trusts, exemptions
- `cargo-fuzz` targets for config parsing and provider response deserialization
- `security.yml` — weekly `cargo audit` workflow

#### Testing & Benchmarks Sprint
- Criterion benchmarks — `registry_bench`, serde round-trip benchmarks in `benches/`
- `proptest` property-based testing for core types (Message, CompletionRequest round-trips)
- Test coverage gate in CI (94.6% line coverage at sprint completion)
- 342 tests across all crates with all features enabled

#### Documentation Sprint
- `docs.rs` metadata in all crate `Cargo.toml` files (`all-features = true`, `rustdoc-args`)
- `MIGRATION.md` — version migration guide with breaking change catalog
- `VERSIONING.md` — semver policy, stability tiers, deprecation process
- `/// # Examples` doc-tests on all core traits (`Provider`, `Tool`, `Channel`, `MemoryStore`, `Agent`)
- `DOC.md` — consolidated documentation index

#### Release & Publishing Sprint
- `cargo-semver-checks` in CI — blocks merge on semver-incompatible changes
- `release-plz.yml` workflow — automated changelog + version bump PRs
- `cargo-dist` configuration for binary distribution
- `backport.yml` workflow — automated cherry-pick to LTS branches on PR merge
- Workspace dependency versions for crates.io publishing (`version = "0.1.1"` on all internal deps)

### Fixed
- REST `chat_handler` now enforces `Authorization: Bearer <token>` when `auth_token` is configured
- API keys no longer appear in `Debug` output for `AnthropicProvider`, `OpenAIProvider`, `GroqProvider`
- `AgentContext::try_from_config` replaces panicking `from_config`
- Fixed duplicate `use std::io::Write` in `ironclaw-channels/src/cli.rs`
- Fixed unused variable warning in `ironclaw-providers/src/registry.rs`
- `retry::is_transient()` rewritten to match `ProviderError` variants instead of brittle string matching
- Flaky retry integration tests stabilized with `serial_test` crate
- `backport.yml` — fixed corrupt Unicode characters and YAML syntax errors in shell blocks

### Changed
- `[workspace.lints]` added to `Cargo.toml` — `unsafe_code = "warn"`, `missing_docs = "warn"`, clippy pedantic
- Added `rustfmt.toml` — `max_width = 100`, `imports_granularity = "Crate"`
- Provider error handling — all provider methods return structured `ProviderError` variants instead of opaque `anyhow::Error`
- Config hot-reload — `arc-swap` based config swap via `ctx.config.load()` in `ironclaw-config`

### Security
- `SECURITY.md` — vulnerability disclosure policy and threat model
- `SECURITY_AUDIT.md` — community audit checklist with v1.0 readiness tracking
- `deny.toml` — `cargo-deny` configuration for license, advisory, and source auditing
- `supply-chain/config.toml` — `cargo-vet` audit configuration with trusted imports

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

[Unreleased]: https://github.com/mednabouli/ironclaw/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/mednabouli/ironclaw/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mednabouli/ironclaw/releases/tag/v0.1.0
