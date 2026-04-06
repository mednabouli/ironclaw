# Migration Guide — IronClaw 0.x → 1.0

> This guide covers every breaking change introduced during the 0.x series
> and explains how to update your code for the upcoming 1.0 stable release.

---

## Quick Checklist

1. Replace `anyhow::Result` with typed error enums at trait boundaries
2. Switch from raw `String` IDs to `SessionId` / `AgentId` newtypes
3. Use builders instead of struct literals (types are `#[non_exhaustive]`)
4. Add wildcard arms to `match` on public enums
5. Use explicit re-exports from `ironclaw_core::{…}`
6. Update feature flags for provider/channel crates

---

## 1. Typed Errors Replace `anyhow::Result`

### Before (0.1.x)

```rust
use anyhow::Result;

#[async_trait]
impl Provider for MyProvider {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        // ...
    }
}
```

### After (0.2+)

```rust
use ironclaw_core::{CompletionRequest, CompletionResponse, ProviderError};

#[async_trait]
impl Provider for MyProvider {
    async fn complete(&self, req: CompletionRequest)
        -> Result<CompletionResponse, ProviderError>
    {
        // Convert internal errors:
        let resp = self.client.post(url)
            .send().await
            .map_err(|e| ProviderError::Request(e.to_string()))?;
        // ...
    }
}
```

**Error enums introduced:**

| Trait | Error type |
|-------|-----------|
| `Provider` | `ProviderError` |
| `Channel` | `ChannelError` |
| `Tool` | `ToolError` |
| `MemoryStore` / `VectorStore` | `MemoryError` |
| `Agent` / `AgentBus` | `AgentError` |
| `MessageHandler` | `HandlerError` |

Every enum includes an `Other(anyhow::Error)` variant for incremental migration —
you can wrap any `anyhow::Error` with `.into()`.

---

## 2. Newtype IDs Replace Raw Strings

### Before

```rust
store.push("session-123", msg).await?;
```

### After

```rust
use ironclaw_core::SessionId;

let session = SessionId::new("session-123");
store.push(&session, msg).await?;
```

Similarly, `AgentId` replaces raw strings for agent identification:

```rust
use ironclaw_core::AgentId;

let id = AgentId::new("summarizer-01");
bus.dispatch(&id, task).await?;
```

---

## 3. `#[non_exhaustive]` on All Public Types

All public structs and enums in `ironclaw-core` are now `#[non_exhaustive]`.
This means:

### Structs — Use Builders, Not Struct Literals

```rust
// ❌ Will not compile — struct is non-exhaustive
let req = CompletionRequest {
    model: "gpt-4o".into(),
    messages: vec![],
    ..Default::default()
};

// ✅ Use the builder
let req = CompletionRequest::builder("gpt-4o")
    .system("You are helpful.")
    .user("Hello!")
    .build();
```

Builders are available for:
- `CompletionRequest::builder(model)`
- `AgentTask::builder()`
- `InboundMessage::builder()`

### Enums — Add Wildcard Arms

```rust
// ❌ May fail when new variants are added
match role {
    Role::System => { /* ... */ }
    Role::User => { /* ... */ }
    Role::Assistant => { /* ... */ }
    Role::Tool => { /* ... */ }
}

// ✅ Include a wildcard
match role {
    Role::System => { /* ... */ }
    Role::User => { /* ... */ }
    Role::Assistant => { /* ... */ }
    Role::Tool => { /* ... */ }
    _ => { /* handle future variants */ }
}
```

---

## 4. Explicit Re-exports

All public types are now re-exported from the `ironclaw_core` crate root.
Import directly instead of reaching into sub-modules:

```rust
// ❌ Fragile — module paths may change
use ironclaw_core::types::CompletionRequest;
use ironclaw_core::traits::Provider;
use ironclaw_core::error::ProviderError;

// ✅ Stable — re-exports from crate root
use ironclaw_core::{CompletionRequest, Provider, ProviderError};
```

---

## 5. Feature Flags

Provider and channel crates now gate all implementations behind feature flags.
Nothing compiles by default that requires an external API key.

### Providers (`ironclaw-providers`)

```toml
[dependencies]
ironclaw-providers = { version = "0.2", features = ["ollama", "anthropic"] }
# Or enable all:
ironclaw-providers = { version = "0.2", features = ["all"] }
```

Available features: `ollama`, `anthropic`, `openai`, `groq`, `openrouter`,
`mistral`, `together`, `cohere`, `all`.

### Channels (`ironclaw-channels`)

```toml
[dependencies]
ironclaw-channels = { version = "0.2", features = ["rest", "cli"] }
```

Available features: `rest`, `cli`, `telegram`, `discord`, `slack`,
`websocket`, `webhook`, `matrix`.

---

## 6. MSRV Bump

The Minimum Supported Rust Version is now **1.86**. Update your toolchain:

```bash
rustup update stable
```

---

## 7. `#[must_use]` on Async Trait Methods

All fallible async trait methods are now annotated `#[must_use]`.
If you were silently discarding `Result`s, the compiler will now warn:

```rust
// ⚠️ Warning — unused Result
provider.health_check().await;

// ✅ Handle the result
provider.health_check().await?;
```

---

## Need Help?

- Open a [GitHub Discussion](https://github.com/mednabouli/ironclaw/discussions)
- File an issue for migration problems not covered here
- See `docs/STABILITY.md` for the full API stability policy
