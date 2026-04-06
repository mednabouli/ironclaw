use std::collections::BTreeMap;

use anyhow::Context;
use ironclaw_core::*;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

use crate::context::AgentContext;

const MAX_ITERATIONS: usize = 10;

pub struct ReActAgent {
    ctx: AgentContext,
    id: AgentId,
}

impl ReActAgent {
    pub fn new(ctx: AgentContext) -> Self {
        Self {
            ctx,
            id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Run with history loaded from memory.
    pub async fn run_with_history(
        &self,
        session_id: &SessionId,
        task: AgentTask,
    ) -> anyhow::Result<AgentOutput> {
        let history = self.ctx.memory.history(session_id, 50).await?;
        let mut messages: Vec<Message> =
            vec![Message::system(&self.ctx.config.agent.system_prompt)];
        messages.extend(history);
        messages.push(Message::user(&task.instruction));

        self.react_loop(task, messages).await
    }

    async fn react_loop(
        &self,
        task: AgentTask,
        mut messages: Vec<Message>,
    ) -> anyhow::Result<AgentOutput> {
        let provider = self
            .ctx
            .providers
            .resolve()
            .await
            .context("No provider available")?;

        let tool_schemas = match &task.tool_allowlist {
            Some(allow) => self.ctx.tools.filtered_schemas(allow),
            None => self.ctx.tools.all_schemas(),
        };

        let mut total_usage = TokenUsage::default();

        for iteration in 0..MAX_ITERATIONS {
            debug!(iteration, "ReAct iteration");

            let req = CompletionRequest {
                messages: messages.clone(),
                tools: tool_schemas.clone(),
                max_tokens: task.max_tokens.or(Some(self.ctx.config.agent.max_tokens)),
                temperature: Some(self.ctx.config.agent.temperature),
                stream: false,
                model: None,
            };

            let resp = provider
                .complete(req)
                .await
                .with_context(|| format!("Provider error at iteration {iteration}"))?;

            total_usage.prompt_tokens += resp.usage.prompt_tokens;
            total_usage.completion_tokens += resp.usage.completion_tokens;
            total_usage.total_tokens += resp.usage.total_tokens;

            if resp.stop_reason == StopReason::ToolUse || !resp.message.tool_calls.is_empty() {
                // Add assistant message with tool calls
                messages.push(resp.message.clone());

                // Invoke each tool
                for tc in &resp.message.tool_calls {
                    info!(tool = %tc.name, "Invoking tool");
                    let result = match self.ctx.tools.get(&tc.name) {
                        Some(tool) => tool
                            .invoke(tc.arguments.clone())
                            .await
                            .unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() })),
                        None => {
                            warn!("Tool '{}' not found", tc.name);
                            serde_json::json!({ "error": format!("Tool '{}' not found", tc.name) })
                        }
                    };
                    messages.push(Message::tool_result(&tc.id, result));
                }
            } else {
                // Final answer
                let text = resp.message.content.clone();
                info!(tokens = total_usage.total_tokens, "ReAct complete");
                return Ok(AgentOutput {
                    task_id: task.id,
                    agent_id: self.id.clone(),
                    text,
                    tool_calls: resp.message.tool_calls,
                    approved: true,
                    usage: total_usage,
                });
            }
        }

