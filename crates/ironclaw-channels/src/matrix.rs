//! Matrix channel using the Client-Server API via `reqwest`.
//!
//! Long-polls the `/sync` endpoint for new messages and sends
//! replies via the room send endpoint. Uses `reqwest` directly
//! to keep the dependency footprint small.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use ironclaw_core::{
    Channel, ChannelError, ChannelId, InboundMessage, MessageHandler, OutboundMessage,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tracing::{debug, error, info, warn};

/// A Matrix channel using the Client-Server API.
pub struct MatrixChannel {
    homeserver_url: String,
    access_token: String,
    user_id: String,
    /// Shared shutdown signal.
    shutdown: Arc<Notify>,
}

impl MatrixChannel {
    /// Create a new Matrix channel.
    pub fn new(
        homeserver_url: impl Into<String>,
        access_token: impl Into<String>,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            homeserver_url: homeserver_url.into(),
            access_token: access_token.into(),
            user_id: user_id.into(),
            shutdown: Arc::new(Notify::new()),
        }
    }
}

// ── Minimal Matrix C-S API types ───────────────────────────────────────────

/// Response from `/sync`.
#[derive(Debug, Deserialize)]
struct SyncResponse {
    next_batch: String,
    #[serde(default)]
    rooms: Option<SyncRooms>,
}

/// Joined rooms in a sync response.
#[derive(Debug, Deserialize)]
struct SyncRooms {
    #[serde(default)]
    join: Option<HashMap<String, JoinedRoom>>,
}

/// A single joined room with timeline events.
#[derive(Debug, Deserialize)]
struct JoinedRoom {
    #[serde(default)]
    timeline: Option<Timeline>,
}

/// Timeline containing events.
#[derive(Debug, Deserialize)]
struct Timeline {
    #[serde(default)]
    events: Vec<TimelineEvent>,
}

/// A single timeline event.
#[derive(Debug, Deserialize)]
struct TimelineEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    event_id: Option<String>,
    #[serde(default)]
    sender: Option<String>,
    #[serde(default)]
    content: Option<MessageContent>,
}

/// Content of an `m.room.message` event.
#[derive(Debug, Deserialize)]
struct MessageContent {
    #[serde(default)]
    msgtype: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

/// Body for sending an `m.room.message`.
#[derive(Debug, Serialize)]
struct MatrixTextMessage {
    msgtype: String,
    body: String,
}

#[async_trait]
impl Channel for MatrixChannel {
    fn name(&self) -> &'static str {
        "matrix"
    }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<(), ChannelError> {
        (async {
            let client = reqwest::Client::new();
            let mut since: Option<String> = None;
            let shutdown = self.shutdown.clone();

            info!("MatrixChannel starting sync loop");

            loop {
                tokio::select! {
                    () = shutdown.notified() => {
                        info!("MatrixChannel shutdown signal received");
                        break;
                    }
                    result = sync_once(
                        &client,
                        &self.homeserver_url,
                        &self.access_token,
                        &self.user_id,
                        since.as_deref(),
                        &handler,
                    ) => {
                        match result {
                            Ok(next_batch) => {
                                since = Some(next_batch);
                            }
                            Err(e) => {
                                warn!(error = %e, "Matrix sync error, retrying in 5s");
                                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            }
                        }
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
        })
        .await
        .map_err(Into::into)
    }

    async fn send(&self, to: &ChannelId, message: OutboundMessage) -> Result<(), ChannelError> {
        (async {
            let room_id = match to {
                ChannelId::Matrix(id) => id.as_str(),
                other => anyhow::bail!("MatrixChannel cannot send to {other:?}"),
            };

            let client = reqwest::Client::new();
            send_matrix_message(
                &client,
                &self.homeserver_url,
                &self.access_token,
                room_id,
                message.as_str(),
            )
            .await
        })
        .await
        .map_err(Into::into)
    }

    async fn stop(&self) -> Result<(), ChannelError> {
        info!("MatrixChannel stopping");
        self.shutdown.notify_one();
        Ok(())
    }
}

/// Perform a single `/sync` call with 30-second long-poll timeout.
async fn sync_once(
    client: &reqwest::Client,
    homeserver: &str,
    token: &str,
    user_id: &str,
    since: Option<&str>,
    handler: &Arc<dyn MessageHandler>,
) -> anyhow::Result<String> {
    let mut url = format!("{homeserver}/_matrix/client/v3/sync?timeout=30000");
    if let Some(since) = since {
        url.push_str(&format!("&since={since}"));
    }

    let resp: SyncResponse = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let next_batch = resp.next_batch.clone();

    // Skip historical messages on initial sync
    if since.is_none() {
        return Ok(next_batch);
    }

    if let Some(rooms) = resp.rooms {
        if let Some(joined) = rooms.join {
            for (room_id, room) in joined {
                if let Some(timeline) = room.timeline {
                    for event in &timeline.events {
                        process_event(client, homeserver, token, user_id, &room_id, event, handler)
                            .await;
                    }
                }
            }
        }
    }

    Ok(next_batch)
}

/// Process a single Matrix timeline event.
async fn process_event(
    client: &reqwest::Client,
    homeserver: &str,
    token: &str,
    user_id: &str,
    room_id: &str,
    event: &TimelineEvent,
    handler: &Arc<dyn MessageHandler>,
) {
    if event.event_type != "m.room.message" {
        return;
    }

    // Skip own messages
    if event.sender.as_deref() == Some(user_id) {
        return;
    }

    let content = match &event.content {
        Some(c) if c.msgtype.as_deref() == Some("m.text") => c.body.clone().unwrap_or_default(),
        _ => return,
    };

    if content.is_empty() {
        return;
    }

    let sender = event.sender.clone().unwrap_or_default();
    let session_id = format!("matrix-{room_id}");
    let event_id = event.event_id.clone().unwrap_or_default();

    let inbound_builder = InboundMessage::builder(ChannelId::Matrix(room_id.to_string()), content)
        .id(event_id)
        .session_id(session_id)
        .author(sender);
    let inbound = inbound_builder.build();

    match handler.handle(inbound).await {
        Ok(Some(out)) => {
            if let Err(e) =
                send_matrix_message(client, homeserver, token, room_id, out.as_str()).await
            {
                error!(room_id = %room_id, error = %e, "Failed to send Matrix reply");
            }
        }
        Ok(None) => {}
        Err(e) => {
            error!(room_id = %room_id, error = %e, "Handler error in Matrix channel");
        }
    }
}

/// Send a text message to a Matrix room.
async fn send_matrix_message(
    client: &reqwest::Client,
    homeserver: &str,
    token: &str,
    room_id: &str,
    text: &str,
) -> anyhow::Result<()> {
    let txn_id = uuid::Uuid::new_v4().to_string();
    let url =
        format!("{homeserver}/_matrix/client/v3/rooms/{room_id}/send/m.room.message/{txn_id}");

    let body = MatrixTextMessage {
        msgtype: "m.text".to_string(),
        body: text.to_string(),
    };

    client
        .put(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    debug!(room_id = %room_id, "Sent Matrix message");
    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_name_is_matrix() {
        let ch = MatrixChannel::new("https://matrix.org", "fake-token", "@bot:matrix.org");
        assert_eq!(ch.name(), "matrix");
    }

    #[test]
    fn stop_notifies_shutdown() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let ch = MatrixChannel::new("https://matrix.org", "fake-token", "@bot:matrix.org");
        rt.block_on(async {
            ch.stop().await.expect("stop should succeed");
        });
    }
}
