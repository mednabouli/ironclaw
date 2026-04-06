use std::sync::Arc;

use async_trait::async_trait;
use ironclaw_core::*;
use tracing::{debug, info, warn};

use crate::context::AgentContext;

/// Pairs an actor agent with a critic agent for iterative refinement.
///
/// The actor proposes an answer, the critic reviews it. If the critic
/// rejects (sets `approved = false`), the actor tries again with the
/// criticism as additional context. Loops up to `max_rounds`.
pub struct CriticActorPair {
    #[allow(dead_code)]
    ctx: AgentContext,
    id: AgentId,
    actor: Arc<dyn Agent>,
    critic: Arc<dyn Agent>,
    max_rounds: usize,
}

impl CriticActorPair {
    /// Create a new critic-actor pair.
    ///
    /// `max_rounds` is the maximum number of actor→critic cycles before
    /// returning the last actor output regardless of approval.
    pub fn new(
        ctx: AgentContext,
        actor: Arc<dyn Agent>,
        critic: Arc<dyn Agent>,
        max_rounds: usize,
    ) -> Self {
        Self {
            ctx,
            id: uuid::Uuid::new_v4().to_string(),
            actor,
            critic,
            max_rounds: max_rounds.max(1),
        }
    }
}

#[async_trait]
impl Agent for CriticActorPair {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn role(&self) -> AgentRole {
        AgentRole::Critic
    }

    async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput> {
        let span = tracing::info_span!(
            "critic_actor.run",
            agent_id = %self.id,
            max_rounds = self.max_rounds,
        );
        let _guard = span.enter();
        drop(_guard);

        let mut actor_task = task.clone();
        let mut last_output: Option<AgentOutput> = None;

        for round in 1..=self.max_rounds {
            debug!(round, "Actor round");

            // Actor proposes
            let actor_output = self.actor.run(actor_task.clone()).await?;
            info!(
                round,
                actor_id = %actor_output.agent_id,
                text_len = actor_output.text.len(),
                "Actor proposed"
            );

            // Critic reviews: send the actor's output as a review task
            let critic_instruction = format!(
                "Review the following response for correctness, completeness, and quality. \
                 If the response is acceptable, respond with APPROVED. \
                 Otherwise, provide specific, actionable feedback for improvement.\n\n\
                 Original task: {}\n\n\
                 Response to review:\n{}",
                actor_task.instruction, actor_output.text
            );

            let critic_task = AgentTask {
                id: uuid::Uuid::new_v4(),
                instruction: critic_instruction,
                context: vec![],
                tool_allowlist: None,
                max_tokens: actor_task.max_tokens,
            };

            let critic_output = self.critic.run(critic_task).await?;
            let approved = critic_output.text.to_uppercase().contains("APPROVED");

            info!(
                round,
                approved,
                critic_id = %critic_output.agent_id,
                "Critic verdict"
            );

            if approved {
                return Ok(AgentOutput {
                    task_id: task.id,
                    agent_id: self.id.clone(),
                    text: actor_output.text,
                    tool_calls: actor_output.tool_calls,
                    approved: true,
                    usage: TokenUsage {
                        prompt_tokens: actor_output.usage.prompt_tokens
                            + critic_output.usage.prompt_tokens,
                        completion_tokens: actor_output.usage.completion_tokens
                            + critic_output.usage.completion_tokens,
                        total_tokens: actor_output.usage.total_tokens
                            + critic_output.usage.total_tokens,
                    },
                });
            }

            // Feed criticism back to actor for next round
            actor_task = AgentTask {
                id: uuid::Uuid::new_v4(),
                instruction: format!(
                    "{}\n\nPrevious attempt:\n{}\n\nCritic feedback:\n{}",
                    task.instruction, actor_output.text, critic_output.text
                ),
                context: actor_task.context.clone(),
                tool_allowlist: actor_task.tool_allowlist.clone(),
                max_tokens: actor_task.max_tokens,
            };

            last_output = Some(actor_output);
        }

        // Exhausted all rounds — return last actor output as not-approved
        warn!(max_rounds = self.max_rounds, "Critic never approved");
        let output = last_output.ok_or_else(|| anyhow::anyhow!("No actor output produced"))?;
        Ok(AgentOutput {
            task_id: task.id,
            agent_id: self.id.clone(),
            text: output.text,
            tool_calls: output.tool_calls,
            approved: false,
            usage: output.usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Actor that always returns the same text.
    struct StubActor {
        id: AgentId,
        answer: String,
    }

    #[async_trait]
    impl Agent for StubActor {
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
                text: self.answer.clone(),
                tool_calls: vec![],
                approved: true,
                usage: TokenUsage::default(),
            })
        }
    }

