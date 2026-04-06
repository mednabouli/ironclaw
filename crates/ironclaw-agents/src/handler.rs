
use async_trait::async_trait;
use ironclaw_core::{InboundMessage, MessageHandler, OutboundMessage};
use tracing::{debug, error};
use crate::context::AgentContext;
use crate::react::ReActAgent;

/// Implements MessageHandler by running a ReAct agent loop per message.
pub struct AgentHandler {
    ctx: AgentContext,
}

impl AgentHandler {
    pub fn new(ctx: AgentContext) -> Self { Self { ctx } }
}

#[async_trait]
impl MessageHandler for AgentHandler {
    async fn handle(&self, msg: InboundMessage) -> anyhow::Result<Option<OutboundMessage>> {
        let span = tracing::info_span!("handle", session = %msg.session_id, channel = ?msg.channel);
        let _g   = span.enter();

        debug!(content = %msg.content, "Handling message");

        let agent  = ReActAgent::new(self.ctx.clone());
        let task   = ironclaw_core::AgentTask::new(msg.content.clone());

        match agent.run_with_history(&msg.session_id, task).await {
            Ok(output) => {
                // Persist assistant reply
                let _ = self.ctx.memory.push(&msg.session_id, ironclaw_core::Message::user(msg.content)).await;
                let _ = self.ctx.memory.push(&msg.session_id, ironclaw_core::Message::assistant(output.text.clone())).await;
                Ok(Some(OutboundMessage::text(msg.session_id, output.text)))
            }
            Err(e) => {
                error!(error = %e, "Agent error");
                Ok(Some(OutboundMessage::text(msg.session_id, format!("Error: {e}"))))
            }
        }
    }
}
