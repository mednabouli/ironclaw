//! Typed error enums for each domain in the IronClaw framework.
//!
//! These replace `anyhow::Result` at trait boundaries so that downstream
//! crates can pattern-match on specific failure modes without down-casting.
//! Every enum includes an `Other` variant that wraps an arbitrary error,
//! making migration from `anyhow::Result` incremental.

use std::fmt;

// ── ProviderError ──────────────────────────────────────────────────────────

/// Errors returned by [`crate::Provider`] trait methods.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProviderError {
    /// HTTP or network transport failure.
    Request(String),
    /// Authentication / authorisation failure (bad API key, expired token, …).
    Auth(String),
    /// The provider throttled the request.
    RateLimit {
        /// Retry-after hint in milliseconds, if the provider supplied one.
        retry_after_ms: Option<u64>,
    },
    /// The requested model is not available on this provider.
    ModelNotFound(String),
    /// The response could not be parsed into the expected shape.
    InvalidResponse(String),
    /// The streaming connection was dropped before a final chunk.
    StreamTerminated,
    /// Catch-all for errors that don't fit any specific variant.
    Other(anyhow::Error),
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(msg) => write!(f, "HTTP request failed: {msg}"),
            Self::Auth(msg) => write!(f, "Authentication failed: {msg}"),
            Self::RateLimit { retry_after_ms } => match retry_after_ms {
                Some(ms) => write!(f, "Rate limited — retry after {ms}ms"),
                None => write!(f, "Rate limited"),
            },
            Self::ModelNotFound(model) => write!(f, "Model not found: {model}"),
            Self::InvalidResponse(msg) => write!(f, "Invalid response: {msg}"),
            Self::StreamTerminated => write!(f, "Stream terminated unexpectedly"),
            Self::Other(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for ProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Other(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for ProviderError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

// ── ChannelError ───────────────────────────────────────────────────────────

/// Errors returned by [`crate::Channel`] trait methods.
#[derive(Debug)]
#[non_exhaustive]
pub enum ChannelError {
    /// Failed to bind / start the channel transport.
    StartFailed(String),
    /// Could not deliver a message to the target.
    SendFailed(String),
    /// Channel has already been stopped or was never started.
    NotRunning,
    /// Catch-all.
    Other(anyhow::Error),
}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartFailed(msg) => write!(f, "Channel start failed: {msg}"),
            Self::SendFailed(msg) => write!(f, "Channel send failed: {msg}"),
            Self::NotRunning => write!(f, "Channel is not running"),
            Self::Other(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for ChannelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Other(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for ChannelError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

// ── ToolError ──────────────────────────────────────────────────────────────

/// Errors returned by [`crate::Tool`] trait methods.
#[derive(Debug)]
#[non_exhaustive]
pub enum ToolError {
    /// The parameters supplied to the tool are invalid.
    InvalidParams(String),
    /// The tool execution failed.
    ExecutionFailed(String),
    /// The tool timed out.
    Timeout,
    /// Catch-all.
    Other(anyhow::Error),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParams(msg) => write!(f, "Invalid tool parameters: {msg}"),
            Self::ExecutionFailed(msg) => write!(f, "Tool execution failed: {msg}"),
            Self::Timeout => write!(f, "Tool execution timed out"),
            Self::Other(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for ToolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Other(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for ToolError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

// ── MemoryError ────────────────────────────────────────────────────────────

/// Errors returned by [`crate::MemoryStore`] and [`crate::VectorStore`] trait methods.
#[derive(Debug)]
#[non_exhaustive]
pub enum MemoryError {
    /// The backing storage is unavailable or returned an I/O error.
    Storage(String),
    /// A serialization / deserialization round-trip failed.
    Serialization(String),
    /// The requested session or entry was not found.
    NotFound(String),
    /// Catch-all.
    Other(anyhow::Error),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(msg) => write!(f, "Memory storage error: {msg}"),
            Self::Serialization(msg) => write!(f, "Serialization error: {msg}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::Other(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for MemoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Other(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for MemoryError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

// ── AgentError ─────────────────────────────────────────────────────────────

/// Errors returned by [`crate::Agent`] and [`crate::AgentBus`] trait methods.
#[derive(Debug)]
#[non_exhaustive]
pub enum AgentError {
    /// The provider call within the agent loop failed.
    Provider(ProviderError),
    /// A tool invocation inside the agent loop failed.
    Tool(ToolError),
    /// The agent exceeded its iteration / token budget.
    BudgetExceeded(String),
    /// The requested agent was not found in the bus registry.
    NotFound(String),
    /// Catch-all.
    Other(anyhow::Error),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(e) => write!(f, "Agent provider error: {e}"),
            Self::Tool(e) => write!(f, "Agent tool error: {e}"),
            Self::BudgetExceeded(msg) => write!(f, "Budget exceeded: {msg}"),
            Self::NotFound(id) => write!(f, "Agent not found: {id}"),
            Self::Other(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for AgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Provider(e) => Some(e),
            Self::Tool(e) => Some(e),
            Self::Other(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for AgentError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

impl From<ProviderError> for AgentError {
    fn from(e: ProviderError) -> Self {
        Self::Provider(e)
    }
}

impl From<ToolError> for AgentError {
    fn from(e: ToolError) -> Self {
        Self::Tool(e)
    }
}

// ── HandlerError ───────────────────────────────────────────────────────────

/// Errors returned by [`crate::MessageHandler`] trait methods.
#[derive(Debug)]
#[non_exhaustive]
pub enum HandlerError {
    /// The underlying agent failed.
    Agent(AgentError),
    /// The channel could not process the message.
    Channel(ChannelError),
    /// Catch-all.
    Other(anyhow::Error),
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent(e) => write!(f, "Handler agent error: {e}"),
            Self::Channel(e) => write!(f, "Handler channel error: {e}"),
            Self::Other(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for HandlerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Agent(e) => Some(e),
            Self::Channel(e) => Some(e),
            Self::Other(e) => Some(e.as_ref()),
        }
    }
}

impl From<anyhow::Error> for HandlerError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

impl From<AgentError> for HandlerError {
    fn from(e: AgentError) -> Self {
        Self::Agent(e)
    }
}

impl From<ChannelError> for HandlerError {
    fn from(e: ChannelError) -> Self {
        Self::Channel(e)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_error_display() {
        let e = ProviderError::Request("connection refused".into());
        assert_eq!(e.to_string(), "HTTP request failed: connection refused");
    }

    #[test]
    fn provider_error_rate_limit_display() {
        let e = ProviderError::RateLimit {
            retry_after_ms: Some(5000),
        };
        assert_eq!(e.to_string(), "Rate limited — retry after 5000ms");
    }

    #[test]
    fn channel_error_display() {
        let e = ChannelError::NotRunning;
        assert_eq!(e.to_string(), "Channel is not running");
    }

    #[test]
    fn tool_error_display() {
        let e = ToolError::InvalidParams("missing 'query'".into());
        assert_eq!(e.to_string(), "Invalid tool parameters: missing 'query'");
    }

    #[test]
    fn memory_error_display() {
        let e = MemoryError::NotFound("session-42".into());
        assert_eq!(e.to_string(), "Not found: session-42");
    }

    #[test]
    fn agent_error_from_provider() {
        let pe = ProviderError::Auth("bad key".into());
        let ae: AgentError = pe.into();
        assert!(matches!(ae, AgentError::Provider(_)));
        assert_eq!(
            ae.to_string(),
            "Agent provider error: Authentication failed: bad key"
        );
    }

    #[test]
    fn agent_error_from_tool() {
        let te = ToolError::Timeout;
        let ae: AgentError = te.into();
        assert!(matches!(ae, AgentError::Tool(_)));
    }

    #[test]
    fn handler_error_from_agent() {
        let ae = AgentError::NotFound("bot-1".into());
        let he: HandlerError = ae.into();
        assert!(matches!(he, HandlerError::Agent(_)));
    }

    #[test]
    fn from_anyhow_creates_other_variant() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let pe: ProviderError = anyhow_err.into();
        assert!(matches!(pe, ProviderError::Other(_)));
    }
}
