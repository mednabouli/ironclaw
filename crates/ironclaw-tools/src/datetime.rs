use async_trait::async_trait;
use ironclaw_core::{Tool, ToolSchema};
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
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "timezone": { "type": "string", "description": "IANA timezone e.g. UTC, America/Toronto", "default": "UTC" }
                },
                "required": []
            }),
        }
    }

    async fn invoke(&self, params: Value) -> anyhow::Result<Value> {
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
