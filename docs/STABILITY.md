# API Stability Policy

> Describes what is stable, what is experimental, and what guarantees IronClaw
> provides between releases.
>
> See also: [`VERSIONING.md`](VERSIONING.md) for the full SemVer policy and
> deprecation process, [`MIGRATION.md`](MIGRATION.md) for upgrading from 0.x.

---

## Versioning

IronClaw follows [Semantic Versioning 2.0.0](https://semver.org/).

| Version range | Meaning |
|---------------|---------|
| `0.x.y`       | Pre-1.0 development. Minor bumps **may** contain breaking changes. Patch bumps are backwards-compatible bug fixes. |
| `1.0.0+`      | Stable. Breaking changes require a major version bump. |

The current version is **0.x** — the API may still change between minor releases,
but we aim to minimize churn and document every break in `CHANGELOG.md`.

---

## Stability Tiers

### Tier 1 — Stable

These items will not change in breaking ways without a major version bump
(or, during 0.x, without a minor bump and a `CHANGELOG.md` entry):

- **Core traits:** `Provider`, `Channel`, `MessageHandler`, `Tool`,
  `MemoryStore`, `Agent`, `AgentBus`, `VectorStore`
- **Core types:** `Message`, `CompletionRequest`, `CompletionResponse`,
  `AgentTask`, `AgentOutput`, `TokenUsage`, `ToolSchema`, `ToolCall`,
  `ToolResult`, `StreamChunk`, `StreamEvent`, `InboundMessage`,
  `OutboundMessage`, `SessionId`, `AgentId`
- **Core enums:** `Role`, `StopReason`, `ResponseFormat`, `ChannelId`,
  `AgentRole`, `AgentState`, `OutboundContent`
- **Error types:** `ProviderError`, `ChannelError`, `ToolError`, `MemoryError`,
  `AgentError`, `HandlerError`
- **Builders:** `CompletionRequestBuilder`, `AgentTaskBuilder`,
  `InboundMessageBuilder`

### Tier 2 — Unstable (may change without notice)

- `ironclaw-wasm` — WASM plugin system (wasmtime integration not yet wired)
- Feature-flag gated channel implementations (`discord`, `telegram`,
  `slack`, `matrix`, `webhook`, `websocket`)
- Provider-specific behaviour beyond the `Provider` trait (e.g. SSE internals)
- `AgentContext` fields and construction — may gain new fields or
  change internal wiring

### Tier 3 — Internal (no stability guarantee)

- Anything inside a `pub(crate)` or private module
- Benchmark harnesses (`benches/`)
- Integration tests (`tests/`)
- CLI commands and flags — the CLI is a consumer of the library, not a
  public API

---

## Non-Exhaustive Policy

All public enums and most public structs in `ironclaw-core` are marked
`#[non_exhaustive]`. This means:

- **Enums:** Downstream code must include a wildcard arm (`_ => …`) when
  matching. New variants may be added in minor releases.
- **Structs:** Downstream code cannot construct them with struct literal
  syntax. Use the provided constructors or builders instead.

---

## MSRV (Minimum Supported Rust Version)

The workspace declares `rust-version = "1.86"` in the root `Cargo.toml`.
MSRV bumps are treated as **minor** breaking changes during 0.x and will
be noted in `CHANGELOG.md`.

---

## Deprecation Process

1. Item is marked `#[deprecated(since = "X.Y.Z", note = "use Foo instead")]`.
2. A compiler warning is emitted for at least **one minor release**.
3. The item is removed in the next minor (0.x) or major (1.x+) release.

---

## How to Check for Breaks

```bash
# Install semver-checks
cargo install cargo-semver-checks

# Compare against the last published version
cargo semver-checks check-release
```

---

## What Counts as a Breaking Change

The following changes are **always** breaking and require a major bump
(or a minor bump + `CHANGELOG.md` entry during 0.x):

- Removing a public type, trait, or function
- Changing the signature of a trait method
- Removing a variant from a `#[non_exhaustive]` enum *(adding is non-breaking)*
- Removing a public field from a struct
- Changing a type's `Serialize`/`Deserialize` wire format
- Raising the MSRV

The following are **non-breaking** changes:

- Adding a new variant to a `#[non_exhaustive]` enum
- Adding a new field to a `#[non_exhaustive]` struct (if it has a default)
- Adding a new trait method with a default implementation
- Adding a new module or re-export
- Relaxing a trait bound
- Bug fixes that change observable but undocumented behaviour
