use std::pin::Pin;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Stream type alias ──────────────────────────────────────────────────────
pub type BoxStream<T> = Pin<Box<dyn futures::Stream<Item = anyhow::Result<T>> + Send + 'static>>;

// ── Session / Agent IDs ────────────────────────────────────────────────────

/// Opaque identifier for a conversation session.
///
/// Wraps a `String` to prevent accidental misuse with other string-typed IDs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Create a new session ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// View the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl AsRef<str> for SessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for SessionId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

/// Opaque identifier for an agent.
///
/// Wraps a `String` to prevent accidental misuse with other string-typed IDs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    /// Create a new agent ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// View the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for AgentId {
    fn from(s: String) -> Self {
        Self(s.to_owned())
    }
}

impl From<&str> for AgentId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl AsRef<str> for AgentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for AgentId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

// ── Message roles ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

// ── Tool calling ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

impl ToolCall {
    /// Create a new tool call.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ToolResult {
    pub call_id: String,
    pub content: serde_json::Value,
}

impl ToolResult {
    /// Create a new tool result.
    pub fn new(call_id: impl Into<String>, content: serde_json::Value) -> Self {
        Self {
            call_id: call_id.into(),
            content,
        }
    }
}

// ── Core Message ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Message {
    pub id: Uuid,
    pub role: Role,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_result: Option<ToolResult>,
    pub timestamp: DateTime<Utc>,
}

impl Message {
    /// Create a message with all fields specified (useful for storage round-trips).
    pub fn with_all(
        id: Uuid,
        role: Role,
        content: impl Into<String>,
        tool_calls: Vec<ToolCall>,
        tool_result: Option<ToolResult>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            role,
            content: content.into(),
            tool_calls,
            tool_result,
            timestamp,
        }
    }

    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::System,
            content: content.into(),
            tool_calls: vec![],
            tool_result: None,
            timestamp: Utc::now(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::User,
            content: content.into(),
            tool_calls: vec![],
            tool_result: None,
            timestamp: Utc::now(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            content: content.into(),
            tool_calls: vec![],
            tool_result: None,
            timestamp: Utc::now(),
        }
    }
    pub fn tool_result(call_id: impl Into<String>, content: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::Tool,
            content: String::new(),
            tool_calls: vec![],
            tool_result: Some(ToolResult {
                call_id: call_id.into(),
                content,
            }),
            timestamp: Utc::now(),
        }
    }
}

// ── Token usage ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl TokenUsage {
    /// Create a new token usage record.
    pub fn new(prompt_tokens: u32, completion_tokens: u32, total_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        }
    }
}

// ── Stop reason ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
}

// ── Response format ────────────────────────────────────────────────────────
/// Controls the output format of the provider response.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ResponseFormat {
    /// Default text output.
    #[default]
    Text,
    /// Request JSON-mode output (provider will return a valid JSON object).
    JsonObject,
}

// ── Completion request / response ─────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSchema>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stream: bool,
    pub model: Option<String>,
    /// Response format — set to `JsonObject` to enable structured JSON output.
    #[serde(default)]
    pub response_format: ResponseFormat,
}

impl CompletionRequest {
    /// Create a minimal request with a single user message.
    pub fn simple(content: impl Into<String>) -> Self {
        Self {
            messages: vec![Message::user(content)],
            tools: vec![],
            max_tokens: Some(4096),
            temperature: Some(0.7),
            stream: false,
            model: None,
            response_format: ResponseFormat::default(),
        }
    }

    /// Start building a request from a list of messages.
    pub fn builder(messages: Vec<Message>) -> CompletionRequestBuilder {
        CompletionRequestBuilder {
            messages,
            tools: vec![],
            max_tokens: None,
            temperature: None,
            stream: false,
            model: None,
            response_format: ResponseFormat::default(),
        }
    }
}

/// Builder for [`CompletionRequest`].
#[derive(Debug, Clone)]
pub struct CompletionRequestBuilder {
    messages: Vec<Message>,
    tools: Vec<ToolSchema>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    stream: bool,
    model: Option<String>,
    response_format: ResponseFormat,
}

impl CompletionRequestBuilder {
    /// Set the available tool schemas.
    pub fn tools(mut self, tools: Vec<ToolSchema>) -> Self {
        self.tools = tools;
        self
    }

