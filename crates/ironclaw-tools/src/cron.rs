//! Cron tool — schedule one-shot or recurring tasks.
//!
//! Uses `tokio::time` to schedule delayed task execution.
//! Tasks are stored in a `DashMap` and can be listed or cancelled.
//! This is a lightweight alternative to `tokio-cron-scheduler` that
//! avoids the external dependency.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use ironclaw_core::{Tool, ToolError, ToolSchema};
use serde_json::{json, Value};
use tracing::{debug, info, warn};

/// A scheduled task entry.
#[derive(Debug, Clone)]
struct ScheduledTask {
    id: String,
    name: String,
    delay_secs: u64,
    /// Whether this task has been fired.
    fired: bool,
}

/// Schedule delayed tasks using `tokio::time::sleep`.
pub struct CronTool {
    tasks: Arc<DashMap<String, ScheduledTask>>,
}

impl CronTool {
    /// Create a new cron tool.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(DashMap::new()),
        }
    }
}

impl Default for CronTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str {
        "cron"
    }

    fn description(&self) -> &str {
        "Schedule, list, or cancel delayed tasks. \
         Actions: 'schedule' (creates a one-shot timer), 'list' (shows all tasks), \
         'cancel' (removes a task by ID)."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            self.name(),
            self.description(),
            json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["schedule", "list", "cancel"],
                        "description": "The action to perform"
                    },
                    "name": {
                        "type": "string",
                        "description": "A human-readable name for the task (required for 'schedule')"
                    },
                    "delay_secs": {
                        "type": "integer",
                        "description": "Seconds from now to fire the task (required for 'schedule')"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "The task ID (required for 'cancel')"
                    }
                },
                "required": ["action"]
            }),
        )
    }

    async fn invoke(&self, params: Value) -> Result<Value, ToolError> {
        (async move {
            let action = params["action"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'action' parameter"))?;

            match action {
                "schedule" => self.schedule(&params).await,
                "list" => self.list_tasks(),
                "cancel" => self.cancel(&params),
                _ => anyhow::bail!(
                    "Unknown action: '{action}'. Use 'schedule', 'list', or 'cancel'."
                ),
            }
        })
        .await
        .map_err(Into::into)
    }
}

impl CronTool {
    async fn schedule(&self, params: &Value) -> anyhow::Result<Value> {
        let name = params["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'name' for schedule action"))?;

        let delay_secs = params["delay_secs"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'delay_secs' for schedule action"))?;

        if delay_secs == 0 {
            anyhow::bail!("delay_secs must be > 0");
        }

        // Cap at 24 hours to prevent resource leaks
        if delay_secs > 86_400 {
            anyhow::bail!("delay_secs cannot exceed 86400 (24 hours)");
        }

        let task_id = uuid::Uuid::new_v4().to_string();
        let task = ScheduledTask {
            id: task_id.clone(),
            name: name.to_string(),
            delay_secs,
            fired: false,
        };

        self.tasks.insert(task_id.clone(), task);

        // Spawn the timer
        let tasks = self.tasks.clone();
        let id_clone = task_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            if let Some(mut entry) = tasks.get_mut(&id_clone) {
                entry.fired = true;
                info!(task_id = %id_clone, name = %entry.name, "Scheduled task fired");
            }
        });

        debug!(task_id = %task_id, name = %name, delay_secs, "Task scheduled");

        Ok(json!({
            "task_id": task_id,
            "name": name,
            "delay_secs": delay_secs,
            "status": "scheduled",
        }))
    }

    fn list_tasks(&self) -> anyhow::Result<Value> {
        let tasks: Vec<Value> = self
            .tasks
            .iter()
            .map(|entry| {
                let t = entry.value();
                json!({
                    "task_id": t.id,
                    "name": t.name,
                    "delay_secs": t.delay_secs,
                    "fired": t.fired,
                })
            })
            .collect();

        Ok(json!({
            "tasks": tasks,
            "count": tasks.len(),
        }))
    }

    fn cancel(&self, params: &Value) -> anyhow::Result<Value> {
        let task_id = params["task_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'task_id' for cancel action"))?;

        if self.tasks.remove(task_id).is_some() {
            info!(task_id = %task_id, "Task cancelled");
            Ok(json!({
                "task_id": task_id,
                "status": "cancelled",
            }))
        } else {
            warn!(task_id = %task_id, "Task not found for cancellation");
            anyhow::bail!("Task '{task_id}' not found")
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_valid() {
        let tool = CronTool::new();
        assert_eq!(tool.name(), "cron");
        let schema = tool.schema();
        assert!(schema.parameters["properties"]["action"].is_object());
    }

    #[tokio::test]
    async fn schedule_and_list() {
        let tool = CronTool::new();

        let result = tool
            .invoke(json!({
                "action": "schedule",
                "name": "test task",
                "delay_secs": 3600
            }))
            .await
            .unwrap();

        assert_eq!(result["status"], "scheduled");
        assert!(result["task_id"].is_string());

        let list = tool.invoke(json!({"action": "list"})).await.unwrap();
        assert_eq!(list["count"], 1);
    }

    #[tokio::test]
    async fn schedule_and_cancel() {
        let tool = CronTool::new();

        let result = tool
            .invoke(json!({
                "action": "schedule",
                "name": "cancellable",
                "delay_secs": 3600
            }))
            .await
            .unwrap();

        let task_id = result["task_id"].as_str().unwrap();

        let cancel = tool
            .invoke(json!({"action": "cancel", "task_id": task_id}))
            .await
            .unwrap();
        assert_eq!(cancel["status"], "cancelled");

        let list = tool.invoke(json!({"action": "list"})).await.unwrap();
        assert_eq!(list["count"], 0);
    }

    #[tokio::test]
    async fn cancel_nonexistent_fails() {
        let tool = CronTool::new();
        let result = tool
            .invoke(json!({"action": "cancel", "task_id": "no-such-id"}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rejects_zero_delay() {
        let tool = CronTool::new();
        let result = tool
            .invoke(json!({"action": "schedule", "name": "bad", "delay_secs": 0}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rejects_excessive_delay() {
        let tool = CronTool::new();
        let result = tool
            .invoke(json!({"action": "schedule", "name": "too long", "delay_secs": 100000}))
            .await;
        assert!(result.is_err());
    }
}
