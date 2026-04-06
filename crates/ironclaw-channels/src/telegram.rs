//! Telegram channel powered by [teloxide](https://docs.rs/teloxide/).
//!
//! Supports text messages, file/photo attachments (captions extracted),
//! and sends a typing indicator before processing each message.

use std::sync::Arc;

use async_trait::async_trait;
use ironclaw_core::{Channel, ChannelId, InboundMessage, MessageHandler, OutboundMessage};
use teloxide::dispatching::UpdateFilterExt;
use teloxide::prelude::*;
use teloxide::types::ChatAction;
use tokio::sync::Notify;
use tracing::{debug, error, info, warn};

/// A Telegram bot channel using long-polling.
pub struct TelegramChannel {
    token: String,
    /// Shared shutdown signal.
    shutdown: Arc<Notify>,
}

impl TelegramChannel {
    /// Create a new Telegram channel with the given bot token.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            shutdown: Arc::new(Notify::new()),
        }
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &'static str {
        "telegram"
    }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> anyhow::Result<()> {
        let bot = Bot::new(&self.token);
        let shutdown = self.shutdown.clone();

        info!("TelegramChannel starting long-poll loop");

        let handler_clone = handler.clone();

        let message_handler = Update::filter_message().endpoint(move |msg: Message, bot: Bot| {
            let handler = handler_clone.clone();
            async move {
                handle_message(msg, &handler, &bot).await;
                respond(())
            }
        });

        let mut dispatcher = Dispatcher::builder(bot.clone(), message_handler).build();

        tokio::select! {
            () = async { dispatcher.dispatch().await } => {},
            () = shutdown.notified() => {
                info!("TelegramChannel shutdown signal received");
            },
        }

        Ok(())
    }

    async fn send(&self, to: &ChannelId, message: OutboundMessage) -> anyhow::Result<()> {
        let chat_id = match to {
            ChannelId::Telegram(id) => ChatId(*id),
            other => anyhow::bail!("TelegramChannel cannot send to {other:?}"),
        };

        let bot = Bot::new(&self.token);
        bot.send_message(chat_id, message.as_str()).await?;
        debug!(chat_id = %chat_id, "Sent outbound message via Telegram");
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("TelegramChannel stopping");
        self.shutdown.notify_one();
        Ok(())
    }
}

/// Process a single inbound Telegram message.
async fn handle_message(msg: Message, handler: &Arc<dyn MessageHandler>, bot: &Bot) {
    let chat_id = msg.chat.id;

    // Extract text content: prefer message text, then caption on media, then skip.
    let content = if let Some(text) = msg.text() {
        text.to_string()
    } else if let Some(caption) = msg.caption() {
        // File / photo attachments — use caption as prompt
        caption.to_string()
    } else {
        warn!(chat_id = %chat_id, "Ignoring non-text Telegram message (no text or caption)");
        return;
    };

    if content.is_empty() {
        return;
    }

    // Send typing indicator
    if let Err(e) = bot.send_chat_action(chat_id, ChatAction::Typing).await {
        warn!(chat_id = %chat_id, error = %e, "Failed to send typing indicator");
    }

    let session_id = format!("tg-{chat_id}");
    let author = msg
        .from
        .as_ref()
        .and_then(|u| u.username.clone())
        .or_else(|| msg.from.as_ref().map(|u| u.first_name.clone()));

    let inbound = InboundMessage {
        id: msg.id.0.to_string(),
        channel: ChannelId::Telegram(chat_id.0),
        session_id,
        content,
        author,
        timestamp: chrono::Utc::now(),
    };

    debug!(chat_id = %chat_id, msg_id = %inbound.id, "Processing Telegram message");

    match handler.handle(inbound).await {
        Ok(Some(out)) => {
            if let Err(e) = bot.send_message(chat_id, out.as_str()).await {
                error!(chat_id = %chat_id, error = %e, "Failed to send reply");
            }
        }
        Ok(None) => {
            debug!(chat_id = %chat_id, "Handler returned no response");
        }
        Err(e) => {
            error!(chat_id = %chat_id, error = %e, "Handler error");
            if let Err(send_err) = bot
                .send_message(chat_id, "Sorry, something went wrong.")
                .await
            {
                error!(chat_id = %chat_id, error = %send_err, "Failed to send error reply");
            }
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_name_is_telegram() {
        let ch = TelegramChannel::new("fake-token");
        assert_eq!(ch.name(), "telegram");
    }

    #[test]
    fn stop_notifies_shutdown() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let ch = TelegramChannel::new("fake-token");
        rt.block_on(async {
            ch.stop().await.expect("stop should succeed");
        });
    }
}
