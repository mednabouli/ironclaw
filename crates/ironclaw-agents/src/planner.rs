use std::sync::Arc;

use async_trait::async_trait;
use ironclaw_core::*;
use tracing::{debug, info, warn};

use crate::context::AgentContext;

/// Decomposes a complex task into subtasks using an LLM, then fans out
/// to worker agents and aggregates results.
///
/// The planner asks the provider to break the task into numbered subtasks,
/// dispatches each subtask to a worker agent (concurrently when possible),
/// and combines the results into a final output.
pub struct PlannerAgent {
    ctx: AgentContext,
    id: AgentId,
    worker: Arc<dyn Agent>,
    max_subtasks: usize,
}

impl PlannerAgent {
    /// Create a new planner agent.
    ///
    /// `worker` is the agent that handles individual subtasks.
    /// `max_subtasks` caps the number of subtasks the planner generates.
    pub fn new(ctx: AgentContext, worker: Arc<dyn Agent>, max_subtasks: usize) -> Self {
        Self {
            ctx,
            id: uuid::Uuid::new_v4().to_string(),
            worker,
            max_subtasks: max_subtasks.max(1),
        }
    }

    /// Build the planning prompt.
    fn planning_prompt(instruction: &str, max_subtasks: usize) -> String {
        format!(
            "You are a task planner. Break the following task into at most {max_subtasks} \
             independent subtasks. Output ONLY a numbered list, one subtask per line. \
             Each line must start with the number and a period (e.g. '1. ...'). \
             Do not include any other text.\n\n\
             Task: {instruction}"
        )
    }

    /// Parse numbered subtasks from the planner output.
    fn parse_subtasks(text: &str) -> Vec<String> {
        text.lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                // Match lines starting with a digit followed by . or )
                if trimmed.is_empty() {
                    return None;
                }
                let first_char = trimmed.chars().next()?;
                if first_char.is_ascii_digit() {
                    // Strip "1. " or "1) " prefix
                    let rest = trimmed
                        .trim_start_matches(|c: char| c.is_ascii_digit())
                        .trim_start_matches(['.', ')'])
                        .trim();
                    if rest.is_empty() {
                        None
                    } else {
                        Some(rest.to_string())
                    }
                } else {
                    None
                }
            })
            .collect()
    }
}

