use std::time::Instant;

use anyhow::Context;
use async_trait::async_trait;
use ironclaw_core::*;
use serde_json::{json, Value};

/// OpenAI-compatible provider (GPT-4, GPT-4o, Groq, LM Studio, DeepSeek, etc.).
///
/// API key is never included in `Debug` output — use `tracing::debug!` safely.
pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl std::fmt::Debug for OpenAIProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIProvider")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl OpenAIProvider {
    /// Create a new OpenAI-compatible provider with the given API key, model, and base URL.
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .expect("reqwest client"),
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn messages_to_json(&self, messages: &[Message]) -> Vec<Value> {
        messages.iter().map(|m| {
            let role = match m.role {
                Role::System    => "system",
                Role::User      => "user",
                Role::Assistant => "assistant",
                Role::Tool      => "tool",
                _ => "user",
            };
            if let Some(tr) = &m.tool_result {
                json!({ "role": role, "tool_call_id": tr.call_id, "content": tr.content.to_string() })
            } else if !m.tool_calls.is_empty() {
                let tcs: Vec<Value> = m.tool_calls.iter().map(|tc| json!({
                    "id": tc.id, "type": "function",
                    "function": { "name": tc.name, "arguments": tc.arguments.to_string() }
                })).collect();
                json!({ "role": role, "content": m.content, "tool_calls": tcs })
            } else {
                json!({ "role": role, "content": m.content })
            }
        }).collect()
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &'static str {
        "openai"
    }
    fn supports_vision(&self) -> bool {
        true
    }

    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        (async {
        let t0 = Instant::now();
        let model = req.model.as_deref().unwrap_or(&self.model).to_string();
        let mut body = json!({
            "model":       model,
            "messages":    self.messages_to_json(&req.messages),
            "max_tokens":  req.max_tokens.unwrap_or(4096),
            "temperature": req.temperature.unwrap_or(0.7),
            "stream":      false,
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
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("OpenAI HTTP")?
            .error_for_status()
            .context("OpenAI API error")?
            .json()
            .await
            .context("OpenAI JSON")?;

        let choice = &resp["choices"][0];
        let message = &choice["message"];
        let content = message["content"].as_str().unwrap_or("").to_string();

        let mut tool_calls = vec![];
        if let Some(arr) = message["tool_calls"].as_array() {
            for tc in arr {
                let id = tc["id"].as_str().unwrap_or("").to_string();
                let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                let args: Value =
                    serde_json::from_str(tc["function"]["arguments"].as_str().unwrap_or("{}"))
                        .unwrap_or(json!({}));
                tool_calls.push(ToolCall::new(
                    id,
                    name,
                    args,
                ));
            }
        }

        let stop_reason = match choice["finish_reason"].as_str().unwrap_or("stop") {
            "tool_calls" => StopReason::ToolUse,
            "length" => StopReason::MaxTokens,
            _ => StopReason::EndTurn,
        };

        let p = resp["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
        let c = resp["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;

        let mut msg = Message::assistant(content);
        msg.tool_calls = tool_calls;

        Ok::<_, anyhow::Error>(CompletionResponse::new(
            msg,
            stop_reason,
            TokenUsage::new(p, c, p + c),
            model,
            t0.elapsed().as_millis() as u64,
        ))
        }).await.map_err(Into::into)
    }

    async fn stream(
        &self,
        req: CompletionRequest,
    ) -> Result<BoxStream<StreamChunk>, ProviderError> {
        (async {
            let model = req.model.as_deref().unwrap_or(&self.model).to_string();
            let body = json!({
                "model":    model,
                "messages": self.messages_to_json(&req.messages),
                "stream":   true,
            });
            let response = self
                .client
                .post(format!("{}/v1/chat/completions", self.base_url))
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await?
                .error_for_status()?;

            Ok::<_, anyhow::Error>(crate::sse::parse_openai_sse_stream(response))
        })
        .await
        .map_err(Into::into)
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        if self.api_key.is_empty() {
            return Err(ProviderError::Auth("OpenAI: api_key not set".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_api_key() {
        let p = OpenAIProvider::new("sk-secret", "gpt-4o-mini", "https://api.openai.com");
        let debug_str = format!("{:?}", p);
        assert!(
            !debug_str.contains("sk-secret"),
            "api_key must not appear in Debug output"
        );
        assert!(
            debug_str.contains("[REDACTED]"),
            "Debug output must contain [REDACTED]"
        );
    }

    #[test]
    fn provider_name_is_openai() {
        let p = OpenAIProvider::new("key", "model", "https://api.openai.com");
        assert_eq!(p.name(), "openai");
    }
}
