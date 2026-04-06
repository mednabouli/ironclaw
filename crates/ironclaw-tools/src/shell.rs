
use async_trait::async_trait;
use ironclaw_core::{Tool, ToolSchema};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::warn;

pub struct ShellTool {
    allowlist:    Vec<String>,
    timeout_secs: u64,
}

impl ShellTool {
    pub fn new(allowlist: Vec<String>, timeout_secs: u64) -> Self {
        Self { allowlist, timeout_secs }
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str { "shell" }
    fn description(&self) -> &str { "Execute an allowlisted shell command and return stdout." }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to run" },
                    "args":    { "type": "array", "items": { "type": "string" }, "description": "Command arguments", "default": [] }
                },
                "required": ["command"]
            }),
        }
    }

    async fn invoke(&self, params: Value) -> anyhow::Result<Value> {
        let cmd = params["command"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' parameter"))?;

        if !self.allowlist.is_empty() && !self.allowlist.iter().any(|a| a == cmd) {
            warn!("Blocked shell command: {cmd}");
            anyhow::bail!("Command '{cmd}' is not in the allowlist");
        }

        let args: Vec<&str> = params["args"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        let output = timeout(
            Duration::from_secs(self.timeout_secs),
            Command::new(cmd).args(&args).output(),
        ).await
        .map_err(|_| anyhow::anyhow!("Command timed out after {}s", self.timeout_secs))?
        .map_err(|e| anyhow::anyhow!("Command failed: {e}"))?;

        Ok(json!({
            "stdout":    String::from_utf8_lossy(&output.stdout).trim().to_string(),
            "stderr":    String::from_utf8_lossy(&output.stderr).trim().to_string(),
            "exit_code": output.status.code().unwrap_or(-1),
        }))
    }
}
