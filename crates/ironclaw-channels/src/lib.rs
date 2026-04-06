
#[cfg(feature = "rest")]
pub mod rest;
#[cfg(feature = "rest")]
pub use rest::RestChannel;

#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "cli")]
pub use cli::CliChannel;

#[cfg(test)]
pub(crate) mod tests {
    use async_trait::async_trait;
    use ironclaw_core::{InboundMessage, MessageHandler, OutboundMessage};

    /// A no-op handler for unit tests that always returns an error (no real agent wired up).
    pub struct NoopHandler;

    #[async_trait]
    impl MessageHandler for NoopHandler {
        async fn handle(&self, _msg: InboundMessage) -> anyhow::Result<Option<OutboundMessage>> {
            anyhow::bail!("NoopHandler: no agent configured")
        }
    }
}
