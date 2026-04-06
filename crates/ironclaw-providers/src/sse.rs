//! Shared SSE and NDJSON stream parsers for all providers.
//!
//! **Every provider must delegate streaming to these functions.**
//! Do NOT write custom SSE parsing in individual provider modules.

use ironclaw_core::{BoxStream, StopReason, StreamChunk, ToolCallDelta};
use serde_json::Value;
use tokio_stream::StreamExt;
use tracing::warn;

/// Parse an OpenAI-compatible SSE response into a `BoxStream<StreamChunk>`.
///
/// Handles the standard format used by OpenAI, Groq, Together, LM Studio,
/// DeepSeek, and any other provider that follows the OpenAI chat completions
/// streaming protocol:
///
/// ```text
/// data: {"choices":[{"delta":{"content":"Hi"}}]}\n\n
/// data: {"choices":[{"delta":{"tool_calls":[...]}}]}\n\n
/// data: [DONE]\n\n
/// ```
pub fn parse_openai_sse_stream(response: reqwest::Response) -> BoxStream<StreamChunk> {
    let (tx, rx) = tokio::sync::mpsc::channel::<anyhow::Result<StreamChunk>>(64);

    tokio::spawn(async move {
        let mut byte_stream = response.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = byte_stream.next().await {
            match chunk {
                Err(e) => {
                    let _ = tx.send(Err(anyhow::anyhow!(e))).await;
                    break;
                }
                Ok(bytes) => {
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(pos) = buf.find('\n') {
                        let line: String = buf.drain(..=pos).collect();
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }

                        let Some(data) = line.strip_prefix("data: ") else {
                            continue;
                        };

                        if data == "[DONE]" {
                            // Pure terminator — do not emit a chunk.
                            // The real stop_reason was already sent on the
                            // preceding chunk that carried finish_reason.
                            return;
                        }

                        let Ok(v) = serde_json::from_str::<Value>(data) else {
                            continue;
                        };

                        let choice = &v["choices"][0];
                        let delta_obj = &choice["delta"];

                        // ── Text delta ──
                        let delta = delta_obj["content"].as_str().unwrap_or("").to_string();

                        // ── Tool call deltas ──
                        let mut tool_calls = vec![];
                        if let Some(tc_arr) = delta_obj["tool_calls"].as_array() {
                            for tc in tc_arr {
                                let index = tc["index"].as_u64().unwrap_or(0) as usize;
                                let arguments_delta = tc["function"]["arguments"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();
                                let tcd = match (tc["id"].as_str(), tc["function"]["name"].as_str())
                                {
                                    (Some(id), Some(name)) => {
                                        ToolCallDelta::first(index, id, name, arguments_delta)
                                    }
                                    _ => ToolCallDelta::new(index, arguments_delta),
                                };
                                tool_calls.push(tcd);
                            }
                        }

                        // ── Stop reason ──
                        let stop_reason = choice["finish_reason"].as_str().and_then(|r| match r {
                            "stop" => Some(StopReason::EndTurn),
                            "tool_calls" => Some(StopReason::ToolUse),
                            "length" => Some(StopReason::MaxTokens),
                            _ => None,
                        });

                        // Skip completely empty chunks (no text, no tool calls)
                        if delta.is_empty() && tool_calls.is_empty() && stop_reason.is_none() {
                            continue;
                        }

                        let done = stop_reason.is_some();
                        let _ = tx
                            .send(Ok(StreamChunk::new(delta, done, tool_calls, stop_reason)))
                            .await;
                    }
                }
            }
        }
    });

    Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
}

