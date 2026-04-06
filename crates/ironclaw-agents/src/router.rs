use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use ironclaw_core::*;
use tracing::{debug, info};

use crate::context::AgentContext;

/// Routes incoming tasks to specialist agents based on LLM intent classification.
///
/// The router sends a classification prompt to the provider, which returns
/// a route name. The task is then dispatched to the matching specialist agent.
pub struct RouterAgent {
    ctx: AgentContext,
    id: AgentId,
    routes: HashMap<String, Arc<dyn Agent>>,
    fallback: Option<Arc<dyn Agent>>,
}

impl RouterAgent {
    /// Create a new router agent.
    pub fn new(ctx: AgentContext) -> Self {
        Self {
            ctx,
            id: uuid::Uuid::new_v4().to_string(),
            routes: HashMap::new(),
            fallback: None,
        }
    }

    /// Register a named route pointing to a specialist agent.
    pub fn add_route(mut self, name: impl Into<String>, agent: Arc<dyn Agent>) -> Self {
        self.routes.insert(name.into(), agent);
        self
    }

    /// Set a fallback agent for unrecognised routes.
    pub fn with_fallback(mut self, agent: Arc<dyn Agent>) -> Self {
        self.fallback = Some(agent);
        self
    }

    /// Build the classification prompt listing available routes.
    fn classification_prompt(&self, instruction: &str) -> String {
        let route_list: Vec<&str> = self.routes.keys().map(|s| s.as_str()).collect();
        format!(
            "You are a routing classifier. Given the user request below, respond with EXACTLY \
             one of the following route names and nothing else:\n\n\
             Routes: {}\n\n\
             User request: {}\n\n\
             Route:",
            route_list.join(", "),
            instruction
        )
    }
}

#[async_trait]
impl Agent for RouterAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn role(&self) -> AgentRole {
        AgentRole::Router
    }

    async fn run(&self, task: AgentTask) -> anyhow::Result<AgentOutput> {
        let span = tracing::info_span!("router.classify", agent_id = %self.id);
        let _guard = span.enter();
        drop(_guard);

        let provider = self.ctx.providers.resolve().await?;

        let classify_prompt = self.classification_prompt(&task.instruction);
        debug!(prompt = %classify_prompt, "Classifying intent");

        let req = CompletionRequest {
            messages: vec![Message::user(&classify_prompt)],
            tools: vec![],
            max_tokens: Some(50),
            temperature: Some(0.0),
            stream: false,
            model: None,
            response_format: Default::default(),
        };

        let resp = provider.complete(req).await?;
        let route_name = resp.message.content.trim().to_lowercase();
        info!(route = %route_name, "Classified intent");

        // Look up the specialist agent
        let specialist = self
            .routes
            .get(&route_name)
            .or(self.fallback.as_ref())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No route matched '{}' and no fallback configured",
                    route_name
                )
            })?;

        // Dispatch the original task to the specialist
        specialist.run(task).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoAgent {
        id: AgentId,
        prefix: String,
    }

    #[async_trait]
    impl Agent for EchoAgent {
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
                text: format!("{}: {}", self.prefix, task.instruction),
                tool_calls: vec![],
                approved: true,
                usage: TokenUsage::default(),
            })
        }
    }

    #[test]
    fn router_role_is_router() {
        let cfg = ironclaw_config::IronClawConfig::default();
        let reg = ironclaw_providers::ProviderRegistry::new();
        let tools = Arc::new(ironclaw_tools::ToolRegistry::from_config(&cfg));
        let memory = Arc::new(ironclaw_memory::InMemoryStore::new(100));
        let config = Arc::new(arc_swap::ArcSwap::from_pointee(cfg));
        let ctx = AgentContext::new(config, Arc::new(reg), tools, memory);
        let router = RouterAgent::new(ctx);
        assert!(matches!(router.role(), AgentRole::Router));
    }

    #[test]
    fn classification_prompt_lists_routes() {
        let cfg = ironclaw_config::IronClawConfig::default();
        let reg = ironclaw_providers::ProviderRegistry::new();
        let tools = Arc::new(ironclaw_tools::ToolRegistry::from_config(&cfg));
        let memory = Arc::new(ironclaw_memory::InMemoryStore::new(100));
        let config = Arc::new(arc_swap::ArcSwap::from_pointee(cfg));
        let ctx = AgentContext::new(config, Arc::new(reg), tools, memory);

        let agent_a = Arc::new(EchoAgent {
            id: "a".into(),
            prefix: "A".into(),
        });
        let router = RouterAgent::new(ctx).add_route("code", agent_a);
        let prompt = router.classification_prompt("write a function");
        assert!(prompt.contains("code"));
        assert!(prompt.contains("write a function"));
    }
}
