pub mod inmemory;
#[cfg(feature = "redis-backend")]
pub mod redis;
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(feature = "sqlite")]
pub mod vector;

use std::sync::Arc;

pub use inmemory::InMemoryStore;
use ironclaw_config::IronClawConfig;
use ironclaw_core::{MemoryStore, VectorStore};
#[cfg(feature = "redis-backend")]
pub use redis::RedisStore;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteStore;
#[cfg(feature = "sqlite")]
pub use vector::SqliteVectorStore;

/// Build a [`MemoryStore`] from the loaded configuration.
///
/// Branches on `cfg.memory.backend`:
/// - `"sqlite"` → persistent [`SqliteStore`] at `cfg.memory.path`
/// - `"redis"`  → distributed [`RedisStore`] at `cfg.memory.redis.url`
/// - anything else → ephemeral [`InMemoryStore`]
pub async fn from_config(cfg: &IronClawConfig) -> anyhow::Result<Arc<dyn MemoryStore>> {
    match cfg.memory.backend.as_str() {
        #[cfg(feature = "sqlite")]
        "sqlite" => {
            let path = expand_tilde(&cfg.memory.path);
            // Ensure parent directory exists
            if let Some(parent) = std::path::Path::new(&path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let store = SqliteStore::new(&path, cfg.memory.max_history).await?;
            Ok(Arc::new(store))
        }
        #[cfg(feature = "redis-backend")]
        "redis" => {
            let rc = &cfg.memory.redis;
            let store = RedisStore::new(&rc.url, &rc.key_prefix, rc.max_history).await?;
            Ok(Arc::new(store))
        }
        _ => Ok(Arc::new(InMemoryStore::new(cfg.memory.max_history))),
    }
}

/// Build a [`VectorStore`] from the loaded configuration.
///
/// The vector store uses the same SQLite path as the memory store
/// (with a `_vectors` suffix) to keep embeddings co-located.
#[cfg(feature = "sqlite")]
pub async fn vector_store_from_config(
    cfg: &IronClawConfig,
) -> anyhow::Result<Arc<dyn VectorStore>> {
    let base_path = expand_tilde(&cfg.memory.path);
    let vec_path = if base_path.ends_with(".db") {
        base_path.replace(".db", "_vectors.db")
    } else {
        format!("{base_path}_vectors.db")
    };

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(&vec_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let store = SqliteVectorStore::new(&vec_path, cfg.memory.embedding_dimensions).await?;
    Ok(Arc::new(store))
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
