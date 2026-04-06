
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use uuid::Uuid;

// ── Stream type alias ──────────────────────────────────────────────────────
pub type BoxStream<T> =
    Pin<Box<dyn futures::Stream<Item = anyhow::Result<T>> + Send + 'static>>;

// ── Session / Agent IDs ────────────────────────────────────────────────────
pub type SessionId = String;
pub type AgentId   = String;

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
    pub id:        String,
    pub name:      String,
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
    pub id:          Uuid,
    pub role:        Role,
    pub content:     String,
    pub tool_calls:  Vec<ToolCall>,
    pub tool_result: Option<ToolResult>,
    pub timestamp:   DateTime<Utc>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { id: Uuid::new_v4(), role: Role::System, content: content.into(),
               tool_calls: vec![], tool_result: None, timestamp: Utc::now() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { id: Uuid::new_v4(), role: Role::User, content: content.into(),
               tool_calls: vec![], tool_result: None, timestamp: Utc::now() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { id: Uuid::new_v4(), role: Role::Assistant, content: content.into(),
               tool_calls: vec![], tool_result: None, timestamp: Utc::now() }
    }
    pub fn tool_result(call_id: impl Into<String>, content: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(), role: Role::Tool, content: String::new(),
            tool_calls: vec![],
            tool_result: Some(ToolResult { call_id: call_id.into(), content }),
            timestamp: Utc::now(),
        }
    }
}

// ── Token usage ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens:     u32,
    pub completion_tokens: u32,
    pub total_tokens:      u32,
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
    pub messages:    Vec<Message>,
    pub tools:       Vec<ToolSchema>,
    pub max_tokens:  Option<u32>,
    pub temperature: Option<f32>,
    pub stream:      bool,
    pub model:       Option<String>,
}

impl CompletionRequest {
    pub fn simple(content: impl Into<String>) -> Self {
        Self {
            messages:    vec![Message::user(content)],
            tools:       vec![],
            max_tokens:  Some(4096),
            temperature: Some(0.7),
            stream:      false,
            model:       None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub message:     Message,
    pub stop_reason: StopReason,
    pub usage:       TokenUsage,
    pub model:       String,
    pub latency_ms:  u64,
}

impl CompletionResponse {
    pub fn text(&self) -> &str { &self.message.content }
    pub fn has_tool_calls(&self) -> bool { !self.message.tool_calls.is_empty() }
}

// ── Stream chunk ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub delta:     String,
    pub done:      bool,
}

// ── Tool schema ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name:        String,
    pub description: String,
    pub parameters:  serde_json::Value,
}

// ── Memory hit (RAG vector search) ───────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHit {
    pub id:       String,
    pub text:     String,
    pub score:    f32,
    pub metadata: serde_json::Value,
}

// ── Conversation search result ────────────────────────────────────────────
/// A single matching message returned by `MemoryStore::search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// The session this message belongs to.
    pub session_id: SessionId,
    /// The matching message.
    pub message:    Message,
}

// ── Agent types ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id:           Uuid,
    pub instruction:  String,
    pub context:      Vec<Message>,
    pub tool_allowlist: Option<Vec<String>>,
    pub max_tokens:   Option<u32>,
}

impl AgentTask {
    pub fn new(instruction: impl Into<String>) -> Self {
        Self { id: Uuid::new_v4(), instruction: instruction.into(),
               context: vec![], tool_allowlist: None, max_tokens: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    pub task_id:    Uuid,
    pub agent_id:   AgentId,
    pub text:       String,
    pub tool_calls: Vec<ToolCall>,
    pub approved:   bool,
    pub usage:      TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentRole { Orchestrator, Worker, Router, Critic, Planner }

// ── Channel / Message types ───────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "id")]
pub enum ChannelId {
    Telegram(i64),
    Discord(String),
    Rest(String),
    WebSocket(String),
    Cli,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub id:         String,
    pub channel:    ChannelId,
    pub session_id: SessionId,
    pub content:    String,
    pub author:     Option<String>,
    pub timestamp:  DateTime<Utc>,
}

impl InboundMessage {
    pub fn cli(content: impl Into<String>) -> Self {
        Self { id: Uuid::new_v4().to_string(), channel: ChannelId::Cli,
               session_id: "cli-default".into(), content: content.into(),
               author: Some("user".into()), timestamp: Utc::now() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutboundContent {
    Text(String),
    Markdown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub content:    OutboundContent,
    pub session_id: SessionId,
    pub reply_to:   Option<String>,
}

impl OutboundMessage {
    pub fn text(session_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self { content: OutboundContent::Text(content.into()),
               session_id: session_id.into(), reply_to: None }
    }
    pub fn as_str(&self) -> &str {
        match &self.content {
            OutboundContent::Text(s) | OutboundContent::Markdown(s) => s,
        }
    }
}