    /// Critic that always approves.
    struct ApproveCritic {
        id: AgentId,
    }

    #[async_trait]
    impl Agent for ApproveCritic {
        fn id(&self) -> &AgentId {
            &self.id
        }
        fn role(&self) -> AgentRole {
            AgentRole::Critic
        }
        async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput> {
            Ok(AgentOutput {
                task_id: task.id,
                agent_id: self.id.clone(),
                text: "APPROVED — looks great!".into(),
                tool_calls: vec![],
                approved: true,
                usage: TokenUsage::default(),
            })
        }
    }

    /// Critic that always rejects.
    struct RejectCritic {
        id: AgentId,
    }

    #[async_trait]
    impl Agent for RejectCritic {
        fn id(&self) -> &AgentId {
            &self.id
        }
        fn role(&self) -> AgentRole {
            AgentRole::Critic
        }
        async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput> {
            Ok(AgentOutput {
                task_id: task.id,
                agent_id: self.id.clone(),
                text: "Not good enough, try harder.".into(),
                tool_calls: vec![],
                approved: false,
                usage: TokenUsage::default(),
            })
        }
    }

    fn make_ctx() -> AgentContext {
        let cfg = ironclaw_config::IronClawConfig::default();
        let reg = ironclaw_providers::ProviderRegistry::new();
        let tools = Arc::new(ironclaw_tools::ToolRegistry::from_config(&cfg));
        let memory = Arc::new(ironclaw_memory::InMemoryStore::new(100));
        let config = Arc::new(arc_swap::ArcSwap::from_pointee(cfg));
        AgentContext::new(config, Arc::new(reg), tools, memory)
    }

    #[tokio::test]
    async fn approved_on_first_round() {
        let actor = Arc::new(StubActor {
            id: "actor-1".into(),
            answer: "my answer".into(),
        });
        let critic = Arc::new(ApproveCritic {
            id: "critic-1".into(),
        });
        let pair = CriticActorPair::new(make_ctx(), actor, critic, 3);
        let task = AgentTask::new("solve this");
        let output = pair.run(task).await.unwrap();
        assert!(output.approved);
        assert_eq!(output.text, "my answer");
    }

    #[tokio::test]
    async fn exhausted_rounds_returns_not_approved() {
        let actor = Arc::new(StubActor {
            id: "actor-2".into(),
            answer: "my answer".into(),
        });
        let critic = Arc::new(RejectCritic {
            id: "critic-2".into(),
        });
        let pair = CriticActorPair::new(make_ctx(), actor, critic, 2);
        let task = AgentTask::new("solve this");
        let output = pair.run(task).await.unwrap();
        assert!(!output.approved);
    }

    #[test]
    fn pair_role_is_critic() {
        let actor = Arc::new(StubActor {
            id: "a".into(),
            answer: String::new(),
        });
        let critic = Arc::new(ApproveCritic { id: "c".into() });
        let pair = CriticActorPair::new(make_ctx(), actor, critic, 1);
        assert!(matches!(pair.role(), AgentRole::Critic));
    }
}
