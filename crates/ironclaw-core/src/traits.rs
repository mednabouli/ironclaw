use std::sync::Arc;

use async_trait::async_trait;

use crate::types::*;

// ── Provider ──────────────────────────────────────────────────────────────
#[async_trait]
pub trait Provider: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn supports_streaming(&self) -> bool {
        true
    }
    fn supports_tools(&self) -> bool {
        true
    }
    fn supports_vision(&self) -> bool {
        false
    }

    async fn complete(&self, req: CompletionRequest) -> anyhow::Result<CompletionResponse>;

    async fn stream(&self, req: CompletionRequest) -> anyhow::Result<BoxStream<StreamChunk>>;

    async fn health_check(&self) -> anyhow::Result<()>;
}

// ── Channel ───────────────────────────────────────────────────────────────
#[async_trait]
pub trait Channel: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> anyhow::Result<()>;

    async fn send(&self, to: &ChannelId, message: OutboundMessage) -> anyhow::Result<()>;

    async fn stop(&self) -> anyhow::Result<()>;
}

// ── MessageHandler ────────────────────────────────────────────────────────
#[async_trait]
pub trait MessageHandler: Send + Sync + 'static {
    async fn handle(&self, msg: InboundMessage) -> anyhow::Result<Option<OutboundMessage>>;
}

// ── Tool ──────────────────────────────────────────────────────────────────
#[async_trait]
pub trait Tool: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> ToolSchema;

    async fn invoke(&self, params: serde_json::Value) -> anyhow::Result<serde_json::Value>;
}

// ── MemoryStore ───────────────────────────────────────────────────────────
#[async_trait]
pub trait MemoryStore: Send + Sync + 'static {
    /// Append a message to the session history.
    async fn push(&self, session: &SessionId, msg: Message) -> anyhow::Result<()>;

    /// Retrieve the last `limit` messages for a session, oldest-first.
    async fn history(&self, session: &SessionId, limit: usize) -> anyhow::Result<Vec<Message>>;

    /// Delete all messages for a session.
    async fn clear(&self, session: &SessionId) -> anyhow::Result<()>;

    /// List all session IDs that have stored messages, most-recently-active first.
    /// Returns an empty vec if the backend does not support listing.
    async fn sessions(&self) -> anyhow::Result<Vec<SessionId>> {
        Ok(vec![])
    }

    /// Full-text search across all stored messages.
    /// Returns up to `limit` matching `SearchHit`s, most-recent first.
    /// Returns an empty vec if the backend does not support search.
    async fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchHit>> {
        let _ = (query, limit);
        Ok(vec![])
    }
}

// ── Agent ─────────────────────────────────────────────────────────────────
#[async_trait]
pub trait Agent: Send + Sync + 'static {
    fn id(&self) -> &AgentId;
    fn role(&self) -> AgentRole;

    async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput>;
}

// ── AgentBus ──────────────────────────────────────────────────────────────
#[async_trait]
pub trait AgentBus: Send + Sync + 'static {
    fn register(&self, agent: Arc<dyn Agent>);
    async fn dispatch(&self, id: &AgentId, task: AgentTask) -> anyhow::Result<AgentOutput>;
}
