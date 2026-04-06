use std::{collections::HashMap, sync::Arc};

use ironclaw_config::IronClawConfig;
use ironclaw_core::Provider;
use tracing::{info, warn};

pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
    fallback_chain: Vec<String>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            fallback_chain: vec![],
        }
    }

    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        let name = provider.name().to_string();
        info!("Registered provider: {name}");
        self.providers.insert(name, provider);
    }

    pub fn set_fallback_chain(&mut self, chain: Vec<String>) {
        self.fallback_chain = chain;
    }

    /// Returns first healthy provider in the chain.
    pub async fn resolve(&self) -> anyhow::Result<Arc<dyn Provider>> {
        for name in &self.fallback_chain {
            if let Some(p) = self.providers.get(name) {
                match p.health_check().await {
                    Ok(_) => {
                        info!("Using provider: {name}");
                        return Ok(Arc::clone(p));
                    }
                    Err(e) => {
                        warn!("Provider '{name}' unhealthy: {e}");
                    }
                }
            }
        }
        // If fallback chain fails, try any available
        for p in self.providers.values() {
            if p.health_check().await.is_ok() {
                return Ok(Arc::clone(p));
            }
        }
        anyhow::bail!("No healthy provider found")
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(name).map(Arc::clone)
    }

    /// Build registry from config
    pub fn from_config(cfg: &IronClawConfig) -> Self {
        let mut reg = Self::new();
        let mut chain = vec![cfg.providers.primary.clone()];
        chain.extend(cfg.providers.fallback.clone());
        reg.set_fallback_chain(chain);

        #[cfg(feature = "ollama")]
        {
            let c = &cfg.providers.ollama;
            reg.register(Arc::new(crate::OllamaProvider::new(&c.base_url, &c.model)));
        }
        #[cfg(feature = "anthropic")]
        if !cfg.providers.claude.api_key.is_empty() {
            let c = &cfg.providers.claude;
            reg.register(Arc::new(crate::AnthropicProvider::new(
                &c.api_key,
                &c.model,
                &c.base_url,
            )));
        }
        #[cfg(feature = "openai")]
        if !cfg.providers.openai.api_key.is_empty() {
            let c = &cfg.providers.openai;
            reg.register(Arc::new(crate::OpenAIProvider::new(
                &c.api_key,
                &c.model,
                &c.base_url,
            )));
        }
        #[cfg(feature = "groq")]
        if !cfg.providers.groq.api_key.is_empty() {
            let c = &cfg.providers.groq;
            reg.register(Arc::new(crate::GroqProvider::new(&c.api_key, &c.model)));
        }
        reg
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
