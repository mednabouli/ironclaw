use std::time::Instant;

use anyhow::Context;
use async_trait::async_trait;
use ironclaw_core::*;
use serde_json::{json, Value};

pub struct OllamaProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .expect("reqwest client"),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
        }
    }

    fn messages_to_json(&self, messages: &[Message]) -> Value {
        let msgs: Vec<Value> = messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "tool",
                };
                if let Some(tr) = &m.tool_result {
                    json!({ "role": role, "content": tr.content.to_string() })
                } else {
                    json!({ "role": role, "content": m.content })
                }
            })
            .collect();
        json!(msgs)
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }
    fn supports_tools(&self) -> bool {
        true
    }

    async fn complete(&self, req: CompletionRequest) -> anyhow::Result<CompletionResponse> {
        let t0 = Instant::now();
        let model = req.model.as_deref().unwrap_or(&self.model).to_string();

        let mut body = json!({
            "model":    model,
            "messages": self.messages_to_json(&req.messages),
            "stream":   false,
            "options": { "temperature": req.temperature.unwrap_or(0.7) }
        });

        if !req.tools.is_empty() {
            let tools: Vec<Value> = req.tools.iter().map(|t| json!({
                "type": "function",
                "function": { "name": t.name, "description": t.description, "parameters": t.parameters }
            })).collect();
            body["tools"] = json!(tools);
        }

        let resp: Value = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .context("Ollama HTTP error")?
            .error_for_status()
            .context("Ollama API error")?
            .json()
            .await
            .context("Ollama JSON parse")?;

        let content = resp["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Parse tool calls if present
        let mut tool_calls = vec![];
        if let Some(arr) = resp["message"]["tool_calls"].as_array() {
            for tc in arr {
                let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                let args = tc["function"]["arguments"].clone();
                tool_calls.push(ToolCall {
                    id: uuid::Uuid::new_v4().to_string(),
                    name,
                    arguments: args,
                });
            }
        }

        let stop_reason = if !tool_calls.is_empty() {
            StopReason::ToolUse
        } else {
            StopReason::EndTurn
        };
        let prompt_tokens = resp["prompt_eval_count"].as_u64().unwrap_or(0) as u32;
        let completion_tokens = resp["eval_count"].as_u64().unwrap_or(0) as u32;

        let mut msg = Message::assistant(content);
        msg.tool_calls = tool_calls;

        Ok(CompletionResponse {
            message: msg,
            stop_reason,
            usage: TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
            model,
            latency_ms: t0.elapsed().as_millis() as u64,
        })
    }

    async fn stream(&self, req: CompletionRequest) -> anyhow::Result<BoxStream<StreamChunk>> {
        let model = req.model.as_deref().unwrap_or(&self.model).to_string();
        let body = json!({
            "model":    model,
            "messages": self.messages_to_json(&req.messages),
            "stream":   true,
        });

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .context("Ollama stream error")?
            .error_for_status()?;

        Ok(crate::sse::parse_ollama_ndjson_stream(response))
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
