use std::time::Instant;

use anyhow::Context;
use async_trait::async_trait;
use ironclaw_core::*;
use serde_json::{json, Value};

/// Anthropic Claude provider (claude-3-5-sonnet, claude-3-7-sonnet, claude-opus-4, etc.).
///
/// API key is never included in `Debug` output — use `tracing::debug!` safely.
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl std::fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicProvider")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl AnthropicProvider {
    /// Create a new Anthropic provider with the given API key, model, and base URL.
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

    fn build_messages(&self, messages: &[Message]) -> (Option<String>, Vec<Value>) {
        let mut system_prompt = None;
        let mut msgs = vec![];
        for m in messages {
            match m.role {
                Role::System => {
                    system_prompt = Some(m.content.clone());
                }
                Role::User => {
                    msgs.push(json!({ "role": "user",      "content": m.content }));
                }
                Role::Assistant => {
                    if !m.tool_calls.is_empty() {
                        let content: Vec<Value> = std::iter::once(json!({"type":"text","text": m.content}))
                            .chain(m.tool_calls.iter().map(|tc| json!({
                                "type": "tool_use", "id": tc.id, "name": tc.name, "input": tc.arguments
                            }))).collect();
                        msgs.push(json!({ "role": "assistant", "content": content }));
                    } else {
                        msgs.push(json!({ "role": "assistant", "content": m.content }));
                    }
                }
                Role::Tool => {
                    if let Some(tr) = &m.tool_result {
                        let content = vec![json!({
                            "type": "tool_result", "tool_use_id": tr.call_id,
                            "content": tr.content.to_string()
                        })];
                        msgs.push(json!({ "role": "user", "content": content }));
                    }
                }
                _ => {
                    msgs.push(json!({ "role": "user", "content": m.content }));
                }
            }
        }
        (system_prompt, msgs)
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "claude"
    }
    fn supports_vision(&self) -> bool {
        true
    }

    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        (async {
            let t0 = Instant::now();
            let model = req.model.as_deref().unwrap_or(&self.model).to_string();
            let (system, messages) = self.build_messages(&req.messages);

            let mut body = json!({
                "model":      model,
                "max_tokens": req.max_tokens.unwrap_or(4096),
                "messages":   messages,
                "stream":     false,
            });
            if let Some(sp) = system {
                body["system"] = json!(sp);
            }
            if !req.tools.is_empty() {
                let tools: Vec<Value> = req
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name, "description": t.description, "input_schema": t.parameters
                    })
                })
                .collect();
                body["tools"] = json!(tools);
            }

            let resp: Value = self
                .client
                .post(format!("{}/v1/messages", self.base_url))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .context("Anthropic HTTP")?
                .error_for_status()
                .context("Anthropic API error")?
                .json()
                .await
                .context("Anthropic JSON")?;

            let mut text = String::new();
            let mut tool_calls = vec![];

            if let Some(content_arr) = resp["content"].as_array() {
                for block in content_arr {
                    match block["type"].as_str().unwrap_or("") {
                        "text" => {
                            text.push_str(block["text"].as_str().unwrap_or(""));
                        }
                        "tool_use" => {
                            tool_calls.push(ToolCall::new(
                                block["id"].as_str().unwrap_or(""),
                                block["name"].as_str().unwrap_or(""),
                                block["input"].clone(),
                            ));
                        }
                        _ => {}
                    }
                }
            }

            let stop_reason = match resp["stop_reason"].as_str().unwrap_or("end_turn") {
                "tool_use" => StopReason::ToolUse,
                "max_tokens" => StopReason::MaxTokens,
                _ => StopReason::EndTurn,
            };

            let p = resp["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32;
            let c = resp["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;

            let mut msg = Message::assistant(text);
            msg.tool_calls = tool_calls;

            Ok::<_, anyhow::Error>(CompletionResponse::new(
                msg,
                stop_reason,
                TokenUsage::new(p, c, p + c),
                model,
                t0.elapsed().as_millis() as u64,
            ))
        })
        .await
        .map_err(Into::into)
    }

    async fn stream(
        &self,
        req: CompletionRequest,
    ) -> Result<BoxStream<StreamChunk>, ProviderError> {
        (async {
            let model = req.model.as_deref().unwrap_or(&self.model).to_string();
            let (system, messages) = self.build_messages(&req.messages);
            let mut body = json!({
                "model": model,
                "max_tokens": req.max_tokens.unwrap_or(4096),
                "messages": messages,
                "stream": true,
            });
            if let Some(sp) = system {
                body["system"] = json!(sp);
            }

            let response = self
                .client
                .post(format!("{}/v1/messages", self.base_url))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send()
                .await?
                .error_for_status()?;

            Ok::<_, anyhow::Error>(crate::sse::parse_anthropic_sse_stream(response))
        })
        .await
        .map_err(Into::into)
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        if self.api_key.is_empty() {
            return Err(ProviderError::Auth("Anthropic: api_key not set".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_api_key() {
        let p = AnthropicProvider::new(
            "sk-ant-secret",
            "claude-3-5-sonnet-20241022",
            "https://api.anthropic.com",
        );
        let debug_str = format!("{:?}", p);
        assert!(
            !debug_str.contains("sk-ant-secret"),
            "api_key must not appear in Debug output"
        );
        assert!(
            debug_str.contains("[REDACTED]"),
            "Debug output must contain [REDACTED]"
        );
    }

    #[test]
    fn provider_name_is_claude() {
        let p = AnthropicProvider::new("key", "model", "https://api.anthropic.com");
        assert_eq!(p.name(), "claude");
    }
}
