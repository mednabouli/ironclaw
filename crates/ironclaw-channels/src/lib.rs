#[cfg(feature = "prometheus")]
pub mod metrics;
pub mod middleware;
pub mod ratelimit;

#[cfg(feature = "rest")]
pub mod rest;
#[cfg(feature = "rest")]
pub use rest::RestChannel;

#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "cli")]
pub use cli::CliChannel;

#[cfg(feature = "telegram")]
pub mod telegram;
#[cfg(feature = "telegram")]
pub use telegram::TelegramChannel;

#[cfg(feature = "discord")]
pub mod discord;
#[cfg(feature = "discord")]
pub use discord::DiscordChannel;

#[cfg(feature = "slack")]
pub mod slack;
#[cfg(feature = "slack")]
pub use slack::SlackChannel;

#[cfg(feature = "websocket")]
pub mod websocket;
#[cfg(feature = "websocket")]
pub use websocket::WebSocketChannel;

#[cfg(feature = "webhook")]
pub mod webhook;
#[cfg(feature = "webhook")]
pub use webhook::WebhookChannel;

#[cfg(feature = "matrix")]
pub mod matrix;
#[cfg(feature = "matrix")]
pub use matrix::MatrixChannel;

#[cfg(test)]
pub(crate) mod tests {
    use async_trait::async_trait;
    use ironclaw_core::{HandlerError, InboundMessage, MessageHandler, OutboundMessage};

    /// A no-op handler for unit tests that always returns an error (no real agent wired up).
    pub struct NoopHandler;

    #[async_trait]
    impl MessageHandler for NoopHandler {
        async fn handle(
            &self,
            _msg: InboundMessage,
        ) -> Result<Option<OutboundMessage>, HandlerError> {
            Err(HandlerError::Other(anyhow::anyhow!(
                "NoopHandler: no agent configured"
            )))
        }
    }
}
