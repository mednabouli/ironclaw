use ironclaw_tools::{ToolRegistry, datetime::DateTimeTool, shell::ShellTool};
use ironclaw_core::Tool;
use std::sync::Arc;

#[test]
fn datetime_tool_schema_name() {
    assert_eq!(DateTimeTool.name(), "get_datetime");
    assert_eq!(DateTimeTool.schema().name, "get_datetime");
}

#[tokio::test]
async fn datetime_tool_returns_rfc3339() {
    let v = DateTimeTool.invoke(serde_json::json!({})).await.unwrap();
    let dt = v["datetime"].as_str().unwrap();
    assert!(dt.contains("T"));  // ISO8601 format
    assert!(v["unix_timestamp"].is_number());
}

#[tokio::test]
async fn shell_tool_blocks_non_allowlist_cmd() {
    let tool = ShellTool::new(vec!["echo".into()], 5);
    let err = tool.invoke(serde_json::json!({"command": "rm"})).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("allowlist"));
}

#[tokio::test]
async fn shell_tool_runs_echo() {
    let tool = ShellTool::new(vec!["echo".into()], 5);
    let v = tool.invoke(serde_json::json!({"command":"echo","args":["hello"]})).await.unwrap();
    assert_eq!(v["stdout"], "hello");
    assert_eq!(v["exit_code"], 0);
}

#[test]
fn tool_registry_get_registered() {
    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(DateTimeTool));
    assert!(reg.get("get_datetime").is_some());
    assert!(reg.get("nonexistent").is_none());
    assert_eq!(reg.all_schemas().len(), 1);
}

#[test]
fn tool_registry_filtered_schemas() {
    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(DateTimeTool));
    reg.register(Arc::new(ShellTool::new(vec![], 5)));
    let filtered = reg.filtered_schemas(&["get_datetime".to_string()]);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "get_datetime");
}