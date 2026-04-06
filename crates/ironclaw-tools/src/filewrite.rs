//! File write tool — write or append to files sandboxed to allowed directories.
//!
//! Enforces the same path-traversal protection as [`crate::fileread::FileReadTool`].

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use ironclaw_core::{Tool, ToolSchema};
use serde_json::{json, Value};
use tracing::warn;

/// Write or append to files sandboxed to a set of allowed directories.
pub struct FileWriteTool {
    allowed_dirs: Vec<PathBuf>,
}

impl FileWriteTool {
    /// Create a new file write tool. If `allowed_dirs` is empty,
    /// all writes are blocked.
    ///
    /// Allowed directories are canonicalized at construction time so that
    /// symlinks (e.g. `/tmp` → `/private/tmp` on macOS) are handled correctly.
    pub fn new(allowed_dirs: Vec<PathBuf>) -> Self {
        let allowed_dirs = allowed_dirs
            .into_iter()
            .filter_map(|d| std::fs::canonicalize(&d).ok())
            .collect();
        Self { allowed_dirs }
    }

    /// Check that the target path is inside one of the allowed directories.
    /// For writes, we check the parent directory since the file may not exist yet.
    fn is_allowed(&self, path: &Path) -> bool {
        self.allowed_dirs.iter().any(|dir| path.starts_with(dir))
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write or append content to a local file. The path must be within allowed directories."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to write to"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write"
                    },
                    "append": {
                        "type": "boolean",
                        "description": "If true, append to the file instead of overwriting (default: false)",
                        "default": false
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn invoke(&self, params: Value) -> anyhow::Result<Value> {
        let path_str = params["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;

        let content = params["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' parameter"))?;

        let append = params["append"].as_bool().unwrap_or(false);

        let path = PathBuf::from(path_str);

        // Resolve the parent directory to check allowlist
        // The file itself may not exist yet, so we canonicalize the parent.
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine parent directory of '{path_str}'"))?;

        let canonical_parent = tokio::fs::canonicalize(parent)
            .await
            .map_err(|e| anyhow::anyhow!("Cannot resolve directory '{}': {e}", parent.display()))?;

        let canonical_path = canonical_parent.join(
            path.file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid file name in '{path_str}'"))?,
        );

        if !self.is_allowed(&canonical_path) {
            warn!(path = %canonical_path.display(), "File write blocked: outside allowed directories");
            anyhow::bail!(
                "Access denied: '{}' is not within allowed directories",
                path_str
            );
        }

        let bytes_written = content.len();

        if append {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&canonical_path)
                .await
                .map_err(|e| anyhow::anyhow!("Cannot open file for append: {e}"))?;
            file.write_all(content.as_bytes())
                .await
                .map_err(|e| anyhow::anyhow!("Write failed: {e}"))?;
        } else {
            tokio::fs::write(&canonical_path, content.as_bytes())
                .await
                .map_err(|e| anyhow::anyhow!("Write failed: {e}"))?;
        }

        Ok(json!({
            "path": canonical_path.display().to_string(),
            "bytes_written": bytes_written,
            "append": append,
        }))
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_valid() {
        let tool = FileWriteTool::new(vec![]);
        assert_eq!(tool.name(), "file_write");
        let schema = tool.schema();
        assert!(schema.parameters["properties"]["path"].is_object());
        assert!(schema.parameters["properties"]["content"].is_object());
    }

    #[test]
    fn is_allowed_checks_prefix() {
        let dir = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let tool = FileWriteTool::new(vec![dir.clone()]);
        assert!(tool.is_allowed(&dir.join("output.txt")));
        assert!(!tool.is_allowed(Path::new("/etc/shadow")));
    }

    #[tokio::test]
    async fn write_and_read_roundtrip() {
        let dir = std::env::temp_dir();
        let tool = FileWriteTool::new(vec![dir.clone()]);
        let test_file = dir.join("ironclaw_test_write.txt");
        let test_path = test_file.display().to_string();

        // Write
        let result = tool
            .invoke(json!({
                "path": test_path,
                "content": "hello world"
            }))
            .await
            .unwrap();
        assert_eq!(result["bytes_written"], 11);
        assert_eq!(result["append"], false);

        // Verify content
        let content = tokio::fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, "hello world");

        // Append
        let result = tool
            .invoke(json!({
                "path": test_path,
                "content": "\nline two",
                "append": true
            }))
            .await
            .unwrap();
        assert_eq!(result["append"], true);

        let content = tokio::fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, "hello world\nline two");

        // Cleanup
        tokio::fs::remove_file(&test_file).await.ok();
    }

    #[tokio::test]
    async fn rejects_disallowed_dir() {
        let tool = FileWriteTool::new(vec![PathBuf::from("/nonexistent_dir_xyz")]);
        let result = tool
            .invoke(json!({
                "path": "/tmp/should_not_write.txt",
                "content": "nope"
            }))
            .await;
        assert!(result.is_err());
    }
}
