//! HTTP GET tool — fetch a URL and return the response body.
//!
//! Returns the body as text (up to a configurable limit) along with
//! status code and content-type header.

use std::time::Duration;

use async_trait::async_trait;
use ironclaw_core::{Tool, ToolError, ToolSchema};
use reqwest::Client;
use serde_json::{json, Value};

const DEFAULT_MAX_BYTES: usize = 65_536; // 64KB
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Fetch a URL via HTTP GET and return the body.
pub struct HttpGetTool {
    client: Client,
    max_bytes: usize,
}

impl HttpGetTool {
    /// Create with default settings.
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
                .user_agent("IronClaw/0.1 (AI Agent Framework)")
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()
                .unwrap_or_default(),
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }

    /// Create with a custom max body size.
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }
}

impl Default for HttpGetTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HttpGetTool {
    fn name(&self) -> &str {
        "http_get"
    }

    fn description(&self) -> &str {
        "Fetch a URL via HTTP GET. Returns status code, content type, and body text (up to 64KB)."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            self.name(),
            self.description(),
            json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch (must start with http:// or https://)"
                    },
                    "headers": {
                        "type": "object",
                        "description": "Optional HTTP headers as key-value pairs",
                        "additionalProperties": { "type": "string" }
                    }
                },
                "required": ["url"]
            }),
        )
    }

    async fn invoke(&self, params: Value) -> Result<Value, ToolError> {
        (async move {
            let url = params["url"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;

            // Basic URL validation
            if !url.starts_with("http://") && !url.starts_with("https://") {
                anyhow::bail!("URL must start with http:// or https://");
            }

            let mut request = self.client.get(url);

            // Add custom headers
            if let Some(headers) = params["headers"].as_object() {
                for (key, value) in headers {
                    if let Some(v) = value.as_str() {
                        request = request.header(key.as_str(), v);
                    }
                }
            }

            let response = request
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("HTTP request failed: {e}"))?;

            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown")
                .to_string();

            let bytes = response
                .bytes()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to read response body: {e}"))?;

            let truncated = bytes.len() > self.max_bytes;
            let body_bytes = if truncated {
                &bytes[..self.max_bytes]
            } else {
                &bytes[..]
            };
            let body = String::from_utf8_lossy(body_bytes);

            Ok(json!({
                "url": url,
                "status": status,
                "content_type": content_type,
                "body": body,
                "size_bytes": bytes.len(),
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
        let tool = HttpGetTool::new();
        assert_eq!(tool.name(), "http_get");
        let schema = tool.schema();
        assert!(schema.parameters["properties"]["url"].is_object());
        assert_eq!(schema.parameters["required"][0], "url");
    }

    #[tokio::test]
    async fn rejects_non_http_url() {
        let tool = HttpGetTool::new();
        let result = tool.invoke(json!({"url": "ftp://evil.com/file"})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("http://"));
    }

    #[tokio::test]
    async fn rejects_missing_url() {
        let tool = HttpGetTool::new();
        let result = tool.invoke(json!({})).await;
        assert!(result.is_err());
    }

    #[test]
    fn with_max_bytes_sets_limit() {
        let tool = HttpGetTool::new().with_max_bytes(1024);
        assert_eq!(tool.max_bytes, 1024);
    }
}
