//! Slack channel using the [Events API](https://api.slack.com/events-api).
//!
//! Listens for Slack event payloads over HTTP, handles URL verification
//! challenges, and sends responses via the Slack Web API.

use std::sync::Arc;

use async_trait::async_trait;
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use ironclaw_core::{
    Channel, ChannelError, ChannelId, InboundMessage, MessageHandler, OutboundMessage,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tracing::{debug, error, info};

/// A Slack bot channel using the Events API + Web API.
pub struct SlackChannel {
    bot_token: String,
    signing_secret: String,
    host: String,
    port: u16,
    /// Shared shutdown signal.
    shutdown: Arc<Notify>,
}

impl SlackChannel {
    /// Create a new Slack channel.
    pub fn new(
        bot_token: impl Into<String>,
        signing_secret: impl Into<String>,
        host: impl Into<String>,
        port: u16,
    ) -> Self {
        Self {
            bot_token: bot_token.into(),
            signing_secret: signing_secret.into(),
            host: host.into(),
            port,
            shutdown: Arc::new(Notify::new()),
        }
    }
}

#[derive(Clone)]
struct SlackState {
    handler: Arc<dyn MessageHandler>,
    bot_token: String,
    #[allow(dead_code)]
    signing_secret: String,
}

/// Top-level Slack Events API payload.
#[derive(Debug, Deserialize)]
struct SlackEventPayload {
    #[serde(rename = "type")]
    event_type: String,
    challenge: Option<String>,
    event: Option<SlackMessageEvent>,
}

/// A Slack message event from the Events API.
#[derive(Debug, Deserialize)]
struct SlackMessageEvent {
    #[serde(rename = "type")]
    event_type: String,
    text: Option<String>,
    user: Option<String>,
    channel: Option<String>,
    ts: Option<String>,
    #[serde(default)]
    bot_id: Option<String>,
}

/// Payload for `chat.postMessage`.
#[derive(Debug, Serialize)]
struct SlackPostMessage {
    channel: String,
    text: String,
}

/// Handle incoming Slack event payloads.
async fn handle_events(
    State(state): State<SlackState>,
    Json(payload): Json<SlackEventPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // URL verification challenge
    if payload.event_type == "url_verification" {
        if let Some(challenge) = payload.challenge {
            return Ok(Json(serde_json::json!({ "challenge": challenge })));
        }
    }

    // Process message events
    if payload.event_type == "event_callback" {
        if let Some(msg_event) = payload.event {
            // Skip bot messages to avoid loops
            if msg_event.bot_id.is_some() {
                return Ok(Json(serde_json::json!({ "ok": true })));
            }

            if msg_event.event_type == "message" {
                if let (Some(text), Some(channel), Some(user)) = (
                    msg_event.text.clone(),
                    msg_event.channel.clone(),
                    msg_event.user.clone(),
                ) {
                    let session_id = format!("slack-{channel}-{user}");
                    let msg_id = msg_event.ts.unwrap_or_default();

                    let inbound = InboundMessage::builder(ChannelId::Slack(channel.clone()), text)
                        .id(msg_id)
                        .session_id(session_id)
                        .author(user)
                        .build();

                    // Process in background to respond to Slack within 3s
                    let handler = state.handler.clone();
                    let bot_token = state.bot_token.clone();
                    tokio::spawn(async move {
                        match handler.handle(inbound).await {
                            Ok(Some(out)) => {
                                if let Err(e) =
                                    post_slack_message(&bot_token, &channel, out.as_str()).await
                                {
                                    error!(error = %e, "Failed to send Slack message");
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                error!(error = %e, "Handler error in Slack channel");
                                post_slack_message(
                                    &bot_token,
                                    &channel,
                                    "Sorry, something went wrong.",
                                )
                                .await
                                .ok();
                            }
                        }
                    });
                }
            }
        }
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Post a message to a Slack channel via the Web API.
async fn post_slack_message(bot_token: &str, channel: &str, text: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let body = SlackPostMessage {
        channel: channel.to_string(),
        text: text.to_string(),
    };

    client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    debug!(channel = %channel, "Sent Slack message");
    Ok(())
}

#[async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &'static str {
        "slack"
    }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<(), ChannelError> {
        (async {
            let state = SlackState {
                handler,
                bot_token: self.bot_token.clone(),
                signing_secret: self.signing_secret.clone(),
            };

            let app = Router::new()
                .route("/slack/events", post(handle_events))
                .with_state(state);

            let addr = format!("{}:{}", self.host, self.port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            info!(addr = %addr, "SlackChannel listening on /slack/events");

            let shutdown = self.shutdown.clone();
            axum::serve(listener, app)
                .with_graceful_shutdown(async move { shutdown.notified().await })
                .await?;

            Ok::<(), anyhow::Error>(())
        })
        .await
        .map_err(Into::into)
    }

    async fn send(&self, to: &ChannelId, message: OutboundMessage) -> Result<(), ChannelError> {
        (async {
            let channel = match to {
                ChannelId::Slack(ch) => ch.as_str(),
                other => anyhow::bail!("SlackChannel cannot send to {other:?}"),
            };

            post_slack_message(&self.bot_token, channel, message.as_str()).await
        })
        .await
        .map_err(Into::into)
    }

    async fn stop(&self) -> Result<(), ChannelError> {
        info!("SlackChannel stopping");
        self.shutdown.notify_one();
        Ok(())
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_name_is_slack() {
        let ch = SlackChannel::new("xoxb-fake", "signing-secret", "127.0.0.1", 3000);
        assert_eq!(ch.name(), "slack");
    }

    #[test]
    fn stop_notifies_shutdown() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let ch = SlackChannel::new("xoxb-fake", "signing-secret", "127.0.0.1", 3000);
        rt.block_on(async {
            ch.stop().await.expect("stop should succeed");
        });
    }
}