    /// Set the maximum number of tokens to generate.
    pub fn max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = Some(n);
        self
    }

    /// Set the sampling temperature.
    pub fn temperature(mut self, t: f32) -> Self {
        self.temperature = Some(t);
        self
    }

    /// Enable or disable streaming.
    pub fn stream(mut self, enabled: bool) -> Self {
        self.stream = enabled;
        self
    }

    /// Override the model name.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the response format.
    pub fn response_format(mut self, fmt: ResponseFormat) -> Self {
        self.response_format = fmt;
        self
    }

    /// Consume the builder and produce a [`CompletionRequest`].
    pub fn build(self) -> CompletionRequest {
        CompletionRequest {
            messages: self.messages,
            tools: self.tools,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: self.stream,
            model: self.model,
            response_format: self.response_format,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompletionResponse {
    pub message: Message,
    pub stop_reason: StopReason,
    pub usage: TokenUsage,
    pub model: String,
    pub latency_ms: u64,
}

impl CompletionResponse {
    /// Create a new completion response.
    pub fn new(
        message: Message,
        stop_reason: StopReason,
        usage: TokenUsage,
        model: impl Into<String>,
        latency_ms: u64,
    ) -> Self {
        Self {
            message,
            stop_reason,
            usage,
            model: model.into(),
            latency_ms,
        }
    }

    /// Get the text content of the response.
    pub fn text(&self) -> &str {
        &self.message.content
    }

    /// Check whether the response contains tool calls.
    pub fn has_tool_calls(&self) -> bool {
        !self.message.tool_calls.is_empty()
    }
}

// ── Streaming tool call delta ─────────────────────────────────────────────
/// An incremental piece of a tool call received during streaming.
/// Providers emit these across multiple chunks; the consumer accumulates
/// them to reconstruct a full `ToolCall`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ToolCallDelta {
    /// Index of the tool call in the current response (providers may
    /// issue multiple tool calls in parallel).
    pub index: usize,
    /// Set on the first delta for this tool call; `None` on subsequent deltas.
    pub id: Option<String>,
    /// Set on the first delta for this tool call; `None` on subsequent deltas.
    pub name: Option<String>,
    /// Incremental JSON fragment of the arguments string.
    pub arguments_delta: String,
}

impl ToolCallDelta {
    /// Create a new tool call delta.
    pub fn new(index: usize, arguments_delta: impl Into<String>) -> Self {
        Self {
            index,
            id: None,
            name: None,
            arguments_delta: arguments_delta.into(),
        }
    }

    /// Create the first delta for a tool call (includes id and name).
    pub fn first(
        index: usize,
        id: impl Into<String>,
        name: impl Into<String>,
        arguments_delta: impl Into<String>,
    ) -> Self {
        Self {
            index,
            id: Some(id.into()),
            name: Some(name.into()),
            arguments_delta: arguments_delta.into(),
        }
    }
}

// ── Stream chunk ──────────────────────────────────────────────────────────
/// A single chunk from a provider's streaming response.
/// Contains text deltas, optional tool call deltas, and a stop reason
/// on the final chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StreamChunk {
    /// Incremental text content.
    pub delta: String,
    /// Whether this is the final chunk in the stream.
    pub done: bool,
    /// Incremental tool call information (empty when no tool calls).
    #[serde(default)]
    pub tool_calls: Vec<ToolCallDelta>,
    /// Set on the final chunk to indicate why generation stopped.
    #[serde(default)]
    pub stop_reason: Option<StopReason>,
}

impl StreamChunk {
    /// Create a stream chunk with all fields specified.
    pub fn new(
        delta: impl Into<String>,
        done: bool,
        tool_calls: Vec<ToolCallDelta>,
        stop_reason: Option<StopReason>,
    ) -> Self {
        Self {
            delta: delta.into(),
            done,
            tool_calls,
            stop_reason,
        }
    }

    /// Create a text delta chunk.
    pub fn delta(text: impl Into<String>) -> Self {
        Self {
            delta: text.into(),
            done: false,
            tool_calls: vec![],
            stop_reason: None,
        }
    }