/// Parse an Anthropic SSE response into a `BoxStream<StreamChunk>`.
///
/// Handles the Anthropic Messages API streaming format:
///
/// ```text
/// event: message_start
/// data: {"type":"message_start",...}
///
/// event: content_block_start
/// data: {"type":"content_block_start","index":0,"content_block":{"type":"text",...}}
///
/// event: content_block_delta
/// data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}
///
/// event: content_block_start
/// data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_...","name":"shell"}}
///
/// event: content_block_delta
/// data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"cmd\":"}}
///
/// event: message_delta
/// data: {"type":"message_delta","delta":{"stop_reason":"end_turn"}}
///
/// event: message_stop
/// data: {"type":"message_stop"}
/// ```
pub fn parse_anthropic_sse_stream(response: reqwest::Response) -> BoxStream<StreamChunk> {
    let (tx, rx) = tokio::sync::mpsc::channel::<anyhow::Result<StreamChunk>>(64);

    tokio::spawn(async move {
        let mut byte_stream = response.bytes_stream();
        let mut buf = String::new();
        // Track active tool_use blocks: index → (id, name)
        let mut active_tools: Vec<(usize, String, String)> = vec![];

        while let Some(chunk) = byte_stream.next().await {
            match chunk {
                Err(e) => {
                    let _ = tx.send(Err(anyhow::anyhow!(e))).await;
                    break;
                }
                Ok(bytes) => {
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(pos) = buf.find('\n') {
                        let line: String = buf.drain(..=pos).collect();
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }

                        // Skip "event: ..." lines — we use the "type" field in data
                        let Some(data) = line.strip_prefix("data: ") else {
                            continue;
                        };

                        let Ok(v) = serde_json::from_str::<Value>(data) else {
                            continue;
                        };

                        let event_type = v["type"].as_str().unwrap_or("");

                        match event_type {
                            // ── New content block (text or tool_use) ──
                            "content_block_start" => {
                                let index = v["index"].as_u64().unwrap_or(0) as usize;
                                let block = &v["content_block"];
                                if block["type"].as_str() == Some("tool_use") {
                                    let id = block["id"].as_str().unwrap_or("").to_string();
                                    let name = block["name"].as_str().unwrap_or("").to_string();
                                    active_tools.push((index, id.clone(), name.clone()));

                                    // Emit the first delta so the consumer knows a
                                    // tool call is starting.
                                    let _ = tx
                                        .send(Ok(StreamChunk::with_tool_calls(vec![
                                            ToolCallDelta::first(index, id, name, String::new()),
                                        ])))
                                        .await;
                                }
                                // text blocks don't need special handling at start
                            }

                            // ── Incremental content ──
                            "content_block_delta" => {
                                let index = v["index"].as_u64().unwrap_or(0) as usize;
                                let delta_obj = &v["delta"];
                                let delta_type = delta_obj["type"].as_str().unwrap_or("");

                                match delta_type {
                                    "text_delta" => {
                                        let text =
                                            delta_obj["text"].as_str().unwrap_or("").to_string();
                                        if !text.is_empty() {
                                            let _ = tx.send(Ok(StreamChunk::delta(text))).await;
                                        }
                                    }
                                    "input_json_delta" => {
                                        let partial = delta_obj["partial_json"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        if !partial.is_empty() {
                                            // Look up id/name from active_tools so
                                            // consumers can associate deltas with
                                            // the correct tool call.
                                            let (id, name) = active_tools
                                                .iter()
                                                .find(|(idx, _, _)| *idx == index)
                                                .map(|(_, id, name)| {
                                                    (Some(id.clone()), Some(name.clone()))
                                                })
                                                .unwrap_or((None, None));
                                            let tcd = match (id, name) {
                                                (Some(id), Some(name)) => {
                                                    ToolCallDelta::first(index, id, name, partial)
                                                }
                                                _ => ToolCallDelta::new(index, partial),
                                            };
                                            let _ = tx
                                                .send(Ok(StreamChunk::with_tool_calls(vec![tcd])))
                                                .await;
                                        }
                                    }
                                    other => {
                                        warn!(
                                            delta_type = other,
                                            "Unknown Anthropic content_block_delta type"
                                        );
                                    }
                                }
                            }

                            // ── Message-level stop reason ──
                            "message_delta" => {
                                let stop =
                                    v["delta"]["stop_reason"].as_str().and_then(|r| match r {
                                        "end_turn" => Some(StopReason::EndTurn),
                                        "tool_use" => Some(StopReason::ToolUse),
                                        "max_tokens" => Some(StopReason::MaxTokens),
                                        "stop_sequence" => Some(StopReason::StopSequence),
                                        _ => None,
                                    });

                                if let Some(reason) = stop {
                                    let _ = tx.send(Ok(StreamChunk::done(reason))).await;
                                }
                            }

                            // ── Stream termination ──
                            "message_stop" => {
                                // Pure terminator — the real stop_reason was
                                // already emitted via `message_delta`.
                                return;
                            }

                            // message_start, content_block_stop, ping — ignored
                            _ => {}
                        }
                    }
                }
            }
        }
    });

    Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
}

