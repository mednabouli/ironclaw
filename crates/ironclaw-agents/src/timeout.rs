use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ironclaw_core::*;
use tracing::{info, warn};

/// Wraps any `Agent` and enforces a per-task timeout via `tokio::time::timeout`.
///
/// If the inner agent does not complete within `deadline`, the task is
/// cancelled and a `Failed` output is returned.
pub struct TimeoutAgent {
    inner: Arc<dyn Agent>,
    deadline: Duration,
}

impl TimeoutAgent {
    /// Create a new timeout wrapper around `inner` with the given `deadline`.
    pub fn new(inner: Arc<dyn Agent>, deadline: Duration) -> Self {
        Self { inner, deadline }
    }

    /// Return a reference to the wrapped agent.
    pub fn inner(&self) -> &Arc<dyn Agent> {
        &self.inner
    }
}

#[async_trait]
impl Agent for TimeoutAgent {
    fn id(&self) -> &AgentId {
        self.inner.id()
    }

    fn role(&self) -> AgentRole {
        self.inner.role()
    }

    async fn run(&self, task: AgentTask) -> Result<AgentOutput, AgentError> {
        let task_id = task.id;
        let agent_id = self.inner.id().clone();
        let deadline = self.deadline;

        info!(
            agent_id = %agent_id,
            deadline_ms = deadline.as_millis() as u64,
            "Starting task with timeout"
        );

        match tokio::time::timeout(deadline, self.inner.run(task)).await {
            Ok(result) => result,
            Err(_elapsed) => {
                warn!(
                    agent_id = %agent_id,
                    deadline_ms = deadline.as_millis() as u64,
                    "Agent task timed out"
                );
                Ok(AgentOutput::new(
                    task_id,
                    agent_id,
                    format!("Task timed out after {}ms", deadline.as_millis()),
                )
                .with_approved(false))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SlowAgent {
        id: AgentId,
    }

    #[async_trait]
    impl Agent for SlowAgent {
        fn id(&self) -> &AgentId {
            &self.id
        }
        fn role(&self) -> AgentRole {
            AgentRole::Worker
        }
        async fn run(&self, task: AgentTask) -> Result<AgentOutput, AgentError> {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(AgentOutput::new(task.id, self.id.clone(), "done").with_approved(true))
        }
    }

    struct FastAgent {
        id: AgentId,
    }

    #[async_trait]
    impl Agent for FastAgent {
        fn id(&self) -> &AgentId {
            &self.id
        }
        fn role(&self) -> AgentRole {
            AgentRole::Worker
        }
        async fn run(&self, task: AgentTask) -> Result<AgentOutput, AgentError> {
            Ok(AgentOutput::new(task.id, self.id.clone(), "fast result").with_approved(true))
        }
    }

    #[tokio::test]
    async fn timeout_cancels_slow_agent() {
        let slow = Arc::new(SlowAgent {
            id: "slow-1".into(),
        });
        let agent = TimeoutAgent::new(slow, Duration::from_millis(50));
        let task = AgentTask::new("do something slow");
        let output = agent.run(task).await.unwrap();
        assert!(!output.approved);
        assert!(output.text.contains("timed out"));
    }

    #[tokio::test]
    async fn timeout_passes_fast_agent() {
        let fast = Arc::new(FastAgent {
            id: "fast-1".into(),
        });
        let agent = TimeoutAgent::new(fast, Duration::from_secs(5));
        let task = AgentTask::new("do something fast");
        let output = agent.run(task).await.unwrap();
        assert!(output.approved);
        assert_eq!(output.text, "fast result");
    }

    #[test]
    fn timeout_agent_delegates_id_and_role() {
        let fast = Arc::new(FastAgent {
            id: "inner-1".into(),
        });
        let agent = TimeoutAgent::new(fast, Duration::from_secs(5));
        assert_eq!(agent.id().as_str(), "inner-1");
        assert!(matches!(agent.role(), AgentRole::Worker));
    }
}
