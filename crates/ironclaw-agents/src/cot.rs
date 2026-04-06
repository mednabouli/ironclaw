use async_trait::async_trait;
use ironclaw_core::*;
use tracing::{debug, info};

use crate::context::AgentContext;

/// Forces a `<thinking>` step in the system prompt before generating the final answer.
///
/// The agent prepends a chain-of-thought instruction to the system prompt,
/// asking the model to reason inside `<thinking>...</thinking>` tags before
/// answering. The thinking block is then stripped from the final output.
pub struct ChainOfThoughtAgent {
    ctx: AgentContext,
    id: AgentId,
}

impl ChainOfThoughtAgent {
    /// Create a new chain-of-thought agent.
    pub fn new(ctx: AgentContext) -> Self {
        Self {
            ctx,
            id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// The system prompt that forces the model to think step-by-step.
    fn cot_system_prompt(base_prompt: &str) -> String {
        format!(
            "{base_prompt}\n\n\
             Before answering, you MUST think step-by-step inside <thinking>...</thinking> tags. \
             Analyze the problem, consider edge cases, and reason through your approach. \
             After the thinking block, provide your final answer outside the tags."
        )
    }

    /// Strip `<thinking>...</thinking>` blocks from response text and return
    /// `(thinking, answer)`.
    fn split_thinking(text: &str) -> (Option<String>, String) {
        if let Some(start) = text.find("<thinking>") {
            if let Some(end) = text.find("</thinking>") {
                let thinking = text[start + "<thinking>".len()..end].trim().to_string();
                let mut answer = String::new();
                answer.push_str(text[..start].trim());
                if !answer.is_empty() {
                    answer.push('\n');
                }
                answer.push_str(text[end + "</thinking>".len()..].trim());
                return (Some(thinking), answer);
            }
        }
        (None, text.to_string())
    }
}

#[async_trait]
impl Agent for ChainOfThoughtAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn role(&self) -> AgentRole {
        AgentRole::Worker
    }

    async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput> {
        let span = tracing::info_span!("cot.run", agent_id = %self.id);
        let _guard = span.enter();
        drop(_guard);

        let provider = self.ctx.providers.resolve().await?;

        let system = Self::cot_system_prompt(&self.ctx.config.agent.system_prompt);
        let mut messages = vec![Message::system(&system)];
        messages.extend(task.context.clone());
        messages.push(Message::user(&task.instruction));

        let req = CompletionRequest {
            messages,
            tools: vec![],
            max_tokens: task.max_tokens.or(Some(self.ctx.config.agent.max_tokens)),
            temperature: Some(self.ctx.config.agent.temperature),
            stream: false,
            model: None,
            response_format: Default::default(),
        };

        let resp = provider.complete(req).await?;
        let (thinking, answer) = Self::split_thinking(&resp.message.content);

        if let Some(ref thought) = thinking {
            debug!(thinking_len = thought.len(), "Chain-of-thought extracted");
        }

        info!(
            answer_len = answer.len(),
            has_thinking = thinking.is_some(),
            "CoT complete"
        );

        Ok(AgentOutput {
            task_id: task.id,
            agent_id: self.id.clone(),
            text: answer,
            tool_calls: vec![],
            approved: true,
            usage: resp.usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_thinking_extracts_thought_block() {
        let text = "<thinking>step 1, step 2</thinking>The final answer is 42.";
        let (thinking, answer) = ChainOfThoughtAgent::split_thinking(text);
        assert_eq!(thinking.unwrap(), "step 1, step 2");
        assert_eq!(answer, "The final answer is 42.");
    }

    #[test]
    fn split_thinking_no_tags_returns_full_text() {
        let text = "Just a plain answer.";
        let (thinking, answer) = ChainOfThoughtAgent::split_thinking(text);
        assert!(thinking.is_none());
        assert_eq!(answer, "Just a plain answer.");
    }

    #[test]
    fn split_thinking_with_content_before_tags() {
        let text = "Preamble <thinking>my thoughts</thinking> Final answer.";
        let (thinking, answer) = ChainOfThoughtAgent::split_thinking(text);
        assert_eq!(thinking.unwrap(), "my thoughts");
        assert!(answer.contains("Preamble"));
        assert!(answer.contains("Final answer."));
    }

    #[test]
    fn cot_system_prompt_includes_thinking_instruction() {
        let prompt = ChainOfThoughtAgent::cot_system_prompt("You are helpful.");
        assert!(prompt.contains("<thinking>"));
        assert!(prompt.contains("You are helpful."));
    }
}
