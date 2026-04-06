use std::sync::Arc;

use async_trait::async_trait;

use crate::error::{AgentError, ChannelError, HandlerError, MemoryError, ProviderError, ToolError};
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

    /// Send a completion request and return the full response.
    ///
    /// # Errors
    ///
    /// - [`ProviderError::Request`] — network or HTTP transport failure.
    /// - [`ProviderError::Auth`] — invalid or expired API key.
    /// - [`ProviderError::RateLimit`] — provider throttled the request;
    ///   inspect `retry_after_ms` for the suggested back-off.
    /// - [`ProviderError::ModelNotFound`] — the requested model is not
    ///   available on this provider.
    /// - [`ProviderError::InvalidResponse`] — the response body could not
    ///   be parsed into [`CompletionResponse`].
    #[must_use]
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, ProviderError>;

    /// Send a completion request and return a streaming response.
    ///
    /// # Errors
    ///
    /// Returns the same error variants as [`Provider::complete`].
    /// Additionally returns [`ProviderError::StreamTerminated`] if the
    /// connection drops before the final chunk is received.
    #[must_use]
    async fn stream(&self, req: CompletionRequest)
        -> Result<BoxStream<StreamChunk>, ProviderError>;

    /// Verify the provider is reachable and the credentials are valid.
    ///
    /// # Errors
    ///
    /// - [`ProviderError::Request`] — provider endpoint unreachable.
    /// - [`ProviderError::Auth`] — credentials rejected.
    #[must_use]
    async fn health_check(&self) -> Result<(), ProviderError>;
}

// ── Channel ───────────────────────────────────────────────────────────────
#[async_trait]
pub trait Channel: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    /// Bind the transport and begin accepting inbound messages.
    ///
    /// # Errors
    ///
    /// - [`ChannelError::StartFailed`] — the transport could not bind
    ///   (port in use, missing credentials, etc.).
    #[must_use]
    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<(), ChannelError>;

    /// Deliver an outbound message to the given target.
    ///
    /// # Errors
    ///
    /// - [`ChannelError::SendFailed`] — delivery failed (network, invalid
    ///   target, serialisation error).
    /// - [`ChannelError::NotRunning`] — the channel has not been started
    ///   or has already been stopped.
    #[must_use]
    async fn send(&self, to: &ChannelId, message: OutboundMessage) -> Result<(), ChannelError>;

    /// Gracefully shut down the channel transport.
    ///
    /// # Errors
    ///
    /// - [`ChannelError::NotRunning`] — the channel was never started.
    #[must_use]
    async fn stop(&self) -> Result<(), ChannelError>;
}

// ── MessageHandler ────────────────────────────────────────────────────────
#[async_trait]
pub trait MessageHandler: Send + Sync + 'static {
    /// Handle an inbound message and return a complete response.
    ///
    /// Returns `Ok(None)` when the handler intentionally produces no reply
    /// (e.g. a middleware that filters the message).
    ///
    /// # Errors
    ///
    /// - [`HandlerError::Agent`] — the underlying agent failed.
    /// - [`HandlerError::Channel`] — a channel-level failure during processing.
    #[must_use]
    async fn handle(&self, msg: InboundMessage) -> Result<Option<OutboundMessage>, HandlerError>;

    /// Handle an inbound message and return a stream of events.
    ///
    /// The default implementation wraps [`MessageHandler::handle`] into a
    /// single `TokenDelta` + `Done` sequence.
    ///
    /// # Errors
    ///
    /// Same as [`MessageHandler::handle`].
    #[must_use]
    async fn handle_stream(
        &self,
        msg: InboundMessage,
    ) -> Result<BoxStream<StreamEvent>, HandlerError> {
        let result = self.handle(msg).await?;
        let events: Vec<anyhow::Result<StreamEvent>> = match result {
            Some(out) => vec![
                Ok(StreamEvent::TokenDelta {
                    delta: out.as_str().to_string(),
                }),
                Ok(StreamEvent::Done { usage: None }),
            ],
            None => vec![Ok(StreamEvent::Done { usage: None })],
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

// ── Tool ──────────────────────────────────────────────────────────────────
#[async_trait]
pub trait Tool: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> ToolSchema;

    /// Execute the tool with the given JSON parameters.
    ///
    /// # Errors
    ///
    /// - [`ToolError::InvalidParams`] — the params do not match the
    ///   expected schema.
    /// - [`ToolError::ExecutionFailed`] — the tool ran but encountered
    ///   an error (e.g. network failure, missing file).
    /// - [`ToolError::Timeout`] — the tool did not complete within its
    ///   configured time limit.
    #[must_use]
    async fn invoke(&self, params: serde_json::Value) -> Result<serde_json::Value, ToolError>;
}

// ── MemoryStore ───────────────────────────────────────────────────────────
#[async_trait]
pub trait MemoryStore: Send + Sync + 'static {
    /// Append a message to the session history.
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not persist the message.
    /// - [`MemoryError::Serialization`] — the message could not be encoded.
    #[must_use]
    async fn push(&self, session: &SessionId, msg: Message) -> Result<(), MemoryError>;

    /// Retrieve the last `limit` messages for a session, oldest-first.
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not be read.
    /// - [`MemoryError::NotFound`] — the session does not exist (optional;
    ///   implementations may return an empty vec instead).
    #[must_use]
    async fn history(&self, session: &SessionId, limit: usize)
        -> Result<Vec<Message>, MemoryError>;

    /// Delete all messages for a session.
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not delete the data.
    #[must_use]
    async fn clear(&self, session: &SessionId) -> Result<(), MemoryError>;

    /// List all session IDs that have stored messages, most-recently-active first.
    /// Returns an empty vec if the backend does not support listing.
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not be queried.
    #[must_use]
    async fn sessions(&self) -> Result<Vec<SessionId>, MemoryError> {
        Ok(vec![])
    }

    /// Full-text search across all stored messages.
    /// Returns up to `limit` matching [`SearchHit`]s, most-recent first.
    /// Returns an empty vec if the backend does not support search.
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not execute the query.
    #[must_use]
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, MemoryError> {
        let _ = (query, limit);
        Ok(vec![])
    }
}

