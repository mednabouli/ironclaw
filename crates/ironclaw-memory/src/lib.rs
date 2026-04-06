pub mod inmemory;
pub mod sqlite;

use std::sync::Arc;

pub use inmemory::InMemoryStore;
use ironclaw_config::IronClawConfig;
use ironclaw_core::MemoryStore;
pub use sqlite::SqliteStore;

/// Build a [`MemoryStore`] from the loaded configuration.
///
/// Branches on `cfg.memory.backend`:
/// - `"sqlite"` → persistent [`SqliteStore`] at `cfg.memory.path`
/// - anything else → ephemeral [`InMemoryStore`]
pub async fn from_config(cfg: &IronClawConfig) -> anyhow::Result<Arc<dyn MemoryStore>> {
    match cfg.memory.backend.as_str() {
        "sqlite" => {
            let path = expand_tilde(&cfg.memory.path);
            // Ensure parent directory exists
            if let Some(parent) = std::path::Path::new(&path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let store = SqliteStore::new(&path, cfg.memory.max_history).await?;
            Ok(Arc::new(store))
        }
        _ => Ok(Arc::new(InMemoryStore::new(cfg.memory.max_history))),
    }
}

/// Replace a leading `~` with the user's home directory.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}{rest}");
        }
    }
    path.to_string()
}
