//! WebSocket channel using axum's built-in WebSocket support.
//!
//! Accepts JSON frames: `{ "session_id": "...", "content": "..." }`
//! Returns JSON frames: `{ "content": "...", "session_id": "..." }`

use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket},
        State, WebSocketUpgrade,
    },
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use ironclaw_core::{Channel, ChannelId, InboundMessage, MessageHandler, OutboundMessage};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tracing::{debug, error, info, warn};

/// A WebSocket channel that accepts JSON frames over a persistent connection.
pub struct WebSocketChannel {
    host: String,
    port: u16,
    /// Shared shutdown signal.
    shutdown: Arc<Notify>,
}

impl WebSocketChannel {
    /// Create a new WebSocket channel bound to the given host and port.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            shutdown: Arc::new(Notify::new()),
        }
    }
}

#[derive(Clone)]
struct WsState {
    handler: Arc<dyn MessageHandler>,
}

/// Inbound JSON frame from a WebSocket client.
#[derive(Debug, Deserialize)]
struct WsInbound {
    session_id: Option<String>,
    content: String,
}

/// Outbound JSON frame sent to a WebSocket client.
#[derive(Debug, Serialize)]
struct WsOutbound {
    content: String,
    session_id: String,
}

/// Upgrade an HTTP request to a WebSocket connection.
async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<WsState>,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

/// Handle a single WebSocket connection.
async fn handle_ws(socket: WebSocket, state: WsState) {
    let (mut sender, mut receiver) = socket.split();
    let conn_id = uuid::Uuid::new_v4().to_string();

    debug!(conn_id = %conn_id, "WebSocket connection established");

    while let Some(Ok(msg)) = receiver.next().await {
        let text = match msg {
            WsMessage::Text(t) => t,
            WsMessage::Close(_) => break,
            _ => continue,
        };

        let inbound: WsInbound = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "Invalid WebSocket JSON frame");
                let err = serde_json::json!({ "error": "Invalid JSON format" });
                sender.send(WsMessage::Text(err.to_string())).await.ok();
                continue;
            }
        };

        let session_id = inbound
            .session_id
            .unwrap_or_else(|| format!("ws-{conn_id}"));

        let msg = InboundMessage {
            id: uuid::Uuid::new_v4().to_string(),
            channel: ChannelId::WebSocket(conn_id.clone()),
            session_id: session_id.clone(),
            content: inbound.content,
            author: None,
            timestamp: chrono::Utc::now(),
        };

        match state.handler.handle(msg).await {
            Ok(Some(out)) => {
                let response = WsOutbound {
                    content: out.as_str().to_string(),
                    session_id,
                };
                if let Ok(json) = serde_json::to_string(&response) {
                    sender.send(WsMessage::Text(json)).await.ok();
                }
            }
            Ok(None) => {}
            Err(e) => {
                error!(error = %e, "Handler error in WebSocket");
                let err = serde_json::json!({ "error": "Internal error" });
                sender.send(WsMessage::Text(err.to_string())).await.ok();
            }
        }
    }

    debug!(conn_id = %conn_id, "WebSocket connection closed");
}

#[async_trait]
impl Channel for WebSocketChannel {
    fn name(&self) -> &'static str {
        "websocket"
    }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> anyhow::Result<()> {
        let state = WsState { handler };

        let app = Router::new()
            .route("/ws", get(ws_upgrade))
            .with_state(state);

        let addr = format!("{}:{}", self.host, self.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        info!(addr = %addr, "WebSocketChannel listening on /ws");

        let shutdown = self.shutdown.clone();
        axum::serve(listener, app)
            .with_graceful_shutdown(async move { shutdown.notified().await })
            .await?;

        Ok(())
    }

    async fn send(&self, _to: &ChannelId, _message: OutboundMessage) -> anyhow::Result<()> {
        // WebSocket responses are sent inline during the connection.
        // Out-of-band push would require connection tracking (not yet implemented).
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("WebSocketChannel stopping");
        self.shutdown.notify_one();
        Ok(())
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_name_is_websocket() {
        let ch = WebSocketChannel::new("127.0.0.1", 8081);
        assert_eq!(ch.name(), "websocket");
    }

    #[test]
    fn stop_notifies_shutdown() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let ch = WebSocketChannel::new("127.0.0.1", 8081);
        rt.block_on(async {
            ch.stop().await.expect("stop should succeed");
        });
    }
}
