use std::sync::Arc;

use arc_swap::ArcSwap;
use ironclaw_config::IronClawConfig;
use ironclaw_core::MemoryStore;
use ironclaw_providers::ProviderRegistry;
use ironclaw_tools::ToolRegistry;

/// Shared runtime context — the dependency injection root.
/// Cheaply clonable (all Arc inside).
#[derive(Clone)]
pub struct AgentContext {
    pub config: Arc<ArcSwap<IronClawConfig>>,
    pub providers: Arc<ProviderRegistry>,
    pub tools: Arc<ToolRegistry>,
    pub memory: Arc<dyn MemoryStore>,
}

impl AgentContext {
    pub fn new(
        config: Arc<ArcSwap<IronClawConfig>>,
        providers: Arc<ProviderRegistry>,
        tools: Arc<ToolRegistry>,
        memory: Arc<dyn MemoryStore>,
    ) -> Self {
        Self {
            config,
            providers,
            tools,
            memory,
        }
    }

    /// Build a context from the loaded configuration.
    ///
    /// This is async because the memory backend may need to open a database.
    pub async fn from_config(cfg: IronClawConfig) -> anyhow::Result<Self> {
        let providers = Arc::new(ProviderRegistry::from_config(&cfg));
        let tools = Arc::new(ToolRegistry::from_config(&cfg));
        let memory = ironclaw_memory::from_config(&cfg).await?;
        let config = Arc::new(ArcSwap::from_pointee(cfg));
        Ok(Self::new(config, providers, tools, memory))
    }
}