/// Parse an Ollama NDJSON streaming response into a `BoxStream<StreamChunk>`.
///
/// Ollama uses newline-delimited JSON (not SSE), one JSON object per line:
///
/// ```text
/// {"message":{"role":"assistant","content":"Hi"},"done":false}
/// {"message":{"role":"assistant","content":"","tool_calls":[...]},"done":true}
/// ```
pub fn parse_ollama_ndjson_stream(response: reqwest::Response) -> BoxStream<StreamChunk> {
    let (tx, rx) = tokio::sync::mpsc::channel::<anyhow::Result<StreamChunk>>(64);

    tokio::spawn(async move {
        let mut byte_stream = response.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = byte_stream.next().await {
            match chunk {
                Err(e) => {
                    let _ = tx.send(Err(anyhow::anyhow!(e))).await;
                    break;
                }
                Ok(bytes) => {
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(pos) = buf.find('\n') {
                        let line: String = buf.drain(..=pos).collect();
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }

                        let Ok(v) = serde_json::from_str::<Value>(line) else {
                            continue;
                        };

                        let delta = v["message"]["content"].as_str().unwrap_or("").to_string();
                        let done = v["done"].as_bool().unwrap_or(false);

                        // ── Parse tool calls from the final message ──
                        let mut tool_calls = vec![];
                        if let Some(tc_arr) = v["message"]["tool_calls"].as_array() {
                            for (i, tc) in tc_arr.iter().enumerate() {
                                let name =
                                    tc["function"]["name"].as_str().unwrap_or("").to_string();
                                let args = tc["function"]["arguments"].to_string();
                                tool_calls.push(ToolCallDelta::first(
                                    i,
                                    uuid::Uuid::new_v4().to_string(),
                                    name,
                                    args,
                                ));
                            }
                        }

                        let stop_reason = if done {
                            if tool_calls.is_empty() {
                                Some(StopReason::EndTurn)
                            } else {
                                Some(StopReason::ToolUse)
                            }
                        } else {
                            None
                        };

                        let _ = tx
                            .send(Ok(StreamChunk::new(delta, done, tool_calls, stop_reason)))
                            .await;

                        if done {
                            return;
                        }
                    }
                }
            }
        }
    });

    Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a mock response from raw bytes.
    async fn mock_response(body: &'static [u8]) -> reqwest::Response {
        let mut server = mockito::Server::new_async().await;
        let mock = server.mock("GET", "/").with_body(body).create_async().await;
        let resp = reqwest::get(server.url()).await.unwrap();
        mock.assert_async().await;
        resp
    }

    #[tokio::test]
    async fn openai_sse_parses_text_deltas() {
        let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n\
                     data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n\
                     data: [DONE]\n\n";
        let resp = mock_response(body).await;
        let mut stream = parse_openai_sse_stream(resp);

        let mut text = String::new();
        while let Some(Ok(chunk)) = StreamExt::next(&mut stream).await {
            text.push_str(&chunk.delta);
            if chunk.done {
                break;
            }
        }
        assert_eq!(text, "Hello world");
    }

    #[tokio::test]
    async fn openai_sse_parses_tool_call_deltas() {
        let body = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call-1\",\"function\":{\"name\":\"shell\",\"arguments\":\"\"}}]}}]}\n\n\
                     data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"cmd\\\"\"}}]}}]}\n\n\
                     data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\": \\\"ls\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n\
                     data: [DONE]\n\n";
        let resp = mock_response(body).await;
        let mut stream = parse_openai_sse_stream(resp);

        let mut tool_deltas: Vec<ToolCallDelta> = vec![];
        let mut final_stop = None;
        while let Some(Ok(chunk)) = StreamExt::next(&mut stream).await {
            tool_deltas.extend(chunk.tool_calls);
            if let Some(sr) = chunk.stop_reason {
                final_stop = Some(sr);
            }
            if chunk.done {
                break;
            }
        }
        assert!(!tool_deltas.is_empty());
        assert_eq!(tool_deltas[0].id.as_deref(), Some("call-1"));
        assert_eq!(tool_deltas[0].name.as_deref(), Some("shell"));
        assert_eq!(final_stop, Some(StopReason::ToolUse));
    }

    #[tokio::test]
    async fn anthropic_sse_parses_text_deltas() {
        let body = b"event: message_start\n\
                     data: {\"type\":\"message_start\"}\n\n\
                     event: content_block_start\n\
                     data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
                     event: content_block_delta\n\
                     data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n\
                     event: content_block_delta\n\
                     data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" there\"}}\n\n\
                     event: message_delta\n\
                     data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n\
                     event: message_stop\n\
                     data: {\"type\":\"message_stop\"}\n\n";
        let resp = mock_response(body).await;
        let mut stream = parse_anthropic_sse_stream(resp);

        let mut text = String::new();
        let mut stop = None;
        while let Some(Ok(chunk)) = StreamExt::next(&mut stream).await {
            text.push_str(&chunk.delta);
            if let Some(sr) = chunk.stop_reason {
                stop = Some(sr);
            }
            if chunk.done {
                break;
            }
        }
        assert_eq!(text, "Hi there");
        assert_eq!(stop, Some(StopReason::EndTurn));
    }

    #[tokio::test]
    async fn anthropic_sse_parses_tool_use_blocks() {
        let body = b"event: content_block_start\n\
                     data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"shell\"}}\n\n\
                     event: content_block_delta\n\
                     data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"cmd\\\":\"}}\n\n\
                     event: content_block_delta\n\
                     data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"ls\\\"}\"}}\n\n\
                     event: message_delta\n\
                     data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"}}\n\n\
                     event: message_stop\n\
                     data: {\"type\":\"message_stop\"}\n\n";
        let resp = mock_response(body).await;
        let mut stream = parse_anthropic_sse_stream(resp);

        let mut tool_deltas: Vec<ToolCallDelta> = vec![];
        let mut final_stop = None;
        while let Some(Ok(chunk)) = StreamExt::next(&mut stream).await {
            tool_deltas.extend(chunk.tool_calls);
            if let Some(sr) = chunk.stop_reason {
                final_stop = Some(sr);
            }
            if chunk.done {
                break;
            }
        }
        assert!(tool_deltas.len() >= 2);
        assert_eq!(tool_deltas[0].id.as_deref(), Some("toolu_1"));
        assert_eq!(tool_deltas[0].name.as_deref(), Some("shell"));
        // input_json_delta chunks now carry id/name from active_tools lookup
        assert_eq!(tool_deltas[1].id.as_deref(), Some("toolu_1"));
        assert_eq!(tool_deltas[1].name.as_deref(), Some("shell"));
        assert_eq!(final_stop, Some(StopReason::ToolUse));
    }

    #[tokio::test]
    async fn ollama_ndjson_parses_text() {
        let body = b"{\"message\":{\"role\":\"assistant\",\"content\":\"Hello\"},\"done\":false}\n\
                     {\"message\":{\"role\":\"assistant\",\"content\":\" world\"},\"done\":true}\n";
        let resp = mock_response(body).await;
        let mut stream = parse_ollama_ndjson_stream(resp);

        let mut text = String::new();
        while let Some(Ok(chunk)) = StreamExt::next(&mut stream).await {
            text.push_str(&chunk.delta);
            if chunk.done {
                assert_eq!(chunk.stop_reason, Some(StopReason::EndTurn));
                break;
            }
        }
        assert_eq!(text, "Hello world");
    }
}
