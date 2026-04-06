use std::{collections::HashMap, sync::Arc};

use ironclaw_config::IronClawConfig;
use ironclaw_core::{Tool, ToolSchema};
use tracing::info;

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

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
        self.tools
            .iter()
            .filter(|(k, _)| allowlist.contains(k))
            .map(|(_, v)| v.schema())
            .collect()
    }

    pub fn from_config(cfg: &IronClawConfig) -> Self {
        let mut reg = Self::new();
        for name in &cfg.tools.enabled {
            match name.as_str() {
                "datetime" => reg.register(Arc::new(crate::datetime::DateTimeTool)),
                "shell" => reg.register(Arc::new(crate::shell::ShellTool::new(
                    cfg.tools.shell.allowlist.clone(),
                    cfg.tools.shell.timeout_secs,
                ))),
                "calculator" => reg.register(Arc::new(crate::calculator::CalculatorTool)),
                "web_search" => reg.register(Arc::new(crate::websearch::WebSearchTool::new())),
                "file_read" => reg.register(Arc::new(crate::fileread::FileReadTool::new(
                    cfg.tools
                        .file_allowed_dirs
                        .iter()
                        .map(std::path::PathBuf::from)
                        .collect(),
                ))),
                "file_write" => reg.register(Arc::new(crate::filewrite::FileWriteTool::new(
                    cfg.tools
                        .file_allowed_dirs
                        .iter()
                        .map(std::path::PathBuf::from)
                        .collect(),
                ))),
                "http_get" => reg.register(Arc::new(crate::httpget::HttpGetTool::new())),
                "cron" => reg.register(Arc::new(crate::cron::CronTool::new())),
                other => tracing::warn!("Unknown tool in config: {other}"),
            }
        }
        reg
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
