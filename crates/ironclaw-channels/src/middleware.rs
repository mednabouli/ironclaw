//! Composable message middleware pipeline.
//!
//! Middleware wraps the inner [`MessageHandler`] and can inspect, modify,
//! or reject messages before/after they reach the handler. The pipeline
//! is assembled once at startup and passed to channels as an
//! `Arc<dyn MessageHandler>`.
//!
//! Built-in middleware:
//! - [`LoggingMiddleware`] — structured tracing for every message
//! - [`SanitizationMiddleware`] — strip control chars, enforce max length
//! - [`AuthMiddleware`] — reject messages from unknown channels/users
//! - [`RateLimitMiddleware`] — per-user token bucket rate limiting

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use ironclaw_core::{
    BoxStream, ChannelId, InboundMessage, MessageHandler, OutboundMessage, StreamEvent,
};
use tracing::{debug, info, warn};

use crate::ratelimit::{RateLimitConfig, RateLimiter};

// ── Logging Middleware ────────────────────────────────────────────────────

/// Logs every inbound message and the resulting outbound response.
pub struct LoggingMiddleware {
    inner: Arc<dyn MessageHandler>,
}

impl LoggingMiddleware {
    /// Wrap a handler with logging.
    pub fn new(inner: Arc<dyn MessageHandler>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl MessageHandler for LoggingMiddleware {
    async fn handle(&self, msg: InboundMessage) -> anyhow::Result<Option<OutboundMessage>> {
        let span = tracing::info_span!(
            "middleware.logging",
            channel = ?msg.channel,
            session_id = %msg.session_id,
            author = ?msg.author,
            content_len = msg.content.len(),
        );

        {
            let _guard = span.enter();
            info!("Inbound message received");
            debug!(content = %msg.content, "Message content");
        }

        let result = self.inner.handle(msg).await;

        {
            let _guard = span.enter();
            match &result {
                Ok(Some(out)) => info!(response_len = out.as_str().len(), "Response sent"),
                Ok(None) => info!("No response"),
                Err(e) => warn!(error = %e, "Handler error"),
            }
        }

        result
    }

    async fn handle_stream(&self, msg: InboundMessage) -> anyhow::Result<BoxStream<StreamEvent>> {
        info!(
            channel = ?msg.channel,
            session_id = %msg.session_id,
            "Inbound stream message"
        );
        self.inner.handle_stream(msg).await
    }
}

// ── Sanitization Middleware ───────────────────────────────────────────────

/// Sanitization rules applied to inbound message content.
#[derive(Debug, Clone)]
pub struct SanitizationConfig {
    /// Maximum message length in bytes. Messages exceeding this are truncated.
    pub max_length: usize,
    /// Whether to strip ASCII control characters (0x00-0x1F except \n, \t).
    pub strip_control_chars: bool,
    /// Whether to trim leading/trailing whitespace.
    pub trim_whitespace: bool,
}

impl Default for SanitizationConfig {
    fn default() -> Self {
        Self {
            max_length: 16_384, // 16KB
            strip_control_chars: true,
            trim_whitespace: true,
        }
    }
}

/// Cleans inbound message content before forwarding to the handler.
pub struct SanitizationMiddleware {
    inner: Arc<dyn MessageHandler>,
    config: SanitizationConfig,
}

impl SanitizationMiddleware {
    /// Wrap a handler with sanitization.
    pub fn new(inner: Arc<dyn MessageHandler>, config: SanitizationConfig) -> Self {
        Self { inner, config }
    }

    /// Apply sanitization rules to message content.
    fn sanitize(&self, input: &str) -> String {
        let mut s = input.to_string();

        // Strip control characters (keep \n and \t)
        if self.config.strip_control_chars {
            s = s
                .chars()
                .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
                .collect();
        }

        // Trim whitespace
        if self.config.trim_whitespace {
            s = s.trim().to_string();
        }

        // Enforce max length (truncate at char boundary)
        if s.len() > self.config.max_length {
            let truncated: String = s.chars().take(self.config.max_length).collect();
            warn!(
                original_len = s.len(),
                max_len = self.config.max_length,
                "Message truncated"
            );
            s = truncated;
        }

        s
    }
}

#[async_trait]
impl MessageHandler for SanitizationMiddleware {
    async fn handle(&self, mut msg: InboundMessage) -> anyhow::Result<Option<OutboundMessage>> {
        let original_len = msg.content.len();
        msg.content = self.sanitize(&msg.content);

        if msg.content.is_empty() {
            debug!(original_len, "Message empty after sanitization, dropping");
            return Ok(None);
        }

        self.inner.handle(msg).await
    }

    async fn handle_stream(
        &self,
        mut msg: InboundMessage,
    ) -> anyhow::Result<BoxStream<StreamEvent>> {
        msg.content = self.sanitize(&msg.content);
        if msg.content.is_empty() {
            return Ok(Box::pin(futures::stream::empty()));
        }
        self.inner.handle_stream(msg).await
    }
}

// ── Auth Middleware ───────────────────────────────────────────────────────

/// Channel-level auth rules. Messages from disallowed channels or
/// non-allowlisted users are rejected.
#[derive(Debug, Clone, Default)]
pub struct AuthConfig {
    /// If non-empty, only these channel types are allowed.
    /// Channel names must match `Channel::name()` (e.g. "rest", "telegram").
    pub allowed_channels: HashSet<String>,
    /// If non-empty, only messages from these author IDs are processed.
    pub allowed_users: HashSet<String>,
}

/// Rejects messages from unauthorized channels or users.
pub struct AuthMiddleware {
    inner: Arc<dyn MessageHandler>,
    config: AuthConfig,
}

impl AuthMiddleware {
    /// Wrap a handler with auth checks.
    pub fn new(inner: Arc<dyn MessageHandler>, config: AuthConfig) -> Self {
        Self { inner, config }
    }

    /// Extract the channel name from a `ChannelId`.
    fn channel_name(id: &ChannelId) -> &'static str {
        match id {
            ChannelId::Telegram(_) => "telegram",
            ChannelId::Discord(_) => "discord",
            ChannelId::Slack(_) => "slack",
            ChannelId::Rest(_) => "rest",
            ChannelId::WebSocket(_) => "websocket",
            ChannelId::Webhook(_) => "webhook",
            ChannelId::Matrix(_) => "matrix",
            ChannelId::Cli => "cli",
            ChannelId::Custom(_) => "custom",
        }
    }

    /// Check if the message is authorized.
    fn is_authorized(&self, msg: &InboundMessage) -> bool {
        // Check channel allowlist
        if !self.config.allowed_channels.is_empty() {
            let name = Self::channel_name(&msg.channel);
            if !self.config.allowed_channels.contains(name) {
                warn!(
                    channel = %name,
                    "Message rejected: channel not in allowlist"
                );
                return false;
            }
        }

        // Check user allowlist
        if !self.config.allowed_users.is_empty() {
            let author = msg.author.as_deref().unwrap_or("");
            if !self.config.allowed_users.contains(author) {
                warn!(
                    author = %author,
                    "Message rejected: user not in allowlist"
                );
                return false;
            }
        }

        true
    }
}

#[async_trait]
impl MessageHandler for AuthMiddleware {
    async fn handle(&self, msg: InboundMessage) -> anyhow::Result<Option<OutboundMessage>> {
        if !self.is_authorized(&msg) {
            return Ok(None);
        }
        self.inner.handle(msg).await
    }

