use std::time::Duration;

use async_trait::async_trait;
use ironclaw_core::{Tool, ToolError, ToolSchema};
use serde_json::{json, Value};
use tokio::{process::Command, time::timeout};
use tracing::warn;

/// Shell metacharacters that must never appear in arguments.
/// These could be exploited if any downstream code pipes through a shell.
const SHELL_METACHARS: &[char] = &[
    ';', '|', '&', '$', '`', '(', ')', '{', '}', '<', '>', '\n', '\r', '\0',
];

/// Execute allowlisted shell commands with timeout enforcement.
///
/// Security properties:
/// - **Default-deny**: if the allowlist is empty, all commands are blocked.
/// - **Exact match**: the command must match an allowlist entry exactly.
/// - **No shell**: commands are executed directly via `tokio::process::Command`,
///   bypassing the system shell (no `sh -c`).
/// - **Argument sanitization**: shell metacharacters are rejected in arguments.
/// - **Clean environment**: child process inherits only `PATH` and `HOME`.
/// - **Timeout**: configurable hard deadline on execution.
pub struct ShellTool {
    allowlist: Vec<String>,
    timeout_secs: u64,
}

impl ShellTool {
    /// Create a new shell tool.
    ///
    /// If `allowlist` is empty, **all commands are blocked** (default-deny).
    pub fn new(allowlist: Vec<String>, timeout_secs: u64) -> Self {
        Self {
            allowlist,
            timeout_secs,
        }
    }

    /// Check that the command is in the allowlist.
    fn is_allowed(&self, cmd: &str) -> bool {
        self.allowlist.iter().any(|a| a == cmd)
    }

    /// Validate that an argument does not contain shell metacharacters.
    fn validate_arg(arg: &str) -> bool {
        !arg.contains(SHELL_METACHARS)
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }
    fn description(&self) -> &str {
        "Execute an allowlisted shell command and return stdout."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            self.name(),
            self.description(),
            json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to run (must be in the allowlist)" },
                    "args":    { "type": "array", "items": { "type": "string" }, "description": "Command arguments (no shell metacharacters allowed)", "default": [] }
                },
                "required": ["command"]
            }),
        )
    }

    async fn invoke(&self, params: Value) -> Result<Value, ToolError> {
        (async move {
            let cmd = params["command"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'command' parameter"))?;

            // Default-deny: empty allowlist blocks everything
            if !self.is_allowed(cmd) {
                warn!(command = cmd, "Blocked shell command: not in allowlist");
                anyhow::bail!("Command '{cmd}' is not in the allowlist");
            }

            // Reject path separators in command name to prevent path traversal
            if cmd.contains('/') || cmd.contains('\\') {
                warn!(
                    command = cmd,
                    "Blocked shell command: path separators not allowed"
                );
                anyhow::bail!("Command must be a bare name, not a path: '{cmd}'");
            }

            let args: Vec<&str> = params["args"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();

            // Validate all arguments for shell metacharacters
            for arg in &args {
                if !Self::validate_arg(arg) {
                    warn!(
                        arg,
                        command = cmd,
                        "Blocked shell argument: contains metacharacters"
                    );
                    anyhow::bail!("Argument contains forbidden shell metacharacters: '{arg}'");
                }
            }

            let output = timeout(
                Duration::from_secs(self.timeout_secs),
                Command::new(cmd)
                    .args(&args)
                    // Clean environment: only inherit PATH and HOME
                    .env_clear()
                    .env("PATH", std::env::var("PATH").unwrap_or_default())
                    .env("HOME", std::env::var("HOME").unwrap_or_default())
                    .output(),
            )
            .await
            .map_err(|_| anyhow::anyhow!("Command timed out after {}s", self.timeout_secs))?
            .map_err(|e| anyhow::anyhow!("Command failed: {e}"))?;

            Ok(json!({
                "stdout":    String::from_utf8_lossy(&output.stdout).trim().to_string(),
                "stderr":    String::from_utf8_lossy(&output.stderr).trim().to_string(),
                "exit_code": output.status.code().unwrap_or(-1),
            }))
        })
        .await
        .map_err(Into::into)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_valid() {
        let tool = ShellTool::new(vec!["echo".into()], 10);
        assert_eq!(tool.name(), "shell");
        assert!(tool.schema().parameters["properties"]["command"].is_object());
    }

    #[test]
    fn empty_allowlist_blocks_all() {
        let tool = ShellTool::new(vec![], 10);
        assert!(!tool.is_allowed("ls"));
        assert!(!tool.is_allowed("echo"));
    }

    #[test]
    fn allowlist_permits_listed_commands() {
        let tool = ShellTool::new(vec!["echo".into(), "date".into()], 10);
        assert!(tool.is_allowed("echo"));
        assert!(tool.is_allowed("date"));
        assert!(!tool.is_allowed("rm"));
        assert!(!tool.is_allowed("bash"));
    }

    #[test]
    fn metacharacter_detection() {
        assert!(ShellTool::validate_arg("hello"));
        assert!(ShellTool::validate_arg("file.txt"));
        assert!(ShellTool::validate_arg("--flag=value"));
        assert!(ShellTool::validate_arg("-n"));
        assert!(!ShellTool::validate_arg("foo;bar"));
        assert!(!ShellTool::validate_arg("$(whoami)"));
        assert!(!ShellTool::validate_arg("a|b"));
        assert!(!ShellTool::validate_arg("a&b"));
        assert!(!ShellTool::validate_arg("a`b"));
        assert!(!ShellTool::validate_arg("a\nb"));
        assert!(!ShellTool::validate_arg("a\0b"));
    }

    #[tokio::test]
    async fn blocked_by_empty_allowlist() {
        let tool = ShellTool::new(vec![], 10);
        let result = tool
            .invoke(json!({"command": "echo", "args": ["hi"]}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn blocked_command_not_in_allowlist() {
        let tool = ShellTool::new(vec!["echo".into()], 10);
        let result = tool
            .invoke(json!({"command": "rm", "args": ["-rf", "/"]}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn blocked_path_in_command() {
        let tool = ShellTool::new(vec!["/bin/echo".into()], 10);
        let result = tool
            .invoke(json!({"command": "/bin/echo", "args": ["hi"]}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn blocked_metacharacter_in_args() {
        let tool = ShellTool::new(vec!["echo".into()], 10);
        let result = tool
            .invoke(json!({"command": "echo", "args": ["$(whoami)"]}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn allowed_command_executes() {
        let tool = ShellTool::new(vec!["echo".into()], 10);
        let result = tool
            .invoke(json!({"command": "echo", "args": ["hello"]}))
            .await;
        let val = result.unwrap();
        assert_eq!(val["stdout"], "hello");
        assert_eq!(val["exit_code"], 0);
    }
}
