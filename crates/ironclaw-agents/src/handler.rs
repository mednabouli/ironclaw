use std::sync::Arc;

use async_trait::async_trait;
use ironclaw_core::{
    BoxStream, HandlerError, InboundMessage, Message, MessageHandler, OutboundMessage, StreamEvent,
    ToolCall,
};
use tokio::sync::Mutex;
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
    async fn handle(&self, msg: InboundMessage) -> Result<Option<OutboundMessage>, HandlerError> {
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

    async fn handle_stream(
        &self,
        msg: InboundMessage,
    ) -> Result<BoxStream<StreamEvent>, HandlerError> {
        let span =
            tracing::info_span!("handle_stream", session = %msg.session_id, channel = ?msg.channel);
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

        // Accumulate the real assistant content while streaming
        let accumulated_text = Arc::new(Mutex::new(String::new()));
        let accumulated_tools = Arc::new(Mutex::new(Vec::<ToolCall>::new()));

        let memory = self.ctx.memory.clone();
        let session_id = msg.session_id;
        let wrapped = tokio_stream::StreamExt::map(event_stream, move |event| {
            let memory = memory.clone();
            let session_id = session_id.clone();
            let text = accumulated_text.clone();
            let tools = accumulated_tools.clone();

            match &event {
                Ok(StreamEvent::TokenDelta { delta }) => {
                    let delta = delta.clone();
                    tokio::spawn(async move {
                        text.lock().await.push_str(&delta);
                    });
                }
                Ok(StreamEvent::ToolCallStart {
                    id,
                    name,
                    arguments,
                }) => {
                    let tc = ToolCall::new(id.clone(), name.clone(), arguments.clone());
                    tokio::spawn(async move {
                        tools.lock().await.push(tc);
                    });
                }
                Ok(StreamEvent::Done { .. }) | Ok(StreamEvent::Error { .. }) => {
                    // Persist the real assistant message with accumulated content
                    tokio::spawn(async move {
                        let content = text.lock().await.clone();
                        let tool_calls = tools.lock().await.clone();
                        let mut assistant_msg = Message::assistant(content);
                        assistant_msg.tool_calls = tool_calls;
                        let _ = memory.push(&session_id, assistant_msg).await;
                    });
                }
                _ => {}
            }
            event
        });

        Ok(Box::pin(wrapped))
    }
}