// ── Agent ─────────────────────────────────────────────────────────────────
#[async_trait]
pub trait Agent: Send + Sync + 'static {
    fn id(&self) -> &AgentId;
    fn role(&self) -> AgentRole;

    /// Execute an agent task and return the final output.
    ///
    /// # Errors
    ///
    /// - [`AgentError::Provider`] — the underlying LLM provider failed.
    /// - [`AgentError::Tool`] — a tool invocation during the run failed.
    /// - [`AgentError::BudgetExceeded`] — the agent hit its iteration or
    ///   token budget before completing.
    /// - [`AgentError::NotFound`] — a delegated sub-agent could not be found.
    #[must_use]
    async fn run(&self, task: AgentTask) -> Result<AgentOutput, AgentError>;
}

// ── AgentBus ──────────────────────────────────────────────────────────────
#[async_trait]
pub trait AgentBus: Send + Sync + 'static {
    fn register(&self, agent: Arc<dyn Agent>);

    /// Dispatch a task to the agent identified by `id`.
    ///
    /// # Errors
    ///
    /// - [`AgentError::NotFound`] — no agent registered with the given id.
    /// - Any error the target agent's [`Agent::run`] may return.
    #[must_use]
    async fn dispatch(&self, id: &AgentId, task: AgentTask) -> Result<AgentOutput, AgentError>;
}

// ── VectorStore (RAG) ─────────────────────────────────────────────────────
/// Trait for storing and retrieving text embeddings for RAG (Retrieval-Augmented
/// Generation). Implementations receive pre-computed embedding vectors and
/// perform nearest-neighbour search using cosine similarity.
#[async_trait]
pub trait VectorStore: Send + Sync + 'static {
    /// Store a text chunk and its embedding vector.
    ///
    /// `id` is a caller-chosen unique identifier. If the id already exists
    /// the entry is replaced.
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not persist the entry.
    /// - [`MemoryError::Serialization`] — the metadata could not be encoded.
    #[must_use]
    async fn upsert(
        &self,
        id: &str,
        text: &str,
        embedding: &[f32],
        metadata: serde_json::Value,
    ) -> Result<(), MemoryError>;

    /// Find the `limit` nearest neighbours to `query_embedding`.
    ///
    /// Returns results sorted by descending cosine similarity score.
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not execute the search.
    #[must_use]
    async fn search(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<MemoryHit>, MemoryError>;

    /// Delete an entry by id.
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not delete the entry.
    /// - [`MemoryError::NotFound`] — no entry with the given id exists.
    #[must_use]
    async fn delete(&self, id: &str) -> Result<(), MemoryError>;

    /// Return the number of stored embeddings.
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not be queried.
    #[must_use]
    async fn count(&self) -> Result<usize, MemoryError>;
}