    async fn handle_stream(&self, msg: InboundMessage) -> anyhow::Result<BoxStream<StreamEvent>> {
        if !self.is_authorized(&msg) {
            return Ok(Box::pin(futures::stream::empty()));
        }
        self.inner.handle_stream(msg).await
    }
}

// ── Rate Limit Middleware ────────────────────────────────────────────────

/// Wraps a handler with per-user, per-channel rate limiting.
pub struct RateLimitMiddleware {
    inner: Arc<dyn MessageHandler>,
    limiter: RateLimiter,
}

impl RateLimitMiddleware {
    /// Wrap a handler with rate limiting.
    pub fn new(inner: Arc<dyn MessageHandler>, config: RateLimitConfig) -> Self {
        Self {
            inner,
            limiter: RateLimiter::new(config),
        }
    }

    /// Extract a user identifier from the message for rate-limit keying.
    fn user_key(msg: &InboundMessage) -> String {
        msg.author.clone().unwrap_or_else(|| msg.session_id.clone())
    }

    /// Extract a channel name from the message.
    fn channel_name(msg: &InboundMessage) -> &'static str {
        AuthMiddleware::channel_name(&msg.channel)
    }
}

#[async_trait]
impl MessageHandler for RateLimitMiddleware {
    async fn handle(&self, msg: InboundMessage) -> anyhow::Result<Option<OutboundMessage>> {
        let user = Self::user_key(&msg);
        let channel = Self::channel_name(&msg);

        if let Err(wait) = self.limiter.try_acquire(channel, &user) {
            let secs = wait.as_secs();
            return Ok(Some(OutboundMessage::text(
                msg.session_id.clone(),
                format!("Rate limited. Please try again in {secs}s."),
            )));
        }

        self.inner.handle(msg).await
    }

