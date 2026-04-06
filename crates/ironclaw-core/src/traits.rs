use std::sync::Arc;

use async_trait::async_trait;

use crate::error::{AgentError, ChannelError, HandlerError, MemoryError, ProviderError, ToolError};
use crate::types::*;

// ── Provider ──────────────────────────────────────────────────────────────
#[async_trait]
pub trait Provider: Send + Sync + 'static {
    /// Returns the provider's unique display name (e.g. `"openai"`, `"anthropic"`).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let provider: &dyn Provider = &my_provider;
    /// assert_eq!(provider.name(), "openai");
    /// ```
    fn name(&self) -> &'static str;

    /// Whether this provider supports streaming responses.
    ///
    /// Returns `true` by default.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if provider.supports_streaming() {
    ///     let stream = provider.stream(req).await?;
    /// }
    /// ```
    fn supports_streaming(&self) -> bool {
        true
    }

    /// Whether this provider supports tool/function calling.
    ///
    /// Returns `true` by default.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if provider.supports_tools() {
    ///     req = req.with_tools(tool_schemas);
    /// }
    /// ```
    fn supports_tools(&self) -> bool {
        true
    }

    /// Whether this provider supports vision (image) inputs.
    ///
    /// Returns `false` by default.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if provider.supports_vision() {
    ///     // attach image content to the request
    /// }
    /// ```
    fn supports_vision(&self) -> bool {
        false
    }

    /// Send a completion request and return the full response.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::{CompletionRequest, Provider};
    ///
    /// let req = CompletionRequest::builder("gpt-4o")
    ///     .user("Explain Rust traits in one sentence.")
    ///     .build();
    /// let resp = provider.complete(req).await?;
    /// println!("{}", resp.text());
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// use futures::StreamExt;
    /// use ironclaw_core::{CompletionRequest, Provider};
    ///
    /// let req = CompletionRequest::builder("gpt-4o")
    ///     .user("Hello!")
    ///     .build();
    /// let mut stream = provider.stream(req).await?;
    /// while let Some(chunk) = stream.next().await {
    ///     let chunk = chunk?;
    ///     print!("{}", chunk.delta);
    /// }
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// match provider.health_check().await {
    ///     Ok(()) => println!("{} is healthy", provider.name()),
    ///     Err(e) => eprintln!("{} health check failed: {e}", provider.name()),
    /// }
    /// ```
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
    /// Returns the channel's display name (e.g. `"rest"`, `"telegram"`).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let ch: &dyn Channel = &my_channel;
    /// tracing::info!("Starting channel: {}", ch.name());
    /// ```
    fn name(&self) -> &'static str;

    /// Bind the transport and begin accepting inbound messages.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use std::sync::Arc;
    /// use ironclaw_core::Channel;
    ///
    /// let handler: Arc<dyn MessageHandler> = Arc::new(my_handler);
    /// channel.start(handler).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// - [`ChannelError::StartFailed`] — the transport could not bind
    ///   (port in use, missing credentials, etc.).
    #[must_use]
    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<(), ChannelError>;

    /// Deliver an outbound message to the given target.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::{ChannelId, OutboundMessage, Channel};
    ///
    /// let target = ChannelId::Rest { session_id: session.clone() };
    /// let msg = OutboundMessage::text("Hello!");
    /// channel.send(&target, msg).await?;
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// channel.stop().await?;
    /// tracing::info!("Channel stopped");
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::{InboundMessage, MessageHandler};
    ///
    /// let msg = InboundMessage::builder()
    ///     .session_id("sess-1")
    ///     .text("Hello!")
    ///     .build();
    /// if let Some(reply) = handler.handle(msg).await? {
    ///     println!("Reply: {}", reply.as_str());
    /// }
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// use futures::StreamExt;
    /// use ironclaw_core::{InboundMessage, MessageHandler, StreamEvent};
    ///
    /// let msg = InboundMessage::builder()
    ///     .session_id("sess-1")
    ///     .text("Hello!")
    ///     .build();
    /// let mut stream = handler.handle_stream(msg).await?;
    /// while let Some(event) = stream.next().await {
    ///     match event? {
    ///         StreamEvent::TokenDelta { delta } => print!("{delta}"),
    ///         StreamEvent::Done { .. } => break,
    ///         _ => {}
    ///     }
    /// }
    /// ```
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
    /// Returns the tool's unique name used in function-calling payloads.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let tool: &dyn Tool = &my_tool;
    /// assert_eq!(tool.name(), "get_datetime");
    /// ```
    fn name(&self) -> &str;

    /// Returns a human-readable description of what the tool does.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// println!("Tool: {} — {}", tool.name(), tool.description());
    /// ```
    fn description(&self) -> &str;

    /// Returns the JSON Schema describing the tool's parameters.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::Tool;
    ///
    /// let schema = tool.schema();
    /// assert_eq!(schema.name, tool.name());
    /// assert!(schema.parameters.get("properties").is_some());
    /// ```
    fn schema(&self) -> ToolSchema;

    /// Execute the tool with the given JSON parameters.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde_json::json;
    /// use ironclaw_core::Tool;
    ///
    /// let params = json!({ "timezone": "UTC" });
    /// let result = tool.invoke(params).await?;
    /// println!("{}", result);
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::{MemoryStore, Message, Role, SessionId};
    ///
    /// let session = SessionId::new();
    /// let msg = Message::new(Role::User, "Hello, agent!");
    /// store.push(&session, msg).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not persist the message.
    /// - [`MemoryError::Serialization`] — the message could not be encoded.
    #[must_use]
    async fn push(&self, session: &SessionId, msg: Message) -> Result<(), MemoryError>;

    /// Retrieve the last `limit` messages for a session, oldest-first.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::{MemoryStore, SessionId};
    ///
    /// let session = SessionId::new();
    /// let history = store.history(&session, 50).await?;
    /// for msg in &history {
    ///     println!("[{:?}] {}", msg.role, msg.content);
    /// }
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::{MemoryStore, SessionId};
    ///
    /// let session = SessionId::new();
    /// store.clear(&session).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not delete the data.
    #[must_use]
    async fn clear(&self, session: &SessionId) -> Result<(), MemoryError>;

    /// List all session IDs that have stored messages, most-recently-active first.
    /// Returns an empty vec if the backend does not support listing.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::MemoryStore;
    ///
    /// let sessions = store.sessions().await?;
    /// println!("{} active sessions", sessions.len());
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::MemoryStore;
    ///
    /// let hits = store.search("deployment issue", 10).await?;
    /// for hit in &hits {
    ///     println!("[{}] {}", hit.session_id, hit.content);
    /// }
    /// ```
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
    /// Returns the agent's unique identifier.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::Agent;
    ///
    /// tracing::info!(agent_id = %agent.id(), "Agent ready");
    /// ```
    fn id(&self) -> &AgentId;

    /// Returns the agent's role in a multi-agent system.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::{Agent, AgentRole};
    ///
    /// match agent.role() {
    ///     AgentRole::Primary => println!("This is the primary agent"),
    ///     _ => {}
    /// }
    /// ```
    fn role(&self) -> AgentRole;

    /// Execute an agent task and return the final output.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::{Agent, AgentTask};
    ///
    /// let task = AgentTask::builder()
    ///     .instruction("Summarize the document")
    ///     .build();
    /// let output = agent.run(task).await?;
    /// println!("Result: {}", output.text);
    /// ```
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
    /// Register an agent so it can receive dispatched tasks.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use std::sync::Arc;
    /// use ironclaw_core::AgentBus;
    ///
    /// let agent: Arc<dyn Agent> = Arc::new(my_agent);
    /// bus.register(agent);
    /// ```
    fn register(&self, agent: Arc<dyn Agent>);

    /// Dispatch a task to the agent identified by `id`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::{AgentBus, AgentTask};
    ///
    /// let task = AgentTask::builder()
    ///     .instruction("Translate to French")
    ///     .build();
    /// let output = bus.dispatch(&agent_id, task).await?;
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// use serde_json::json;
    /// use ironclaw_core::VectorStore;
    ///
    /// let embedding = vec![0.1, 0.2, 0.3]; // from an embedding model
    /// store.upsert(
    ///     "doc-42",
    ///     "Rust is a systems programming language.",
    ///     &embedding,
    ///     json!({ "source": "wiki" }),
    /// ).await?;
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::VectorStore;
    ///
    /// let query_vec = vec![0.1, 0.2, 0.3]; // from an embedding model
    /// let hits = store.search(&query_vec, 5).await?;
    /// for hit in &hits {
    ///     println!("score={:.3} text={}", hit.score, hit.text);
    /// }
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::VectorStore;
    ///
    /// store.delete("doc-42").await?;
    /// ```
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not delete the entry.
    /// - [`MemoryError::NotFound`] — no entry with the given id exists.
    #[must_use]
    async fn delete(&self, id: &str) -> Result<(), MemoryError>;

    /// Return the number of stored embeddings.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use ironclaw_core::VectorStore;
    ///
    /// let n = store.count().await?;
    /// println!("{n} vectors in store");
    /// ```
    ///
    /// # Errors
    ///
    /// - [`MemoryError::Storage`] — the backend could not be queried.
    #[must_use]
    async fn count(&self) -> Result<usize, MemoryError>;
}