    /// Create the final chunk.
    pub fn done(stop_reason: StopReason) -> Self {
        Self {
            delta: String::new(),
            done: true,
            tool_calls: vec![],
            stop_reason: Some(stop_reason),
        }
    }

    /// Create a chunk with tool call deltas.
    pub fn with_tool_calls(tool_calls: Vec<ToolCallDelta>) -> Self {
        Self {
            delta: String::new(),
            done: false,
            tool_calls,
            stop_reason: None,
        }
    }
}

// ── Stream events (SSE-level) ─────────────────────────────────────────────
/// High-level events emitted during a streaming agent interaction.
/// These are sent to clients over SSE and represent the full lifecycle
/// of a streamed response including tool call execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum StreamEvent {
    /// An incremental text token from the model.
    TokenDelta {
        /// The text fragment.
        delta: String,
    },
    /// A tool call is about to be executed.
    ToolCallStart {
        /// Unique ID for this tool call.
        id: String,
        /// Name of the tool being invoked.
        name: String,
        /// The parsed arguments for the tool.
        arguments: serde_json::Value,
    },
    /// A tool call has finished executing.
    ToolCallEnd {
        /// The tool call ID (matches the preceding `ToolCallStart`).
        id: String,
        /// The result returned by the tool.
        result: serde_json::Value,
    },
    /// The stream has completed successfully.
    Done {
        /// Token usage for the full interaction, if available.
        /// Streaming completions may not report usage.
        usage: Option<TokenUsage>,
    },
    /// An error occurred during streaming.
    Error {
        /// Human-readable error description.
        message: String,
    },
}

// ── Tool schema ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolSchema {
    /// Create a new tool schema.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

// ── Memory hit (RAG vector search) ───────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MemoryHit {
    pub id: String,
    pub text: String,
    pub score: f32,
    pub metadata: serde_json::Value,
}

impl MemoryHit {
    /// Create a new memory hit.
    pub fn new(
        id: impl Into<String>,
        text: impl Into<String>,
        score: f32,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            score,
            metadata,
        }
    }
}

// ── Conversation search result ────────────────────────────────────────────
/// A single matching message returned by `MemoryStore::search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SearchHit {
    /// The session this message belongs to.
    pub session_id: SessionId,
    /// The matching message.
    pub message: Message,
}

impl SearchHit {
    /// Create a new search hit.
    pub fn new(session_id: impl Into<SessionId>, message: Message) -> Self {
        Self {
            session_id: session_id.into(),
            message,
        }
    }
}

// ── Agent types ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AgentTask {
    pub id: Uuid,
    pub instruction: String,
    pub context: Vec<Message>,
    pub tool_allowlist: Option<Vec<String>>,
    pub max_tokens: Option<u32>,
}

impl AgentTask {
    /// Create a task with just an instruction.
    pub fn new(instruction: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            instruction: instruction.into(),
            context: vec![],
            tool_allowlist: None,
            max_tokens: None,
        }
    }

    /// Start building a task from an instruction string.
    pub fn builder(instruction: impl Into<String>) -> AgentTaskBuilder {
        AgentTaskBuilder {
            id: Uuid::new_v4(),
            instruction: instruction.into(),
            context: vec![],
            tool_allowlist: None,
            max_tokens: None,
        }
    }
}

/// Builder for [`AgentTask`].
#[derive(Debug, Clone)]
pub struct AgentTaskBuilder {
    id: Uuid,
    instruction: String,
    context: Vec<Message>,
    tool_allowlist: Option<Vec<String>>,
    max_tokens: Option<u32>,
}

impl AgentTaskBuilder {
    /// Override the auto-generated task ID.
    pub fn id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Provide conversation context for the task.
    pub fn context(mut self, context: Vec<Message>) -> Self {
        self.context = context;
        self
    }

    /// Restrict which tools the agent may use.
    pub fn tool_allowlist(mut self, tools: Vec<String>) -> Self {
        self.tool_allowlist = Some(tools);
        self
    }

