pub mod error;
pub mod traits;
pub mod types;

// ── Re-exports: types ─────────────────────────────────────────────────────
pub use types::{
    AgentId, AgentOutput, AgentRole, AgentState, AgentTask, AgentTaskBuilder, BoxStream, ChannelId,
    CompletionRequest, CompletionRequestBuilder, CompletionResponse, InboundMessage,
    InboundMessageBuilder, MemoryHit, Message, OutboundContent, OutboundMessage, ResponseFormat,
    Role, SearchHit, SessionId, StopReason, StreamChunk, StreamEvent, TokenUsage, ToolCall,
    ToolCallDelta, ToolResult, ToolSchema,
};

// ── Re-exports: traits ────────────────────────────────────────────────────
pub use traits::{
    Agent, AgentBus, Channel, MemoryStore, MessageHandler, Provider, Tool, VectorStore,
};

// ── Re-exports: errors ────────────────────────────────────────────────────
pub use error::{AgentError, ChannelError, HandlerError, MemoryError, ProviderError, ToolError};