    async fn handle_stream(&self, msg: InboundMessage) -> anyhow::Result<BoxStream<StreamEvent>> {
        let user = Self::user_key(&msg);
        let channel = Self::channel_name(&msg);

        if let Err(wait) = self.limiter.try_acquire(channel, &user) {
            let secs = wait.as_secs();
            let err_msg = format!("Rate limited. Please try again in {secs}s.");
            let events: Vec<anyhow::Result<StreamEvent>> =
                vec![Ok(StreamEvent::Error { message: err_msg })];
            return Ok(Box::pin(futures::stream::iter(events)));
        }

        self.inner.handle_stream(msg).await
    }
}

// ── Pipeline Builder ─────────────────────────────────────────────────────

/// Builds a middleware pipeline around a base [`MessageHandler`].
///
/// Middleware is applied in the order added — the first middleware added
/// is the outermost wrapper (first to receive messages).
///
/// # Example
/// ```ignore
/// let handler = MiddlewarePipeline::new(base_handler)
///     .with_logging()
///     .with_sanitization(SanitizationConfig::default())
///     .with_rate_limit(RateLimitConfig::default())
///     .build();
/// ```
pub struct MiddlewarePipeline {
    handler: Arc<dyn MessageHandler>,
}

impl MiddlewarePipeline {
    /// Start a pipeline with the given base handler.
    pub fn new(handler: Arc<dyn MessageHandler>) -> Self {
        Self { handler }
    }

    /// Add logging middleware (outermost recommended).
    pub fn with_logging(self) -> Self {
        Self {
            handler: Arc::new(LoggingMiddleware::new(self.handler)),
        }
    }

    /// Add content sanitization.
    pub fn with_sanitization(self, config: SanitizationConfig) -> Self {
        Self {
            handler: Arc::new(SanitizationMiddleware::new(self.handler, config)),
        }
    }

    /// Add auth checks.
    pub fn with_auth(self, config: AuthConfig) -> Self {
        Self {
            handler: Arc::new(AuthMiddleware::new(self.handler, config)),
        }
    }

    /// Add per-user rate limiting.
    pub fn with_rate_limit(self, config: RateLimitConfig) -> Self {
        Self {
            handler: Arc::new(RateLimitMiddleware::new(self.handler, config)),
        }
    }

    /// Return the assembled handler.
    pub fn build(self) -> Arc<dyn MessageHandler> {
        self.handler
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use ironclaw_core::ChannelId;

    struct EchoHandler;

    #[async_trait]
    impl MessageHandler for EchoHandler {
        async fn handle(&self, msg: InboundMessage) -> anyhow::Result<Option<OutboundMessage>> {
            Ok(Some(OutboundMessage::text(&msg.session_id, &msg.content)))
        }
    }

    fn make_msg(content: &str) -> InboundMessage {
        InboundMessage {
            id: "test-1".into(),
            channel: ChannelId::Rest("s1".into()),
            session_id: "s1".into(),
            content: content.into(),
            author: Some("alice".into()),
            timestamp: chrono::Utc::now(),
        }
    }

    // ── Sanitization tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn sanitization_strips_control_chars() {
        let handler = Arc::new(EchoHandler);
        let mw = SanitizationMiddleware::new(handler, SanitizationConfig::default());

        let msg = make_msg("hello\x00\x01\x02world\nok");
        let result = mw.handle(msg).await.unwrap().unwrap();
        assert_eq!(result.as_str(), "helloworld\nok");
    }

    #[tokio::test]
    async fn sanitization_trims_whitespace() {
        let handler = Arc::new(EchoHandler);
        let mw = SanitizationMiddleware::new(handler, SanitizationConfig::default());

        let msg = make_msg("  hello  ");
        let result = mw.handle(msg).await.unwrap().unwrap();
        assert_eq!(result.as_str(), "hello");
    }

    #[tokio::test]
    async fn sanitization_truncates_long_messages() {
        let handler = Arc::new(EchoHandler);
        let config = SanitizationConfig {
            max_length: 5,
            ..Default::default()
        };
        let mw = SanitizationMiddleware::new(handler, config);

        let msg = make_msg("abcdefghij");
        let result = mw.handle(msg).await.unwrap().unwrap();
        assert_eq!(result.as_str(), "abcde");
    }

