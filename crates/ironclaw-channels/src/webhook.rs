//! Generic inbound webhook channel.
//!
//! Listens for HTTP POST requests and converts them into `InboundMessage`s.
//! Supports optional Bearer token authentication.
//!
//! Request body: `{ "content": "...", "session_id": "...", "author": "..." }`
//! Response body: `{ "content": "...", "session_id": "..." }`

use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use ironclaw_core::{Channel, ChannelId, InboundMessage, MessageHandler, OutboundMessage};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tracing::{info, warn};

/// A generic inbound webhook channel.
pub struct WebhookChannel {
    host: String,
    port: u16,
    path: String,
    auth_token: Option<String>,
    /// Shared shutdown signal.
    shutdown: Arc<Notify>,
}

impl WebhookChannel {
    /// Create a new webhook channel listening on the given host, port, and path.
    pub fn new(host: impl Into<String>, port: u16, path: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port,
            path: path.into(),
            auth_token: None,
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Set a Bearer token for authentication.
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }
}

#[derive(Clone)]
struct WebhookState {
    handler: Arc<dyn MessageHandler>,
    auth_token: Option<String>,
}

/// Inbound webhook request body.
#[derive(Debug, Deserialize)]
struct WebhookPayload {
    content: String,
    session_id: Option<String>,
    author: Option<String>,
}

/// Outbound webhook response body.
#[derive(Debug, Serialize)]
struct WebhookResponse {
    content: String,
    session_id: String,
}

/// Handle an inbound webhook POST.
async fn handle_webhook(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    Json(payload): Json<WebhookPayload>,
) -> Result<Json<WebhookResponse>, StatusCode> {
    // Auth check
    if let Some(expected) = &state.auth_token {
        if !expected.is_empty() {
            let provided = headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "));

            if provided != Some(expected.as_str()) {
                warn!("Webhook request rejected: invalid or missing auth token");
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }

    let session_id = payload
        .session_id
        .unwrap_or_else(|| format!("webhook-{}", uuid::Uuid::new_v4()));

    let inbound = InboundMessage {
        id: uuid::Uuid::new_v4().to_string(),
        channel: ChannelId::Webhook(session_id.clone()),
        session_id: session_id.clone(),
        content: payload.content,
        author: payload.author,
        timestamp: chrono::Utc::now(),
    };

    match state.handler.handle(inbound).await {
        Ok(Some(out)) => Ok(Json(WebhookResponse {
            content: out.as_str().to_string(),
            session_id,
        })),
        Ok(None) => Ok(Json(WebhookResponse {
            content: String::new(),
            session_id,
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[async_trait]
impl Channel for WebhookChannel {
    fn name(&self) -> &'static str {
        "webhook"
    }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> anyhow::Result<()> {
        let state = WebhookState {
            handler,
            auth_token: self.auth_token.clone(),
        };

        let app = Router::new()
            .route(&self.path, post(handle_webhook))
            .with_state(state);

        let addr = format!("{}:{}", self.host, self.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        info!(addr = %addr, path = %self.path, "WebhookChannel listening");

        let shutdown = self.shutdown.clone();
        axum::serve(listener, app)
            .with_graceful_shutdown(async move { shutdown.notified().await })
            .await?;

        Ok(())
    }

    async fn send(&self, _to: &ChannelId, _message: OutboundMessage) -> anyhow::Result<()> {
        // Webhooks are inbound-only; responses are returned synchronously.
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("WebhookChannel stopping");
        self.shutdown.notify_one();
        Ok(())
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_name_is_webhook() {
        let ch = WebhookChannel::new("127.0.0.1", 9000, "/webhook");
        assert_eq!(ch.name(), "webhook");
    }

    #[test]
    fn stop_notifies_shutdown() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let ch = WebhookChannel::new("127.0.0.1", 9000, "/webhook");
        rt.block_on(async {
            ch.stop().await.expect("stop should succeed");
        });
    }
}
