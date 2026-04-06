use std::{io::Write, sync::Arc};
use async_trait::async_trait;
use ironclaw_core::{Channel, ChannelId, InboundMessage, MessageHandler, OutboundMessage};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::error;

pub struct CliChannel {
    pub prompt: String,
}

impl CliChannel {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self { prompt: prompt.into() }
    }
}

impl Default for CliChannel {
    fn default() -> Self { Self::new("You") }
}

#[async_trait]
impl Channel for CliChannel {
    fn name(&self) -> &'static str { "cli" }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> anyhow::Result<()> {
        let stdin  = tokio::io::stdin();
        let mut lines = BufReader::new(stdin).lines();
        let session_id = "cli-default".to_string();

        println!("\x1b[1;33m🦀 IronClaw\x1b[0m  (type /quit to exit, /reset to clear history)");
        println!("{}", "─".repeat(50));

        loop {
            print!("\x1b[1;36m{}\x1b[0m: ", self.prompt);
            // flush stdout
            std::io::stdout().flush().ok();

            let line = match lines.next_line().await {
                Ok(Some(l)) => l,
                Ok(None)    => break,   // EOF
                Err(e)      => { error!("stdin error: {e}"); break; }
            };

            let line = line.trim().to_string();
            if line.is_empty()     { continue; }
            if line == "/quit"     { break; }

            let inbound = InboundMessage {
                id:         uuid::Uuid::new_v4().to_string(),
                channel:    ChannelId::Cli,
                session_id: session_id.clone(),
                content:    line,
                author:     Some("user".into()),
                timestamp:  chrono::Utc::now(),
            };

            print!("\x1b[2m…\x1b[0m");
            std::io::stdout().flush().ok();

            match handler.handle(inbound).await {
                Ok(Some(out)) => {
                    print!("\r");
                    println!("\x1b[1;32mIronClaw\x1b[0m: {}", out.as_str());
                }
                Ok(None)  => {}
                Err(e)    => { println!("\x1b[31mError: {e}\x1b[0m"); }
            }
        }
        Ok(())
    }

    async fn send(&self, _to: &ChannelId, msg: OutboundMessage) -> anyhow::Result<()> {
        println!("\x1b[1;32mIronClaw\x1b[0m: {}", msg.as_str());
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> { Ok(()) }
}
