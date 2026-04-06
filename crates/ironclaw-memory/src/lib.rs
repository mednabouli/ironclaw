
pub mod inmemory;
pub use inmemory::InMemoryStore;

use ironclaw_config::IronClawConfig;
use ironclaw_core::MemoryStore;
use std::sync::Arc;

pub fn from_config(cfg: &IronClawConfig) -> Arc<dyn MemoryStore> {
    // Future: match cfg.memory.backend { "sqlite" => ..., "redis" => ... }
    Arc::new(InMemoryStore::new(cfg.memory.max_history))
}
