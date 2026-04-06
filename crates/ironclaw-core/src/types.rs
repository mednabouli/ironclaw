use std::pin::Pin;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Stream type alias ──────────────────────────────────────────────────────
pub type BoxStream<T> = Pin<Box<dyn futures::Stream<Item = anyhow::Result<T>> + Send + 'static>>;

// ── Session / Agent IDs ────────────────────────────────────────────────────
pub type SessionId = String;
pub type AgentId = String;

// ── Message roles ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

// ── Tool calling ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub content: serde_json::Value,
}

// ── Core Message ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub role: Role,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_result: Option<ToolResult>,
    pub timestamp: DateTime<Utc>,
}

impl Message {
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
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ── Stop reason ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
}

// ── Completion request / response ─────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSchema>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stream: bool,
    pub model: Option<String>,
}

impl CompletionRequest {
    pub fn simple(content: impl Into<String>) -> Self {
        Self {
            messages: vec![Message::user(content)],
            tools: vec![],
            max_tokens: Some(4096),
            temperature: Some(0.7),
            stream: false,
            model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub message: Message,
    pub stop_reason: StopReason,
    pub usage: TokenUsage,
    pub model: String,
    pub latency_ms: u64,
}

impl CompletionResponse {
    pub fn text(&self) -> &str {
        &self.message.content
    }
    pub fn has_tool_calls(&self) -> bool {
        !self.message.tool_calls.is_empty()
    }
}

// ── Streaming tool call delta ─────────────────────────────────────────────
/// An incremental piece of a tool call received during streaming.
/// Providers emit these across multiple chunks; the consumer accumulates
/// them to reconstruct a full `ToolCall`.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ── Stream chunk ──────────────────────────────────────────────────────────
/// A single chunk from a provider's streaming response.
/// Contains text deltas, optional tool call deltas, and a stop reason
/// on the final chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ── Stream events (SSE-level) ─────────────────────────────────────────────
/// High-level events emitted during a streaming agent interaction.
/// These are sent to clients over SSE and represent the full lifecycle
/// of a streamed response including tool call execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// ── Memory hit (RAG vector search) ───────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHit {
    pub id: String,
    pub text: String,
    pub score: f32,
    pub metadata: serde_json::Value,
}

// ── Conversation search result ────────────────────────────────────────────
/// A single matching message returned by `MemoryStore::search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// The session this message belongs to.
    pub session_id: SessionId,
    /// The matching message.
    pub message: Message,
}

// ── Agent types ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: Uuid,
    pub instruction: String,
    pub context: Vec<Message>,
    pub tool_allowlist: Option<Vec<String>>,
    pub max_tokens: Option<u32>,
}

impl AgentTask {
    pub fn new(instruction: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            instruction: instruction.into(),
            context: vec![],
            tool_allowlist: None,
            max_tokens: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    pub task_id: Uuid,
    pub agent_id: AgentId,
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub approved: bool,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentState {
    /// Agent is idle, waiting for a task.
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

impl Default for AgentState {
    fn default() -> Self {
        Self::Idle
    }
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
pub struct InboundMessage {
    pub id: String,
    pub channel: ChannelId,
    pub session_id: SessionId,
    pub content: String,
    pub author: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl InboundMessage {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutboundContent {
    Text(String),
    Markdown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub content: OutboundContent,
    pub session_id: SessionId,
    pub reply_to: Option<String>,
}

impl OutboundMessage {
    pub fn text(session_id: impl Into<String>, content: impl Into<String>) -> Self {
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