    /// Set a token budget for the task.
    pub fn max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = Some(n);
        self
    }

    /// Consume the builder and produce an [`AgentTask`].
    pub fn build(self) -> AgentTask {
        AgentTask {
            id: self.id,
            instruction: self.instruction,
            context: self.context,
            tool_allowlist: self.tool_allowlist,
            max_tokens: self.max_tokens,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AgentOutput {
    pub task_id: Uuid,
    pub agent_id: AgentId,
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub approved: bool,
    pub usage: TokenUsage,
}

impl AgentOutput {
    /// Create a new agent output.
    pub fn new(task_id: Uuid, agent_id: impl Into<AgentId>, text: impl Into<String>) -> Self {
        Self {
            task_id,
            agent_id: agent_id.into(),
            text: text.into(),
            tool_calls: vec![],
            approved: false,
            usage: TokenUsage::default(),
        }
    }

    /// Set tool calls on the output.
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = tool_calls;
        self
    }

    /// Mark the output as approved.
    pub fn with_approved(mut self, approved: bool) -> Self {
        self.approved = approved;
        self
    }

    /// Set token usage.
    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = usage;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AgentRole {
    Orchestrator,
    Worker,
    Router,
    Critic,
    Planner,
}

/// State machine for agent lifecycle tracking.
///
/// Transitions: `Idle → Running → Waiting → Running → Done | Failed`
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum AgentState {
    /// Agent is idle, waiting for a task.
    #[default]
    Idle,
    /// Agent is actively processing a task.
    Running,
    /// Agent is waiting on an external resource (tool call, sub-agent, etc.).
    Waiting,
    /// Agent completed the task successfully.
    Done,
    /// Agent failed with an error message.
    Failed(String),
}

impl std::fmt::Display for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Waiting => write!(f, "Waiting"),
            Self::Done => write!(f, "Done"),
            Self::Failed(msg) => write!(f, "Failed: {msg}"),
        }
    }
}

// ── Channel / Message types ───────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "id")]
#[non_exhaustive]
pub enum ChannelId {
    Telegram(i64),
    Discord(String),
    Slack(String),
    Rest(String),
    WebSocket(String),
    Webhook(String),
    Matrix(String),
    Cli,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InboundMessage {
    pub id: String,
    pub channel: ChannelId,
    pub session_id: SessionId,
    pub content: String,
    pub author: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl InboundMessage {
    /// Convenience constructor for CLI input.
    pub fn cli(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            channel: ChannelId::Cli,
            session_id: "cli-default".into(),
            content: content.into(),
            author: Some("user".into()),
            timestamp: Utc::now(),
        }
    }

    /// Start building an inbound message for a given channel.
    pub fn builder(channel: ChannelId, content: impl Into<String>) -> InboundMessageBuilder {
        InboundMessageBuilder {
            id: Uuid::new_v4().to_string(),
            channel,
            session_id: "default".into(),
            content: content.into(),
            author: None,
            timestamp: Utc::now(),
        }
    }
}

/// Builder for [`InboundMessage`].
#[derive(Debug, Clone)]
pub struct InboundMessageBuilder {
    id: String,
    channel: ChannelId,
    session_id: SessionId,
    content: String,
    author: Option<String>,
    timestamp: DateTime<Utc>,
}

impl InboundMessageBuilder {
    /// Override the auto-generated message ID.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Set the session ID.
    pub fn session_id(mut self, session_id: impl Into<SessionId>) -> Self {
        self.session_id = session_id.into();
        self
    }

    /// Set the message author.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Override the timestamp.
    pub fn timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = ts;
        self
    }

    /// Consume the builder and produce an [`InboundMessage`].
    pub fn build(self) -> InboundMessage {
        InboundMessage {
            id: self.id,
            channel: self.channel,
            session_id: self.session_id,
            content: self.content,
            author: self.author,
            timestamp: self.timestamp,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OutboundContent {
    Text(String),
    Markdown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OutboundMessage {
    pub content: OutboundContent,
    pub session_id: SessionId,
    pub reply_to: Option<String>,
}

impl OutboundMessage {
    pub fn text(session_id: impl Into<SessionId>, content: impl Into<String>) -> Self {
        Self {
            content: OutboundContent::Text(content.into()),
            session_id: session_id.into(),
            reply_to: None,
        }
    }
    pub fn as_str(&self) -> &str {
        match &self.content {
            OutboundContent::Text(s) | OutboundContent::Markdown(s) => s,
        }
    }
}