    #[tokio::test]
    async fn sanitization_drops_empty_messages() {
        let handler = Arc::new(EchoHandler);
        let mw = SanitizationMiddleware::new(handler, SanitizationConfig::default());

        let msg = make_msg("   ");
        let result = mw.handle(msg).await.unwrap();
        assert!(result.is_none());
    }

    // ── Auth tests ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn auth_allows_when_no_restrictions() {
        let handler = Arc::new(EchoHandler);
        let mw = AuthMiddleware::new(handler, AuthConfig::default());

        let msg = make_msg("hello");
        let result = mw.handle(msg).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn auth_rejects_wrong_channel() {
        let handler = Arc::new(EchoHandler);
        let config = AuthConfig {
            allowed_channels: ["telegram".to_string()].into_iter().collect(),
            allowed_users: HashSet::new(),
        };
        let mw = AuthMiddleware::new(handler, config);

        // msg is from "rest" channel
        let msg = make_msg("hello");
        let result = mw.handle(msg).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn auth_rejects_wrong_user() {
        let handler = Arc::new(EchoHandler);
        let config = AuthConfig {
            allowed_channels: HashSet::new(),
            allowed_users: ["bob".to_string()].into_iter().collect(),
        };
        let mw = AuthMiddleware::new(handler, config);

        // msg author is "alice"
        let msg = make_msg("hello");
        let result = mw.handle(msg).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn auth_allows_correct_user() {
        let handler = Arc::new(EchoHandler);
        let config = AuthConfig {
            allowed_channels: HashSet::new(),
            allowed_users: ["alice".to_string()].into_iter().collect(),
        };
        let mw = AuthMiddleware::new(handler, config);

        let msg = make_msg("hello");
        let result = mw.handle(msg).await.unwrap();
        assert!(result.is_some());
    }

    // ── Rate limit middleware tests ──────────────────────────────────────

    #[tokio::test]
    async fn rate_limit_allows_within_capacity() {
        let handler = Arc::new(EchoHandler);
        let config = RateLimitConfig {
            capacity: 2,
            refill_tokens: 1,
            refill_interval: std::time::Duration::from_secs(60),
        };
        let mw = RateLimitMiddleware::new(handler, config);

        let msg1 = make_msg("one");
        let msg2 = make_msg("two");
        assert_eq!(mw.handle(msg1).await.unwrap().unwrap().as_str(), "one");
        assert_eq!(mw.handle(msg2).await.unwrap().unwrap().as_str(), "two");
    }

    #[tokio::test]
    async fn rate_limit_rejects_over_capacity() {
        let handler = Arc::new(EchoHandler);
        let config = RateLimitConfig {
            capacity: 1,
            refill_tokens: 1,
            refill_interval: std::time::Duration::from_secs(60),
        };
        let mw = RateLimitMiddleware::new(handler, config);

        let msg1 = make_msg("ok");
        assert_eq!(mw.handle(msg1).await.unwrap().unwrap().as_str(), "ok");

        let msg2 = make_msg("rejected");
        let result = mw.handle(msg2).await.unwrap().unwrap();
        assert!(result.as_str().contains("Rate limited"));
    }

    // ── Pipeline builder tests ──────────────────────────────────────────

    #[tokio::test]
    async fn pipeline_chains_middleware() {
        let base: Arc<dyn MessageHandler> = Arc::new(EchoHandler);
        let pipeline = MiddlewarePipeline::new(base)
            .with_sanitization(SanitizationConfig::default())
            .with_rate_limit(RateLimitConfig {
                capacity: 10,
                refill_tokens: 1,
                refill_interval: std::time::Duration::from_secs(60),
            })
            .with_logging()
            .build();

        let msg = make_msg("  hello\x00world  ");
        let result = pipeline.handle(msg).await.unwrap().unwrap();
        assert_eq!(result.as_str(), "helloworld");
    }

    // ── Logging middleware test ──────────────────────────────────────────

    #[tokio::test]
    async fn logging_passes_through() {
        let handler = Arc::new(EchoHandler);
        let mw = LoggingMiddleware::new(handler);

        let msg = make_msg("test");
        let result = mw.handle(msg).await.unwrap().unwrap();
        assert_eq!(result.as_str(), "test");
    }
}
