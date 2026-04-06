
use anyhow::Context;
use ironclaw_core::*;
use tracing::{debug, info, warn};
use crate::context::AgentContext;

const MAX_ITERATIONS: usize = 10;

pub struct ReActAgent {
    ctx: AgentContext,
    id:  AgentId,
}

impl ReActAgent {
    pub fn new(ctx: AgentContext) -> Self {
        Self { ctx, id: uuid::Uuid::new_v4().to_string() }
    }

    /// Run with history loaded from memory.
    pub async fn run_with_history(
        &self,
        session_id: &SessionId,
        task:       AgentTask,
    ) -> anyhow::Result<AgentOutput> {
        let history = self.ctx.memory.history(session_id, 50).await?;
        let mut messages: Vec<Message> = vec![
            Message::system(&self.ctx.config.agent.system_prompt),
        ];
        messages.extend(history);
        messages.push(Message::user(&task.instruction));

        self.react_loop(task, messages).await
    }

    async fn react_loop(&self, task: AgentTask, mut messages: Vec<Message>) -> anyhow::Result<AgentOutput> {
        let provider = self.ctx.providers.resolve().await
            .context("No provider available")?;

        let tool_schemas = match &task.tool_allowlist {
            Some(allow) => self.ctx.tools.filtered_schemas(allow),
            None        => self.ctx.tools.all_schemas(),
        };

        let mut total_usage = TokenUsage::default();

        for iteration in 0..MAX_ITERATIONS {
            debug!(iteration, "ReAct iteration");

            let req = CompletionRequest {
                messages:    messages.clone(),
                tools:       tool_schemas.clone(),
                max_tokens:  task.max_tokens.or(Some(self.ctx.config.agent.max_tokens)),
                temperature: Some(self.ctx.config.agent.temperature),
                stream:      false,
                model:       None,
            };

            let resp = provider.complete(req).await
                .with_context(|| format!("Provider error at iteration {iteration}"))?;

            total_usage.prompt_tokens     += resp.usage.prompt_tokens;
            total_usage.completion_tokens += resp.usage.completion_tokens;
            total_usage.total_tokens      += resp.usage.total_tokens;

            if resp.stop_reason == StopReason::ToolUse || !resp.message.tool_calls.is_empty() {
                // Add assistant message with tool calls
                messages.push(resp.message.clone());

                // Invoke each tool
                for tc in &resp.message.tool_calls {
                    info!(tool = %tc.name, "Invoking tool");
                    let result = match self.ctx.tools.get(&tc.name) {
                        Some(tool) => {
                            tool.invoke(tc.arguments.clone()).await
                                .unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() }))
                        }
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
                    task_id:    task.id,
                    agent_id:   self.id.clone(),
                    text,
                    tool_calls: resp.message.tool_calls,
                    approved:   true,
                    usage:      total_usage,
                });
            }
        }

        warn!("Max iterations ({MAX_ITERATIONS}) reached");
        Ok(AgentOutput {
            task_id:   task.id,
            agent_id:  self.id.clone(),
            text:      "I reached the maximum number of reasoning steps. Please try a simpler request.".into(),
            tool_calls: vec![],
            approved:  false,
            usage:     total_usage,
        })
    }
}

#[async_trait::async_trait]
impl Agent for ReActAgent {
    fn id(&self)   -> &AgentId   { &self.id }
    fn role(&self) -> AgentRole  { AgentRole::Worker }

    async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput> {
        let mut messages = vec![Message::system(&self.ctx.config.agent.system_prompt)];
        messages.extend(task.context.clone());
        messages.push(Message::user(&task.instruction));
        self.react_loop(task, messages).await
    }
}
