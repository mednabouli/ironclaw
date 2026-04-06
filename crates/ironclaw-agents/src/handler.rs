use async_trait::async_trait;
use ironclaw_core::{BoxStream, InboundMessage, Message, MessageHandler, OutboundMessage, StreamEvent};
use tracing::{debug, error};

use crate::{context::AgentContext, react::ReActAgent};

/// Implements MessageHandler by running a ReAct agent loop per message.
pub struct AgentHandler {
    ctx: AgentContext,
}

impl AgentHandler {
    pub fn new(ctx: AgentContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl MessageHandler for AgentHandler {
    async fn handle(&self, msg: InboundMessage) -> anyhow::Result<Option<OutboundMessage>> {
        let span = tracing::info_span!("handle", session = %msg.session_id, channel = ?msg.channel);
        let _g = span.enter();

        debug!(content = %msg.content, "Handling message");

        let agent = ReActAgent::new(self.ctx.clone());
        let task = ironclaw_core::AgentTask::new(msg.content.clone());

        match agent.run_with_history(&msg.session_id, task).await {
            Ok(output) => {
                // Persist assistant reply
                let _ = self
                    .ctx
                    .memory
                    .push(&msg.session_id, ironclaw_core::Message::user(msg.content))
                    .await;
                let _ = self
                    .ctx
                    .memory
                    .push(
                        &msg.session_id,
                        ironclaw_core::Message::assistant(output.text.clone()),
                    )
                    .await;
                Ok(Some(OutboundMessage::text(msg.session_id, output.text)))
            }
            Err(e) => {
                error!(error = %e, "Agent error");
                Ok(Some(OutboundMessage::text(
                    msg.session_id,
                    format!("Error: {e}"),
                )))
            }
        }
    }

    async fn handle_stream(&self, msg: InboundMessage) -> anyhow::Result<BoxStream<StreamEvent>> {
        let span = tracing::info_span!("handle_stream", session = %msg.session_id, channel = ?msg.channel);
        let _g = span.enter();

        debug!(content = %msg.content, "Handling stream message");

        // Persist the user message up front
        let _ = self
            .ctx
            .memory
            .push(&msg.session_id, Message::user(&msg.content))
            .await;

        let agent = ReActAgent::new(self.ctx.clone());
        let task = ironclaw_core::AgentTask::new(msg.content);

        let event_stream = agent.stream_with_history(msg.session_id.clone(), task);

        // Wrap the stream to persist the assistant reply when Done arrives
        let memory = self.ctx.memory.clone();
        let session_id = msg.session_id;
        let wrapped = tokio_stream::StreamExt::map(event_stream, move |event| {
            let memory = memory.clone();
            let session_id = session_id.clone();
            if let Ok(StreamEvent::Done { .. }) = &event {
                // Persistence is best-effort; fire and forget
                tokio::spawn(async move {
                    let _ = memory
                        .push(&session_id, Message::assistant("[streamed]"))
                        .await;
                });
            }
            event
        });

        Ok(Box::pin(wrapped))
    }
}
