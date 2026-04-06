//! File-system watcher that hot-reloads `ironclaw.toml` into an `ArcSwap<IronClawConfig>`.
//!
//! # Usage
//!
//! ```no_run
//! use std::sync::Arc;
//! use arc_swap::ArcSwap;
//! use ironclaw_config::{IronClawConfig, ConfigWatcher};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = Arc::new(ArcSwap::from_pointee(IronClawConfig::from_file("ironclaw.toml")?));
//! let _watcher = ConfigWatcher::start("ironclaw.toml", config.clone())?;
//! // config.load() now always returns the latest version.
//! # Ok(())
//! # }
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arc_swap::ArcSwap;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::IronClawConfig;

/// Watches a config file and atomically swaps the `ArcSwap` on every write.
pub struct ConfigWatcher {
    /// Kept alive to maintain the watch — drop to stop watching.
    _watcher: RecommendedWatcher,
}

impl ConfigWatcher {
    /// Start watching `path` for modifications.
    ///
    /// Returns an error if the initial watch setup fails.
    /// Parse errors during hot-reload are logged and swallowed
    /// (the previous config remains active).
    pub fn start(
        path: impl AsRef<Path>,
        store: Arc<ArcSwap<IronClawConfig>>,
    ) -> anyhow::Result<Self> {
        let config_path: PathBuf = path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| path.as_ref().to_path_buf());

        let watched_path = config_path.clone();
        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        tracing::debug!(path = %watched_path.display(), "Config file changed, reloading");
                        match IronClawConfig::from_file(&watched_path) {
                            Ok(new_cfg) => {
                                store.store(Arc::new(new_cfg));
                                tracing::info!(path = %watched_path.display(), "Config hot-reloaded");
                            }
                            Err(e) => {
                                tracing::warn!(
                                    path = %watched_path.display(),
                                    error = %e,
                                    "Config reload failed — keeping previous config"
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Config watcher error");
                }
            },
        )?;

        watcher.watch(
            config_path.parent().unwrap_or_else(|| Path::new(".")),
            RecursiveMode::NonRecursive,
        )?;

        tracing::info!(path = %config_path.display(), "Config watcher started");
        Ok(Self { _watcher: watcher })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn watcher_creates_without_panic() {
        let dir = std::env::temp_dir().join("ironclaw_test_watcher");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ironclaw.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "[agent]\nname = \"test\"").unwrap();

        let cfg = IronClawConfig::from_file(&path).unwrap();
        let store = Arc::new(ArcSwap::from_pointee(cfg));
        let _w = ConfigWatcher::start(&path, store).unwrap();

        std::fs::remove_dir_all(dir).ok();
    }
}
