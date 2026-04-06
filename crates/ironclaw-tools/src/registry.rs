
use ironclaw_core::{Tool, ToolSchema};
use std::{collections::HashMap, sync::Arc};
use ironclaw_config::IronClawConfig;
use tracing::info;

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self { Self { tools: HashMap::new() } }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        info!("Registered tool: {}", tool.name());
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).map(Arc::clone)
    }

    pub fn all_schemas(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|t| t.schema()).collect()
    }

    pub fn filtered_schemas(&self, allowlist: &[String]) -> Vec<ToolSchema> {
        if allowlist.is_empty() {
            return self.all_schemas();
        }
        self.tools.iter()
            .filter(|(k, _)| allowlist.contains(k))
            .map(|(_, v)| v.schema())
            .collect()
    }

    pub fn from_config(cfg: &IronClawConfig) -> Self {
        let mut reg = Self::new();
        for name in &cfg.tools.enabled {
            match name.as_str() {
                "datetime" => reg.register(Arc::new(crate::datetime::DateTimeTool)),
                "shell"    => reg.register(Arc::new(crate::shell::ShellTool::new(
                    cfg.tools.shell.allowlist.clone(),
                    cfg.tools.shell.timeout_secs,
                ))),
                other => tracing::warn!("Unknown tool in config: {other}"),
            }
        }
        reg
    }
}

impl Default for ToolRegistry {
    fn default() -> Self { Self::new() }
}
