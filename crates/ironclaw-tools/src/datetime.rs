use async_trait::async_trait;
use ironclaw_core::{Tool, ToolError, ToolSchema};
use serde_json::{json, Value};

pub struct DateTimeTool;

#[async_trait]
impl Tool for DateTimeTool {
    fn name(&self) -> &str {
        "get_datetime"
    }
    fn description(&self) -> &str {
        "Get the current date and time in any timezone."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            self.name(),
            self.description(),
            json!({
                "type": "object",
                "properties": {
                    "timezone": { "type": "string", "description": "IANA timezone e.g. UTC, America/Toronto", "default": "UTC" }
                },
                "required": []
            }),
        )
    }

    async fn invoke(&self, params: Value) -> Result<Value, ToolError> {
        let tz_str = params
            .get("timezone")
            .and_then(|v| v.as_str())
            .unwrap_or("UTC");
        let now = chrono::Utc::now();
        Ok(json!({
            "datetime":       now.to_rfc3339(),
            "timezone":       tz_str,
            "unix_timestamp": now.timestamp(),
        }))
    }
}
