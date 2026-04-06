
use async_trait::async_trait;
use dashmap::DashMap;
use ironclaw_core::{Agent, AgentBus, AgentId, AgentOutput, AgentTask};
use std::sync::Arc;

/// Local in-process agent bus using DashMap.
pub struct LocalBus {
    agents: DashMap<AgentId, Arc<dyn Agent>>,
}

impl LocalBus {
    pub fn new() -> Self { Self { agents: DashMap::new() } }
}

impl Default for LocalBus {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl AgentBus for LocalBus {
    fn register(&self, agent: Arc<dyn Agent>) {
        self.agents.insert(agent.id().clone(), agent);
    }

    async fn dispatch(&self, id: &AgentId, task: AgentTask) -> anyhow::Result<AgentOutput> {
        let agent = self.agents.get(id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {id}"))?
            .clone();
        agent.run(task).await
    }
}
