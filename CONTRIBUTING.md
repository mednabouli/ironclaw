# 🤝 Contributing to IronClaw

Thank you for taking the time to contribute! IronClaw is MIT OR Apache-2.0 and welcomes contributors of all skill levels.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Quick Start](#quick-start)
- [Finding an Issue](#finding-an-issue)
- [Branch Naming](#branch-naming)
- [Commit Format](#commit-format)
- [Development Workflow](#development-workflow)
- [PR Requirements](#pr-requirements)
- [Code Style](#code-style)
- [Testing Guide](#testing-guide)
- [Adding a Provider](#adding-a-provider)
- [Adding a Channel](#adding-a-channel)
- [Adding a Tool](#adding-a-tool)
- [Documentation Standards](#documentation-standards)
- [Release Process](#release-process)

---

## Code of Conduct

Be kind. Be constructive. No harassment, discrimination, or gatekeeping.  
Violations → email `conduct@ironclaw.dev`.

---

## Quick Start

```bash
# 1. Fork the repo on GitHub, then clone your fork
git clone https://github.com/mednabouli/ironclaw.git
cd ironclaw

# 2. Install Rust stable (1.80+ required)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable

# 3. Install dev tooling
cargo install just          # task runner
cargo install cargo-deny    # license + CVE checks
cargo install cargo-llvm-cov # code coverage

# 4. Copy env vars and configure
cp .env.example .env
# Edit .env — at minimum set ANTHROPIC_API_KEY or leave blank to use Ollama

# 5. Install Ollama for local testing (optional but recommended)
# https://ollama.ai/install.sh
ollama pull llama3.2

# 6. Run the full test suite
just test

# 7. Verify everything passes
just lint
```

---

## Finding an Issue

| Want to... | Go here |
|-----------|---------|
| Fix something broken | Issues labeled `bug` |
| Build a feature | Issues labeled `feat` + your skill level |
| Improve docs | Issues labeled `docs` |
| Write tests | Issues labeled `test` |
| First contribution | Issues labeled `good first issue` |

**Priority labels:**

| Label | Meaning |
|-------|---------|
| `P0 · blocker` 🔴 | Fix before anything else — security or crash |
| `P1 · critical` 🟠 | Blocks a milestone |
| `P2 · important` 🟡 | Scheduled milestone feature |
| `P3 · nice-to-have` 🟢 | Quality of life improvement |
| `P4 · idea` 🔵 | Under discussion, not committed |

**Always comment on an issue before starting work** to avoid duplicate effort. A maintainer will assign it to you.

---

## Branch Naming

```
<type>/<issue-number>-<short-description>

Examples:
  feat/10-sqlite-memory
  fix/02-rest-auth-enforcement
  docs/18-core-trait-comments
  chore/07-rustfmt-config
  perf/15-provider-connection-pool
```

| Type | When to use |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `chore` | Deps, CI, tooling |
| `perf` | Performance improvement |
| `refactor` | Code restructure, no behavior change |
| `test` | Adding or fixing tests |
| `security` | Security fix |

---

## Commit Format

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description> (#issue)

[optional body]

[optional footer]
```

**Examples:**
```
feat(memory): add SQLite persistence backend (#10)
fix(rest): enforce Bearer token auth in chat_handler (#02)
docs(core): add /// doc comments to all public traits (#18)
chore(workspace): add rustfmt.toml and clippy.toml (#07)
perf(providers): reuse reqwest::Client via Arc (#22)
security(providers): redact API keys in Debug output (#02)
```

**Scopes:** `core`, `config`, `providers`, `memory`, `tools`, `channels`, `agents`, `cli`, `wasm`, `workspace`, `ci`, `docs`

---

## Development Workflow

```bash
# Check everything compiles
just check

# Run all tests (no live LLM needed — uses wiremock mocks)
just test

# Run a specific crate's tests
cargo test -p ironclaw-core

# Run a specific test
cargo test -p ironclaw-memory -- inmemory::tests::push_and_retrieve

# Lint (must pass before PR)
just lint

# Format (applied automatically in CI)
just fmt

# Build release binary and check size
just build
just size

# Generate and open docs
just docs

# Run the CLI locally
cargo run --bin ironclaw -- chat
cargo run --bin ironclaw -- health
cargo run --bin ironclaw -- run "What is Rust?"
```

---

## PR Requirements

Every PR must satisfy all of the following before review:

### Automated Checks (CI enforces)
- [ ] `cargo test --workspace --all-features` — all tests pass
- [ ] `cargo clippy --workspace --all-features -- -D warnings` — zero warnings
- [ ] `cargo fmt --all -- --check` — formatting correct
- [ ] `cargo deny check` — no license violations or CVEs
- [ ] `cargo doc --workspace --no-deps` — zero doc warnings

### Manual Checklist
- [ ] New public items have `///` doc comments with at least one sentence
- [ ] New features have unit tests covering happy path + at least one error case
- [ ] No `unwrap()` or `expect()` in library code — use `?` or `.map_err()`
- [ ] No `println!()` in library code — use `tracing::info!()` / `tracing::debug!()`
- [ ] No `unsafe` blocks without a `// SAFETY:` comment explaining why it is sound
- [ ] `CHANGELOG.md` updated under `[Unreleased]` section
- [ ] PR description explains **what** changed and **why**
- [ ] Breaking changes to `ironclaw-core` traits noted explicitly

### PR Title Format
```
feat(memory): add SQLite backend (#10)
^    ^         ^                   ^
type scope     description         issue
```

---

## Code Style

### Rust Guidelines

```rust
// ✅ Use ? for error propagation
fn load_config(path: &str) -> anyhow::Result<IronClawConfig> {
    let contents = std::fs::read_to_string(path)?;
    let config: IronClawConfig = toml::from_str(&contents)?;
    Ok(config)
}

// ❌ Never unwrap in library code
fn load_config(path: &str) -> IronClawConfig {
    let contents = std::fs::read_to_string(path).unwrap(); // NO
    toml::from_str(&contents).unwrap()                     // NO
}

// ✅ Use tracing for output
tracing::info!("Provider registered: {}", name);
tracing::debug!("Request: {:?}", req);
tracing::warn!("Provider unhealthy, trying fallback: {}", err);
tracing::error!("Fatal: {}", err);

// ❌ Never println! in library code
println!("Provider registered: {}", name); // NO

// ✅ Impl Default where it makes sense
impl Default for ToolRegistry {
    fn default() -> Self { Self::new() }
}

// ✅ Builder pattern for complex config structs
let req = CompletionRequest::builder()
    .message("Hello!")
    .max_tokens(1024)
    .temperature(0.7)
    .build();

// ✅ Doc comments on all public items
/// Registers a tool, making it available to all agents.
///
/// # Example
/// ```rust
/// use ironclaw_tools::{ToolRegistry, datetime::DateTimeTool};
/// let mut registry = ToolRegistry::new();
/// registry.register(std::sync::Arc::new(DateTimeTool));
/// assert!(registry.get("get_datetime").is_some());
/// ```
pub fn register(&mut self, tool: Arc<dyn Tool>) { ... }
```

### File Organisation
```
crates/ironclaw-<name>/
  Cargo.toml
  src/
    lib.rs          ← re-exports only, no logic
    types.rs        ← data structures
    traits.rs       ← trait definitions
    <impl>.rs       ← implementations
  tests/
    <name>_tests.rs ← integration tests
  benches/
    <name>.rs       ← criterion benchmarks
```

### Error Handling
- Use `anyhow::Error` for application-level errors
- Use `thiserror::Error` for library-level typed errors
- Always add context: `thing.context("loading provider config")?`
- Never silently swallow errors: `.ok()` only when failure is truly ignorable

---

## Testing Guide

### Unit Tests
Place `#[cfg(test)] mod tests { ... }` at the bottom of the source file being tested.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_get_registered_tool() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(DateTimeTool));
        assert!(reg.get("get_datetime").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[tokio::test]
    async fn datetime_tool_returns_valid_iso8601() {
        let v = DateTimeTool.invoke(serde_json::json!({})).await.unwrap();
        let dt = v["datetime"].as_str().unwrap();
        assert!(dt.contains("T"), "Expected ISO8601 format");
    }
}
```

### Integration Tests
Place in `crates/<name>/tests/` — these test the public API and may use `wiremock`:

```rust
// crates/ironclaw-providers/tests/ollama_test.rs
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn ollama_complete_parses_ndjson_response() {
    let mock = MockServer::start().await;
    Mock::given(method("POST")).and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string("{"message":{"content":"hello"},"done":true}"))
        .mount(&mock).await;

    let provider = OllamaProvider::new(&mock.uri(), "llama3.2");
    let resp = provider.complete(CompletionRequest::simple("hi")).await.unwrap();
    assert_eq!(resp.text(), "hello");
}
```

### Test Categories

| Category | Location | Needs LLM? | Speed |
|----------|----------|------------|-------|
| Unit | `src/` `#[cfg(test)]` | No | < 1ms |
| Integration | `tests/` | No (wiremock) | < 100ms |
| E2E | `tests/e2e/` | Yes (Ollama) | Seconds |
| Benchmarks | `benches/` | No | Varies |

Run only fast tests: `cargo test --workspace -- --skip e2e`  
Run E2E tests: `cargo test --workspace --features e2e_tests`

---

## Adding a Provider

1. Create `crates/ironclaw-providers/src/<name>.rs`
2. Add feature flag in `crates/ironclaw-providers/Cargo.toml`
3. Implement `Provider` trait — see `OllamaProvider` as reference
4. Register in `ProviderRegistry::from_config` behind the feature flag
5. Add `[providers.<name>]` section to default `IronClawConfig`
6. Write at minimum: `name()` unit test + `wiremock` completion test
7. Add to `CHANGELOG.md` under `[Unreleased]`

```rust
// Minimum viable provider
pub struct MyProvider { api_key: String, model: String }

#[async_trait]
impl Provider for MyProvider {
    fn name(&self) -> &'static str { "myprovider" }

    async fn complete(&self, req: CompletionRequest) -> anyhow::Result<CompletionResponse> {
        // ... HTTP call to provider API ...
    }

    async fn stream(&self, req: CompletionRequest) -> anyhow::Result<BoxStream<StreamChunk>> {
        // ... SSE or NDJSON stream ...
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        // ... lightweight ping endpoint ...
    }
}
```

---

## Adding a Channel

1. Create `crates/ironclaw-channels/src/<name>.rs`
2. Add feature flag in `crates/ironclaw-channels/Cargo.toml`
3. Implement `Channel` trait
4. Register in `ChannelRunner` behind the feature flag
5. Add `[channels.<name>]` section to default config
6. Write unit test for `name()` + at least one message flow test
7. Document in `docs/channels/<name>.md`

---

## Adding a Tool

1. Create `crates/ironclaw-tools/src/<name>.rs`
2. Implement `Tool` trait
3. Export from `crates/ironclaw-tools/src/lib.rs`
4. Register in default `ToolRegistry::default_tools()`
5. Add `[tools.<name>]` config section
6. Write tests: `schema()` name/description, `invoke()` happy path, `invoke()` error path
7. Add doctest example in `invoke()` doc comment

---

## Documentation Standards

- Every `pub fn`, `pub struct`, `pub trait`, `pub enum` needs a `///` doc comment
- First line: one sentence describing what it does (not how)
- Add `# Example` section with a runnable doctest for non-trivial items
- Add `# Errors` section if the function returns `Result`
- Add `# Panics` section if the function can panic (it shouldn't in library code)

```rust
/// Creates a new completion request with a single user message.
///
/// # Example
/// ```
/// use ironclaw_core::CompletionRequest;
/// let req = CompletionRequest::simple("What is Rust?");
/// assert_eq!(req.messages.len(), 1);
/// assert!(!req.stream);
/// ```
///
/// # Errors
/// This function cannot fail. For fallible construction, use `CompletionRequest::builder()`.
pub fn simple(content: impl Into<String>) -> Self { ... }
```

---

## Release Process _(maintainers only)_

```bash
# 1. Ensure main branch is green
gh run list --limit 5

# 2. Bump all workspace versions
cargo set-version --workspace X.Y.Z

# 3. Generate changelog
git cliff --tag vX.Y.Z --prepend CHANGELOG.md

# 4. Final checks
cargo test --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
cargo deny check

# 5. Commit + tag
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: release vX.Y.Z"
git tag vX.Y.Z
git push origin main --tags

# GitHub Actions automatically:
# - Publishes all crates to crates.io
# - Builds binaries for all targets
# - Creates GitHub Release with CHANGELOG notes
# - Pushes Docker image to ghcr.io/ironclaw/ironclaw
```

---

_Questions? Open a [GitHub Discussion](https://github.com/mednabouli/ironclaw/discussions) or ask in `#contributors` on Discord.`
