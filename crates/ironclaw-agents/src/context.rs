
use ironclaw_config::IronClawConfig;
use ironclaw_core::MemoryStore;
use ironclaw_providers::ProviderRegistry;
use ironclaw_tools::ToolRegistry;
use std::sync::Arc;

/// Shared runtime context — the dependency injection root.
/// Cheaply clonable (all Arc inside).
#[derive(Clone)]
pub struct AgentContext {
    pub config:    Arc<IronClawConfig>,
    pub providers: Arc<ProviderRegistry>,
    pub tools:     Arc<ToolRegistry>,
    pub memory:    Arc<dyn MemoryStore>,
}

impl AgentContext {
    pub fn new(
        config:    Arc<IronClawConfig>,
        providers: Arc<ProviderRegistry>,
        tools:     Arc<ToolRegistry>,
        memory:    Arc<dyn MemoryStore>,
    ) -> Self {
        Self { config, providers, tools, memory }
    }

    /// Build a context from the loaded configuration.
    ///
    /// This is async because the memory backend may need to open a database.
    pub async fn from_config(cfg: IronClawConfig) -> anyhow::Result<Self> {
        let cfg       = Arc::new(cfg);
        let providers = Arc::new(ProviderRegistry::from_config(&cfg));
        let tools     = Arc::new(ToolRegistry::from_config(&cfg));
        let memory    = ironclaw_memory::from_config(&cfg).await?;
        Ok(Self::new(cfg, providers, tools, memory))
    }
}
