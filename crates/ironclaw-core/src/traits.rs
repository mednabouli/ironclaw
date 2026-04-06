
use async_trait::async_trait;
use std::sync::Arc;
use crate::types::*;

// ── Provider ──────────────────────────────────────────────────────────────
#[async_trait]
pub trait Provider: Send + Sync + 'static {
    fn name(&self)               -> &'static str;
    fn supports_streaming(&self) -> bool { true }
    fn supports_tools(&self)     -> bool { true }
    fn supports_vision(&self)    -> bool { false }

    async fn complete(&self, req: CompletionRequest)
        -> anyhow::Result<CompletionResponse>;

    async fn stream(&self, req: CompletionRequest)
        -> anyhow::Result<BoxStream<StreamChunk>>;

    async fn health_check(&self) -> anyhow::Result<()>;
}

// ── Channel ───────────────────────────────────────────────────────────────
#[async_trait]
pub trait Channel: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    async fn start(
        &self,
        handler: Arc<dyn MessageHandler>,
    ) -> anyhow::Result<()>;

    async fn send(
        &self,
        to: &ChannelId,
        message: OutboundMessage,
    ) -> anyhow::Result<()>;

    async fn stop(&self) -> anyhow::Result<()>;
}

// ── MessageHandler ────────────────────────────────────────────────────────
#[async_trait]
pub trait MessageHandler: Send + Sync + 'static {
    async fn handle(
        &self,
        msg: InboundMessage,
    ) -> anyhow::Result<Option<OutboundMessage>>;
}

// ── Tool ──────────────────────────────────────────────────────────────────
#[async_trait]
pub trait Tool: Send + Sync + 'static {
    fn name(&self)        -> &str;
    fn description(&self) -> &str;
    fn schema(&self)      -> ToolSchema;

    async fn invoke(
        &self,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value>;
}

// ── MemoryStore ───────────────────────────────────────────────────────────
#[async_trait]
pub trait MemoryStore: Send + Sync + 'static {
    async fn push(
        &self,
        session: &SessionId,
        msg: Message,
    ) -> anyhow::Result<()>;

    async fn history(
        &self,
        session: &SessionId,
        limit:   usize,
    ) -> anyhow::Result<Vec<Message>>;

    async fn clear(&self, session: &SessionId) -> anyhow::Result<()>;
}

// ── Agent ─────────────────────────────────────────────────────────────────
#[async_trait]
pub trait Agent: Send + Sync + 'static {
    fn id(&self)   -> &AgentId;
    fn role(&self) -> AgentRole;

    async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput>;
}

// ── AgentBus ──────────────────────────────────────────────────────────────
#[async_trait]
pub trait AgentBus: Send + Sync + 'static {
    fn register(&self, agent: Arc<dyn Agent>);
    async fn dispatch(&self, id: &AgentId, task: AgentTask) -> anyhow::Result<AgentOutput>;
}
