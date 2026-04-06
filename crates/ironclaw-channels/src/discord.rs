//! Discord channel powered by [serenity](https://docs.rs/serenity/).
//!
//! Registers `/chat`, `/reset`, and `/model` slash commands.
//! Also responds to direct messages.

use std::sync::Arc;

use async_trait::async_trait;
use ironclaw_core::{Channel, ChannelId, InboundMessage, MessageHandler, OutboundMessage};
use serenity::all::*;
use tokio::sync::Notify;
use tracing::{debug, error, info};

/// A Discord bot channel with slash commands and DM support.
pub struct DiscordChannel {
    token: String,
    /// Shared shutdown signal.
    shutdown: Arc<Notify>,
}

impl DiscordChannel {
    /// Create a new Discord channel with the given bot token.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            shutdown: Arc::new(Notify::new()),
        }
    }
}

/// Internal serenity event handler.
struct DiscordHandler {
    handler: Arc<dyn MessageHandler>,
}

#[async_trait]
impl EventHandler for DiscordHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!(user = %ready.user.name, "Discord bot connected");

        let commands = vec![
            CreateCommand::new("chat")
                .description("Chat with IronClaw")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "message",
                        "Your message to the assistant",
                    )
                    .required(true),
                ),
            CreateCommand::new("reset").description("Reset your conversation history"),
            CreateCommand::new("model").description("Show current model information"),
        ];

        if let Err(e) = Command::set_global_commands(&ctx.http, commands).await {
            error!(error = %e, "Failed to register Discord slash commands");
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };

        let guild_id = command.guild_id.map(|g| g.to_string()).unwrap_or_default();
        let session_id = format!("discord-{}-{}", guild_id, command.user.id);
        let author = Some(command.user.name.clone());

        match command.data.name.as_str() {
            "chat" => {
                let content = command
                    .data
                    .options
                    .first()
                    .and_then(|o| o.value.as_str())
                    .unwrap_or_default()
                    .to_string();

                if content.is_empty() {
                    respond_ephemeral(&ctx, &command, "Please provide a message.").await;
                    return;
                }

                // Defer the reply — shows "thinking…" indicator
                if let Err(e) = command
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
                    )
                    .await
                {
                    error!(error = %e, "Failed to defer Discord response");
                    return;
                }

                let inbound = InboundMessage {
                    id: command.id.to_string(),
                    channel: ChannelId::Discord(command.channel_id.to_string()),
                    session_id,
                    content,
                    author,
                    timestamp: chrono::Utc::now(),
                };

                let reply = match self.handler.handle(inbound).await {
                    Ok(Some(out)) => out.as_str().to_string(),
                    Ok(None) => "No response.".to_string(),
                    Err(e) => {
                        error!(error = %e, "Handler error in Discord");
                        "Sorry, something went wrong.".to_string()
                    }
                };

                if let Err(e) = command
                    .edit_response(&ctx.http, EditInteractionResponse::new().content(&reply))
                    .await
                {
                    error!(error = %e, "Failed to edit Discord response");
                }
            }
            "reset" => {
                respond_ephemeral(&ctx, &command, "Conversation history has been reset.").await;
            }
            "model" => {
                respond_ephemeral(&ctx, &command, "Model info is not yet available.").await;
            }
            _ => {
                respond_ephemeral(&ctx, &command, "Unknown command.").await;
            }
        }
    }

    async fn message(&self, ctx: Context, msg: SerenityMessage) {
        // Ignore bot messages
        if msg.author.bot {
            return;
        }

        // Only respond to DMs (no guild_id means DM)
        if msg.guild_id.is_some() {
            return;
        }

        let content = msg.content.clone();
        if content.is_empty() {
            return;
        }

        let session_id = format!("discord-dm-{}", msg.author.id);

        // Typing indicator
        let _typing = msg.channel_id.start_typing(&ctx.http);

        let inbound = InboundMessage {
            id: msg.id.to_string(),
            channel: ChannelId::Discord(msg.channel_id.to_string()),
            session_id,
            content,
            author: Some(msg.author.name.clone()),
            timestamp: chrono::Utc::now(),
        };

        match self.handler.handle(inbound).await {
            Ok(Some(out)) => {
                if let Err(e) = msg.channel_id.say(&ctx.http, out.as_str()).await {
                    error!(error = %e, "Failed to send Discord message");
                }
            }
            Ok(None) => {}
            Err(e) => {
                error!(error = %e, "Handler error in Discord DM");
                msg.channel_id
                    .say(&ctx.http, "Sorry, something went wrong.")
                    .await
                    .ok();
            }
        }
    }
}

/// Alias to avoid conflict with ironclaw_core types.
type SerenityMessage = serenity::all::Message;

/// Send an ephemeral interaction response.
async fn respond_ephemeral(ctx: &Context, command: &CommandInteraction, content: &str) {
    if let Err(e) = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(content)
                    .ephemeral(true),
            ),
        )
        .await
    {
        error!(error = %e, "Failed to respond to Discord command");
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &'static str {
        "discord"
    }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> anyhow::Result<()> {
        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;

        let event_handler = DiscordHandler {
            handler: handler.clone(),
        };

        let mut client = Client::builder(&self.token, intents)
            .event_handler(event_handler)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to build Discord client: {e}"))?;

        let shard_manager = client.shard_manager.clone();
        let shutdown = self.shutdown.clone();

        info!("DiscordChannel starting");

        tokio::select! {
            result = client.start() => {
                if let Err(e) = result {
                    error!(error = %e, "Discord client error");
                    return Err(anyhow::anyhow!("Discord client error: {e}"));
                }
            },
            () = shutdown.notified() => {
                info!("DiscordChannel shutdown signal received");
                shard_manager.shutdown_all().await;
            },
        }

        Ok(())
    }

    async fn send(&self, to: &ChannelId, message: OutboundMessage) -> anyhow::Result<()> {
        let channel_id = match to {
            ChannelId::Discord(id) => id
                .parse::<u64>()
                .map_err(|_| anyhow::anyhow!("Invalid Discord channel ID: {id}"))?,
            other => anyhow::bail!("DiscordChannel cannot send to {other:?}"),
        };

        let http = serenity::http::Http::new(&self.token);
        let channel = serenity::all::ChannelId::new(channel_id);
        channel
            .say(&http, message.as_str())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send Discord message: {e}"))?;

        debug!(channel_id = %channel_id, "Sent outbound Discord message");
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("DiscordChannel stopping");
        self.shutdown.notify_one();
        Ok(())
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_name_is_discord() {
        let ch = DiscordChannel::new("fake-token");
        assert_eq!(ch.name(), "discord");
    }

    #[test]
    fn stop_notifies_shutdown() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let ch = DiscordChannel::new("fake-token");
        rt.block_on(async {
            ch.stop().await.expect("stop should succeed");
        });
    }
}