        warn!("Max iterations ({MAX_ITERATIONS}) reached");
        Ok(AgentOutput {
            task_id: task.id,
            agent_id: self.id.clone(),
            text: "I reached the maximum number of reasoning steps. Please try a simpler request."
                .into(),
            tool_calls: vec![],
            approved: false,
            usage: total_usage,
        })
    }

    /// Run a streaming ReAct loop with conversation history.
    /// Returns a `BoxStream<StreamEvent>` that yields token deltas,
    /// tool call start/end events, and a final `Done` event.
    pub fn stream_with_history(
        &self,
        session_id: SessionId,
        task: AgentTask,
    ) -> BoxStream<StreamEvent> {
        let ctx = self.ctx.clone();
        let agent_id = self.id.clone();
        let (tx, rx) = mpsc::channel::<anyhow::Result<StreamEvent>>(64);

        tokio::spawn(async move {
            if let Err(e) =
                Self::stream_react_loop(ctx, agent_id, session_id, task, tx.clone()).await
            {
                let _ = tx
                    .send(Ok(StreamEvent::Error {
                        message: e.to_string(),
                    }))
                    .await;
            }
        });

        Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
    }

    /// Inner streaming ReAct loop that sends events through the channel.
    async fn stream_react_loop(
        ctx: AgentContext,
        agent_id: String,
        session_id: SessionId,
        task: AgentTask,
        tx: mpsc::Sender<anyhow::Result<StreamEvent>>,
    ) -> anyhow::Result<()> {
        let history = ctx.memory.history(&session_id, 50).await?;
        let mut messages: Vec<Message> = vec![Message::system(&ctx.config.agent.system_prompt)];
        messages.extend(history);
        messages.push(Message::user(&task.instruction));

        let provider = ctx
            .providers
            .resolve()
            .await
            .context("No provider available")?;

        let tool_schemas = match &task.tool_allowlist {
            Some(allow) => ctx.tools.filtered_schemas(allow),
            None => ctx.tools.all_schemas(),
        };

        for iteration in 0..MAX_ITERATIONS {
            debug!(iteration, "Streaming ReAct iteration");

            let req = CompletionRequest {
                messages: messages.clone(),
                tools: tool_schemas.clone(),
                max_tokens: task.max_tokens.or(Some(ctx.config.agent.max_tokens)),
                temperature: Some(ctx.config.agent.temperature),
                stream: true,
                model: None,
            };

            let mut stream = provider
                .stream(req)
                .await
                .with_context(|| format!("Provider stream error at iteration {iteration}"))?;

            // Accumulate text and tool call deltas from the stream
            let mut full_text = String::new();
            let mut tool_call_map: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
            let mut stop: Option<StopReason> = None;

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result?;

                // Forward text deltas
                if !chunk.delta.is_empty() {
                    full_text.push_str(&chunk.delta);
                    let _ = tx
                        .send(Ok(StreamEvent::TokenDelta { delta: chunk.delta }))
                        .await;
                }

                // Accumulate tool call deltas
                for tcd in &chunk.tool_calls {
                    let entry = tool_call_map
                        .entry(tcd.index)
                        .or_insert_with(|| (String::new(), String::new(), String::new()));
                    if let Some(id) = &tcd.id {
                        entry.0 = id.clone();
                    }
                    if let Some(name) = &tcd.name {
                        entry.1 = name.clone();
                    }
                    entry.2.push_str(&tcd.arguments_delta);
                }

                if let Some(reason) = chunk.stop_reason {
                    stop = Some(reason);
                }
            }

            let stop_reason = stop.unwrap_or(StopReason::EndTurn);

            if stop_reason == StopReason::ToolUse || !tool_call_map.is_empty() {
                // Reconstruct full ToolCalls
                let mut tool_calls = Vec::new();
                for (id, name, args_json) in tool_call_map.values() {
                    let arguments: serde_json::Value =
                        serde_json::from_str(args_json).unwrap_or(serde_json::json!({}));
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: arguments.clone(),
                    });
                }

                // Add assistant message to history
                let mut assistant_msg = Message::assistant(&full_text);
                assistant_msg.tool_calls = tool_calls.clone();
                messages.push(assistant_msg);

                // Execute each tool and emit events
                for tc in &tool_calls {
                    let _ = tx
                        .send(Ok(StreamEvent::ToolCallStart {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        }))
                        .await;

                    let result = match ctx.tools.get(&tc.name) {
                        Some(tool) => tool
                            .invoke(tc.arguments.clone())
                            .await
                            .unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() })),
                        None => {
                            warn!("Tool '{}' not found", tc.name);
                            serde_json::json!({ "error": format!("Tool '{}' not found", tc.name) })
                        }
                    };

                    messages.push(Message::tool_result(&tc.id, result.clone()));

                    let _ = tx
                        .send(Ok(StreamEvent::ToolCallEnd {
                            id: tc.id.clone(),
                            result,
                        }))
                        .await;
                }
            } else {
                // Final answer — done
                info!(agent_id, "Streaming ReAct complete");
                let _ = tx.send(Ok(StreamEvent::Done { usage: None })).await;
                return Ok(());
            }
        }

        warn!("Max iterations ({MAX_ITERATIONS}) reached in streaming ReAct");
        let _ = tx.send(Ok(StreamEvent::Done { usage: None })).await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Agent for ReActAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }
    fn role(&self) -> AgentRole {
        AgentRole::Worker
    }

    async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput> {
        let mut messages = vec![Message::system(&self.ctx.config.agent.system_prompt)];
        messages.extend(task.context.clone());
        messages.push(Message::user(&task.instruction));
        self.react_loop(task, messages).await
    }
}