#[async_trait]
impl Agent for PlannerAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn role(&self) -> AgentRole {
        AgentRole::Planner
    }

    async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput> {
        let span = tracing::info_span!(
            "planner.run",
            agent_id = %self.id,
            max_subtasks = self.max_subtasks,
        );
        let _guard = span.enter();
        drop(_guard);

        let provider = self.ctx.providers.resolve().await?;

        // Step 1: Ask LLM to decompose the task
        let plan_prompt = Self::planning_prompt(&task.instruction, self.max_subtasks);
        debug!(prompt = %plan_prompt, "Planning subtasks");

        let req = CompletionRequest {
            messages: vec![Message::user(&plan_prompt)],
            tools: vec![],
            max_tokens: Some(500),
            temperature: Some(0.3),
            stream: false,
            model: None,
            response_format: Default::default(),
        };

        let resp = provider.complete(req).await?;
        let mut subtasks = Self::parse_subtasks(&resp.message.content);

        // Cap to max_subtasks
        subtasks.truncate(self.max_subtasks);

        if subtasks.is_empty() {
            warn!("Planner produced no subtasks, running task directly");
            return self.worker.run(task).await;
        }

        info!(count = subtasks.len(), "Subtasks planned");

        // Step 2: Fan out subtasks to worker agents concurrently
        let mut handles = Vec::new();
        for (i, subtask_instruction) in subtasks.iter().enumerate() {
            let worker = Arc::clone(&self.worker);
            let sub_task = AgentTask {
                id: uuid::Uuid::new_v4(),
                instruction: subtask_instruction.clone(),
                context: task.context.clone(),
                tool_allowlist: task.tool_allowlist.clone(),
                max_tokens: task.max_tokens,
            };
            debug!(subtask = i + 1, instruction = %subtask_instruction, "Dispatching subtask");
            handles.push(tokio::spawn(async move { worker.run(sub_task).await }));
        }

        // Step 3: Collect results
        let mut all_text = Vec::new();
        let mut total_usage = resp.usage; // include planning tokens
        let mut all_approved = true;

        for (i, handle) in handles.into_iter().enumerate() {
            match handle.await {
                Ok(Ok(output)) => {
                    all_text.push(format!("## Subtask {}\n{}", i + 1, output.text));
                    total_usage.prompt_tokens += output.usage.prompt_tokens;
                    total_usage.completion_tokens += output.usage.completion_tokens;
                    total_usage.total_tokens += output.usage.total_tokens;
                    if !output.approved {
                        all_approved = false;
                    }
                }
                Ok(Err(e)) => {
                    warn!(subtask = i + 1, error = %e, "Subtask failed");
                    all_text.push(format!("## Subtask {} (FAILED)\n{}", i + 1, e));
                    all_approved = false;
                }
                Err(e) => {
                    warn!(subtask = i + 1, error = %e, "Subtask panicked");
                    all_text.push(format!("## Subtask {} (PANIC)\n{}", i + 1, e));
                    all_approved = false;
                }
            }
        }

        let combined = all_text.join("\n\n");
        info!(
            total_subtasks = subtasks.len(),
            result_len = combined.len(),
            "Planner complete"
        );

        Ok(AgentOutput {
            task_id: task.id,
            agent_id: self.id.clone(),
            text: combined,
            tool_calls: vec![],
            approved: all_approved,
            usage: total_usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_subtasks_numbered_list() {
        let text = "1. Do first thing\n2. Do second thing\n3. Do third thing\n";
        let subtasks = PlannerAgent::parse_subtasks(text);
        assert_eq!(subtasks.len(), 3);
        assert_eq!(subtasks[0], "Do first thing");
        assert_eq!(subtasks[1], "Do second thing");
        assert_eq!(subtasks[2], "Do third thing");
    }

    #[test]
    fn parse_subtasks_with_parentheses() {
        let text = "1) First\n2) Second\n";
        let subtasks = PlannerAgent::parse_subtasks(text);
        assert_eq!(subtasks.len(), 2);
        assert_eq!(subtasks[0], "First");
    }

    #[test]
    fn parse_subtasks_ignores_non_numbered() {
        let text = "Here are the subtasks:\n1. First\nSome note\n2. Second\n";
        let subtasks = PlannerAgent::parse_subtasks(text);
        assert_eq!(subtasks.len(), 2);
    }

    #[test]
    fn parse_subtasks_empty_returns_empty() {
        let text = "No numbered lines here.";
        let subtasks = PlannerAgent::parse_subtasks(text);
        assert!(subtasks.is_empty());
    }

    #[test]
    fn planning_prompt_includes_instruction() {
        let prompt = PlannerAgent::planning_prompt("build a website", 5);
        assert!(prompt.contains("build a website"));
        assert!(prompt.contains("5"));
    }

    struct EchoWorker {
        id: AgentId,
    }

    #[async_trait]
    impl Agent for EchoWorker {
        fn id(&self) -> &AgentId {
            &self.id
        }
        fn role(&self) -> AgentRole {
            AgentRole::Worker
        }
        async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput> {
            Ok(AgentOutput {
                task_id: task.id,
                agent_id: self.id.clone(),
                text: format!("Done: {}", task.instruction),
                tool_calls: vec![],
                approved: true,
                usage: TokenUsage::default(),
            })
        }
    }

    #[test]
    fn planner_role_is_planner() {
        let cfg = Arc::new(ironclaw_config::IronClawConfig::default());
        let reg = ironclaw_providers::ProviderRegistry::new();
        let tools = Arc::new(ironclaw_tools::ToolRegistry::from_config(&cfg));
        let memory = Arc::new(ironclaw_memory::InMemoryStore::new(100));
        let ctx = AgentContext::new(cfg, Arc::new(reg), tools, memory);
        let worker = Arc::new(EchoWorker { id: "w".into() });
        let planner = PlannerAgent::new(ctx, worker, 5);
        assert!(matches!(planner.role(), AgentRole::Planner));
    }
}
