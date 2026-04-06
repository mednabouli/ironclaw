//! File read tool — read local files sandboxed to allowed directories.
//!
//! Prevents path traversal by resolving to canonical paths and checking
//! that the result is within one of the allowed directories.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use ironclaw_core::{Tool, ToolError, ToolSchema};
use serde_json::{json, Value};
use tracing::warn;

/// Read files sandboxed to a set of allowed directories.
pub struct FileReadTool {
    allowed_dirs: Vec<PathBuf>,
}

impl FileReadTool {
    /// Create a new file read tool. If `allowed_dirs` is empty,
    /// all file reads are blocked.
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

    /// Check that the resolved path is inside one of the allowed directories.
    fn is_allowed(&self, path: &Path) -> bool {
        self.allowed_dirs.iter().any(|dir| path.starts_with(dir))
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read the contents of a local file. The path must be within allowed directories."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            self.name(),
            self.description(),
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to read"
                    },
                    "max_bytes": {
                        "type": "integer",
                        "description": "Maximum bytes to read (default: 65536)",
                        "default": 65536
                    }
                },
                "required": ["path"]
            }),
        )
    }

    async fn invoke(&self, params: Value) -> Result<Value, ToolError> {
        (async move {
            let path_str = params["path"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;

            // Reject null bytes — prevents null byte injection in C-backed syscalls
            if path_str.contains('\0') {
                anyhow::bail!("Path contains null byte");
            }

            let max_bytes = params["max_bytes"].as_u64().unwrap_or(65_536) as usize;

            // Resolve to canonical path to prevent traversal attacks
            let path = tokio::fs::canonicalize(path_str)
                .await
                .map_err(|e| anyhow::anyhow!("Cannot resolve path '{path_str}': {e}"))?;

            if !self.is_allowed(&path) {
                warn!(path = %path.display(), "File read blocked: outside allowed directories");
                anyhow::bail!(
                    "Access denied: '{}' is not within allowed directories",
                    path_str
                );
            }

            let metadata = tokio::fs::metadata(&path)
                .await
                .map_err(|e| anyhow::anyhow!("Cannot read metadata: {e}"))?;

            if !metadata.is_file() {
                anyhow::bail!("'{}' is not a file", path_str);
            }

            let file_size = metadata.len() as usize;
            let content = tokio::fs::read(&path)
                .await
                .map_err(|e| anyhow::anyhow!("Cannot read file: {e}"))?;

            let truncated = file_size > max_bytes;
            let bytes = if truncated {
                &content[..max_bytes]
            } else {
                &content
            };

            let text = String::from_utf8_lossy(bytes);

            Ok(json!({
                "path": path.display().to_string(),
                "content": text,
                "size_bytes": file_size,
                "truncated": truncated,
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
        let tool = FileReadTool::new(vec![]);
        assert_eq!(tool.name(), "file_read");
        assert!(tool.schema().parameters["properties"]["path"].is_object());
    }

    #[test]
    fn is_allowed_checks_prefix() {
        let dir = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let tool = FileReadTool::new(vec![dir.clone()]);
        assert!(tool.is_allowed(&dir.join("foo.txt")));
        assert!(tool.is_allowed(&dir.join("sub/deep/file")));
        assert!(!tool.is_allowed(Path::new("/etc/passwd")));
        assert!(!tool.is_allowed(Path::new("/home/user")));
    }

    #[test]
    fn empty_allowlist_blocks_all() {
        let tool = FileReadTool::new(vec![]);
        assert!(!tool.is_allowed(
            &std::fs::canonicalize(std::env::temp_dir())
                .unwrap()
                .join("foo")
        ));
    }

    #[tokio::test]
    async fn rejects_disallowed_path() {
        let tool = FileReadTool::new(vec![PathBuf::from("/nonexistent_allowed")]);
        let result = tool.invoke(json!({"path": "/etc/hosts"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rejects_null_byte_in_path() {
        let dir = std::env::temp_dir();
        let tool = FileReadTool::new(vec![dir]);
        let result = tool.invoke(json!({"path": "/tmp/evil\0.txt"})).await;
        assert!(result.is_err());
    }
}
