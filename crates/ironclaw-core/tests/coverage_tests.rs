//! Comprehensive tests targeting coverage gaps in ironclaw-core.
//! Covers: error Display/source/From, type constructors, builders,
//! Display impls, trait default methods.

use async_trait::async_trait;
use futures::stream;
use ironclaw_core::*;

// ═══════════════════════════════════════════════════════════════════════════
// ERROR COVERAGE
// ═══════════════════════════════════════════════════════════════════════════

// ── ProviderError ──────────────────────────────────────────────────────────

#[test]
fn provider_error_request_display() {
    let e = ProviderError::Request("timeout".into());
    assert!(e.to_string().contains("timeout"));
}

#[test]
fn provider_error_auth_display() {
    let e = ProviderError::Auth("expired token".into());
    assert!(e.to_string().contains("expired token"));
}

#[test]
fn provider_error_rate_limit_no_retry() {
    let e = ProviderError::RateLimit {
        retry_after_ms: None,
    };
    assert!(e.to_string().contains("Rate limited"));
}

#[test]
fn provider_error_rate_limit_with_retry() {
    let e = ProviderError::RateLimit {
        retry_after_ms: Some(3000),
    };
    assert!(e.to_string().contains("3000"));
}

#[test]
fn provider_error_model_not_found_display() {
    let e = ProviderError::ModelNotFound("gpt-99".into());
    assert!(e.to_string().contains("gpt-99"));
}

#[test]
fn provider_error_invalid_response_display() {
    let e = ProviderError::InvalidResponse("missing field".into());
    assert!(e.to_string().contains("missing field"));
}

#[test]
fn provider_error_stream_terminated_display() {
    let e = ProviderError::StreamTerminated;
    assert!(e.to_string().contains("terminated"));
}

#[test]
fn provider_error_other_display() {
    let e = ProviderError::Other(anyhow::anyhow!("custom"));
    assert!(e.to_string().contains("custom"));
}

#[test]
fn provider_error_source_request() {
    use std::error::Error;
    let e = ProviderError::Request("x".into());
    assert!(e.source().is_none());
}

#[test]
fn provider_error_source_auth() {
    use std::error::Error;
    let e = ProviderError::Auth("x".into());
    assert!(e.source().is_none());
}

#[test]
fn provider_error_source_rate_limit() {
    use std::error::Error;
    let e = ProviderError::RateLimit {
        retry_after_ms: None,
    };
    assert!(e.source().is_none());
}

#[test]
fn provider_error_source_model_not_found() {
    use std::error::Error;
    let e = ProviderError::ModelNotFound("x".into());
    assert!(e.source().is_none());
}

#[test]
fn provider_error_source_invalid_response() {
    use std::error::Error;
    let e = ProviderError::InvalidResponse("x".into());
    assert!(e.source().is_none());
}

#[test]
fn provider_error_source_stream_terminated() {
    use std::error::Error;
    let e = ProviderError::StreamTerminated;
    assert!(e.source().is_none());
}

#[test]
fn provider_error_source_other() {
    use std::error::Error;
    let e = ProviderError::Other(anyhow::anyhow!("x"));
    assert!(e.source().is_some());
}

#[test]
fn provider_error_from_anyhow() {
    let e: ProviderError = anyhow::anyhow!("fail").into();
    assert!(matches!(e, ProviderError::Other(_)));
}

// ── ChannelError ───────────────────────────────────────────────────────────

#[test]
fn channel_error_start_failed_display() {
    let e = ChannelError::StartFailed("port in use".into());
    assert!(e.to_string().contains("port in use"));
}

#[test]
fn channel_error_send_failed_display() {
    let e = ChannelError::SendFailed("network".into());
    assert!(e.to_string().contains("network"));
}

#[test]
fn channel_error_not_running_display() {
    let e = ChannelError::NotRunning;
    assert!(e.to_string().contains("not running"));
}

#[test]
fn channel_error_other_display() {
    let e = ChannelError::Other(anyhow::anyhow!("misc"));
    assert!(e.to_string().contains("misc"));
}

#[test]
fn channel_error_source_start_failed() {
    use std::error::Error;
    let e = ChannelError::StartFailed("x".into());
    assert!(e.source().is_none());
}

#[test]
fn channel_error_source_send_failed() {
    use std::error::Error;
    let e = ChannelError::SendFailed("x".into());
    assert!(e.source().is_none());
}

#[test]
fn channel_error_source_not_running() {
    use std::error::Error;
    let e = ChannelError::NotRunning;
    assert!(e.source().is_none());
}

#[test]
fn channel_error_source_other() {
    use std::error::Error;
    let e = ChannelError::Other(anyhow::anyhow!("x"));
    assert!(e.source().is_some());
}

#[test]
fn channel_error_from_anyhow() {
    let e: ChannelError = anyhow::anyhow!("fail").into();
    assert!(matches!(e, ChannelError::Other(_)));
}

// ── ToolError ──────────────────────────────────────────────────────────────

#[test]
fn tool_error_invalid_params_display() {
    let e = ToolError::InvalidParams("missing query".into());
    assert!(e.to_string().contains("missing query"));
}

#[test]
fn tool_error_execution_failed_display() {
    let e = ToolError::ExecutionFailed("file not found".into());
    assert!(e.to_string().contains("file not found"));
}

#[test]
fn tool_error_timeout_display() {
    let e = ToolError::Timeout;
    assert!(e.to_string().contains("timed out"));
}

#[test]
fn tool_error_other_display() {
    let e = ToolError::Other(anyhow::anyhow!("xyz"));
    assert!(e.to_string().contains("xyz"));
}

#[test]
fn tool_error_source_invalid_params() {
    use std::error::Error;
    let e = ToolError::InvalidParams("x".into());
    assert!(e.source().is_none());
}

#[test]
fn tool_error_source_execution_failed() {
    use std::error::Error;
    let e = ToolError::ExecutionFailed("x".into());
    assert!(e.source().is_none());
}

#[test]
fn tool_error_source_timeout() {
    use std::error::Error;
    let e = ToolError::Timeout;
    assert!(e.source().is_none());
}

#[test]
fn tool_error_source_other() {
    use std::error::Error;
    let e = ToolError::Other(anyhow::anyhow!("x"));
    assert!(e.source().is_some());
}

#[test]
fn tool_error_from_anyhow() {
    let e: ToolError = anyhow::anyhow!("fail").into();
    assert!(matches!(e, ToolError::Other(_)));
}

// ── MemoryError ────────────────────────────────────────────────────────────

#[test]
fn memory_error_storage_display() {
    let e = MemoryError::Storage("disk full".into());
    assert!(e.to_string().contains("disk full"));
}

#[test]
fn memory_error_serialization_display() {
    let e = MemoryError::Serialization("bad utf8".into());
    assert!(e.to_string().contains("bad utf8"));
}

#[test]
fn memory_error_not_found_display() {
    let e = MemoryError::NotFound("session-99".into());
    assert!(e.to_string().contains("session-99"));
}

#[test]
fn memory_error_other_display() {
    let e = MemoryError::Other(anyhow::anyhow!("oops"));
    assert!(e.to_string().contains("oops"));
}

#[test]
fn memory_error_source_storage() {
    use std::error::Error;
    let e = MemoryError::Storage("x".into());
    assert!(e.source().is_none());
}

#[test]
fn memory_error_source_serialization() {
    use std::error::Error;
    let e = MemoryError::Serialization("x".into());
    assert!(e.source().is_none());
}

#[test]
fn memory_error_source_not_found() {
    use std::error::Error;
    let e = MemoryError::NotFound("x".into());
    assert!(e.source().is_none());
}

#[test]
fn memory_error_source_other() {
    use std::error::Error;
    let e = MemoryError::Other(anyhow::anyhow!("x"));
    assert!(e.source().is_some());
}

#[test]
fn memory_error_from_anyhow() {
    let e: MemoryError = anyhow::anyhow!("fail").into();
    assert!(matches!(e, MemoryError::Other(_)));
}

// ── AgentError ─────────────────────────────────────────────────────────────

#[test]
fn agent_error_provider_display() {
    let pe = ProviderError::Auth("key expired".into());
    let ae = AgentError::Provider(pe);
    assert!(ae.to_string().contains("key expired"));
}

#[test]
fn agent_error_tool_display() {
    let te = ToolError::Timeout;
    let ae = AgentError::Tool(te);
    assert!(ae.to_string().contains("timed out"));
}

#[test]
fn agent_error_budget_exceeded_display() {
    let ae = AgentError::BudgetExceeded("100 iterations".into());
    assert!(ae.to_string().contains("100 iterations"));
}

#[test]
fn agent_error_not_found_display() {
    let ae = AgentError::NotFound("agent-42".into());
    assert!(ae.to_string().contains("agent-42"));
}

#[test]
fn agent_error_other_display() {
    let ae = AgentError::Other(anyhow::anyhow!("misc"));
    assert!(ae.to_string().contains("misc"));
}

#[test]
fn agent_error_source_provider() {
    use std::error::Error;
    let ae = AgentError::Provider(ProviderError::StreamTerminated);
    assert!(ae.source().is_some());
}

#[test]
fn agent_error_source_tool() {
    use std::error::Error;
    let ae = AgentError::Tool(ToolError::Timeout);
    assert!(ae.source().is_some());
}

#[test]
fn agent_error_source_budget() {
    use std::error::Error;
    let ae = AgentError::BudgetExceeded("x".into());
    assert!(ae.source().is_none());
}

#[test]
fn agent_error_source_not_found() {
    use std::error::Error;
    let ae = AgentError::NotFound("x".into());
    assert!(ae.source().is_none());
}

#[test]
fn agent_error_source_other() {
    use std::error::Error;
    let ae = AgentError::Other(anyhow::anyhow!("x"));
    assert!(ae.source().is_some());
}

#[test]
fn agent_error_from_provider() {
    let pe = ProviderError::Auth("bad".into());
    let ae: AgentError = pe.into();
    assert!(matches!(ae, AgentError::Provider(_)));
}

#[test]
fn agent_error_from_tool() {
    let te = ToolError::Timeout;
    let ae: AgentError = te.into();
    assert!(matches!(ae, AgentError::Tool(_)));
}

#[test]
fn agent_error_from_anyhow() {
    let ae: AgentError = anyhow::anyhow!("fail").into();
    assert!(matches!(ae, AgentError::Other(_)));
}

// ── HandlerError ───────────────────────────────────────────────────────────

#[test]
fn handler_error_agent_display() {
    let ae = AgentError::NotFound("x".into());
    let he = HandlerError::Agent(ae);
    assert!(he.to_string().contains("Agent not found"));
}

#[test]
fn handler_error_channel_display() {
    let ce = ChannelError::NotRunning;
    let he = HandlerError::Channel(ce);
    assert!(he.to_string().contains("not running"));
}

#[test]
fn handler_error_other_display() {
    let he = HandlerError::Other(anyhow::anyhow!("misc"));
    assert!(he.to_string().contains("misc"));
}

#[test]
fn handler_error_source_agent() {
    use std::error::Error;
    let he = HandlerError::Agent(AgentError::BudgetExceeded("x".into()));
    assert!(he.source().is_some());
}

#[test]
fn handler_error_source_channel() {
    use std::error::Error;
    let he = HandlerError::Channel(ChannelError::NotRunning);
    assert!(he.source().is_some());
}

#[test]
fn handler_error_source_other() {
    use std::error::Error;
    let he = HandlerError::Other(anyhow::anyhow!("x"));
    assert!(he.source().is_some());
}

#[test]
fn handler_error_from_agent() {
    let ae = AgentError::NotFound("agent-1".into());
    let he: HandlerError = ae.into();
    assert!(matches!(he, HandlerError::Agent(_)));
}

#[test]
fn handler_error_from_channel() {
    let ce = ChannelError::NotRunning;
    let he: HandlerError = ce.into();
    assert!(matches!(he, HandlerError::Channel(_)));
}

#[test]
fn handler_error_from_anyhow() {
    let he: HandlerError = anyhow::anyhow!("fail").into();
    assert!(matches!(he, HandlerError::Other(_)));
}

// ═══════════════════════════════════════════════════════════════════════════
// TYPES COVERAGE
// ═══════════════════════════════════════════════════════════════════════════

// ── SessionId / AgentId ────────────────────────────────────────────────────

#[test]
fn session_id_display() {
    let id = SessionId::new("sess-1");
    assert_eq!(format!("{id}"), "sess-1");
}

#[test]
fn session_id_from_string() {
    let id: SessionId = String::from("hello").into();
    assert_eq!(id.as_str(), "hello");
}

#[test]
fn session_id_from_str_ref() {
    let id: SessionId = "test".into();
    assert_eq!(id.as_str(), "test");
}

#[test]
fn session_id_as_ref_str() {
    let id = SessionId::new("x");
    let r: &str = id.as_ref();
    assert_eq!(r, "x");
}

#[test]
fn session_id_deref() {
    let id = SessionId::new("abc");
    assert_eq!(&*id, "abc");
}

#[test]
fn session_id_eq_hash() {
    use std::collections::HashSet;
    let a = SessionId::new("one");
    let b = SessionId::new("one");
    let c = SessionId::new("two");
    assert_eq!(a, b);
    assert_ne!(a, c);
    let mut set = HashSet::new();
    set.insert(a);
    set.insert(b);
    assert_eq!(set.len(), 1);
}

#[test]
fn agent_id_display() {
    let id = AgentId::new("bot-1");
    assert_eq!(format!("{id}"), "bot-1");
}

#[test]
fn agent_id_from_string() {
    let id: AgentId = String::from("agent").into();
    assert_eq!(id.as_str(), "agent");
}

#[test]
fn agent_id_from_str_ref() {
    let id: AgentId = "test".into();
    assert_eq!(id.as_str(), "test");
}

#[test]
fn agent_id_as_ref() {
    let id = AgentId::new("x");
    let r: &str = id.as_ref();
    assert_eq!(r, "x");
}

#[test]
fn agent_id_deref() {
    let id = AgentId::new("def");
    assert_eq!(&*id, "def");
}

// ── Role ───────────────────────────────────────────────────────────────────

#[test]
fn role_serde_system() {
    let j = serde_json::to_string(&Role::System).unwrap();
    assert_eq!(j, "\"system\"");
    let r: Role = serde_json::from_str(&j).unwrap();
    assert_eq!(r, Role::System);
}

#[test]
fn role_serde_user() {
    let j = serde_json::to_string(&Role::User).unwrap();
    assert_eq!(j, "\"user\"");
}

#[test]
fn role_serde_assistant() {
    let j = serde_json::to_string(&Role::Assistant).unwrap();
    assert_eq!(j, "\"assistant\"");
}

#[test]
fn role_serde_tool() {
    let j = serde_json::to_string(&Role::Tool).unwrap();
    assert_eq!(j, "\"tool\"");
}

// ── Message ────────────────────────────────────────────────────────────────

#[test]
fn message_with_all_roundtrips() {
    let id = uuid::Uuid::new_v4();
    let ts = chrono::Utc::now();
    let tc = vec![ToolCall::new("c1", "shell", serde_json::json!({}))];
    let tr = Some(ToolResult::new("c1", serde_json::json!({"ok": true})));
    let msg = Message::with_all(id, Role::Tool, "result", tc, tr, ts);
    let j = serde_json::to_string(&msg).unwrap();
    let msg2: Message = serde_json::from_str(&j).unwrap();
    assert_eq!(msg2.id, id);
    assert_eq!(msg2.role, Role::Tool);
    assert_eq!(msg2.tool_calls.len(), 1);
    assert!(msg2.tool_result.is_some());
}

#[test]
fn message_tool_result_constructor() {
    let m = Message::tool_result("call-1", serde_json::json!(42));
    assert_eq!(m.role, Role::Tool);
    assert!(m.content.is_empty());
    assert!(m.tool_calls.is_empty());
    let tr = m.tool_result.unwrap();
    assert_eq!(tr.call_id, "call-1");
}

// ── TokenUsage ─────────────────────────────────────────────────────────────

#[test]
fn token_usage_new() {
    let u = TokenUsage::new(10, 20, 30);
    assert_eq!(u.prompt_tokens, 10);
    assert_eq!(u.completion_tokens, 20);
    assert_eq!(u.total_tokens, 30);
}

// ── StopReason ─────────────────────────────────────────────────────────────

#[test]
fn stop_reason_max_tokens_serde() {
    let j = serde_json::to_string(&StopReason::MaxTokens).unwrap();
    assert_eq!(j, "\"max_tokens\"");
}

#[test]
fn stop_reason_stop_sequence_serde() {
    let j = serde_json::to_string(&StopReason::StopSequence).unwrap();
    assert_eq!(j, "\"stop_sequence\"");
}

// ── ResponseFormat ─────────────────────────────────────────────────────────

#[test]
fn response_format_default_is_text() {
    assert_eq!(ResponseFormat::default(), ResponseFormat::Text);
}

#[test]
fn response_format_json_object_serde() {
    let j = serde_json::to_string(&ResponseFormat::JsonObject).unwrap();
    assert_eq!(j, "\"json_object\"");
    let rf: ResponseFormat = serde_json::from_str(&j).unwrap();
    assert_eq!(rf, ResponseFormat::JsonObject);
}

// ── CompletionRequest builder ──────────────────────────────────────────────

#[test]
fn completion_request_builder_all_fields() {
    let schema = ToolSchema::new("tool1", "desc", serde_json::json!({}));
    let req = CompletionRequest::builder(vec![Message::user("hi")])
        .tools(vec![schema])
        .max_tokens(100)
        .temperature(0.5)
        .stream(true)
        .model("gpt-4")
        .response_format(ResponseFormat::JsonObject)
        .build();

    assert_eq!(req.tools.len(), 1);
    assert_eq!(req.max_tokens, Some(100));
    assert_eq!(req.temperature, Some(0.5));
    assert!(req.stream);
    assert_eq!(req.model.as_deref(), Some("gpt-4"));
    assert_eq!(req.response_format, ResponseFormat::JsonObject);
}

// ── CompletionResponse ────────────────────────────────────────────────────

#[test]
fn completion_response_text() {
    let resp = CompletionResponse::new(
        Message::assistant("hello"),
        StopReason::EndTurn,
        TokenUsage::default(),
        "model-1",
        10,
    );
    assert_eq!(resp.text(), "hello");
    assert!(!resp.has_tool_calls());
}

#[test]
fn completion_response_has_tool_calls() {
    let mut msg = Message::assistant("");
    msg.tool_calls = vec![ToolCall::new("c1", "shell", serde_json::json!({}))];
    let resp = CompletionResponse::new(
        msg,
        StopReason::ToolUse,
        TokenUsage::default(),
        "model-1",
        5,
    );
    assert!(resp.has_tool_calls());
}

// ── ToolCallDelta ──────────────────────────────────────────────────────────

#[test]
fn tool_call_delta_new() {
    let d = ToolCallDelta::new(1, r#"{"key":"#);
    assert_eq!(d.index, 1);
    assert!(d.id.is_none());
    assert!(d.name.is_none());
    assert_eq!(d.arguments_delta, r#"{"key":"#);
}

#[test]
fn tool_call_delta_first() {
    let d = ToolCallDelta::first(0, "c1", "shell", "{}");
    assert_eq!(d.id.as_deref(), Some("c1"));
    assert_eq!(d.name.as_deref(), Some("shell"));
}

// ── StreamChunk ────────────────────────────────────────────────────────────

#[test]
fn stream_chunk_delta_constructor() {
    let c = StreamChunk::delta("hello");
    assert_eq!(c.delta, "hello");
    assert!(!c.done);
    assert!(c.tool_calls.is_empty());
    assert!(c.stop_reason.is_none());
}

#[test]
fn stream_chunk_done_constructor() {
    let c = StreamChunk::done(StopReason::EndTurn);
    assert!(c.done);
    assert!(c.delta.is_empty());
    assert_eq!(c.stop_reason, Some(StopReason::EndTurn));
}

#[test]
fn stream_chunk_with_tool_calls_constructor() {
    let tc = vec![ToolCallDelta::new(0, "{}")];
    let c = StreamChunk::with_tool_calls(tc);
    assert!(!c.done);
    assert_eq!(c.tool_calls.len(), 1);
}

// ── StreamEvent ────────────────────────────────────────────────────────────

#[test]
fn stream_event_tool_call_end_serde() {
    let evt = StreamEvent::ToolCallEnd {
        id: "call-1".into(),
        result: serde_json::json!({"files": ["a.rs"]}),
    };
    let j = serde_json::to_string(&evt).unwrap();
    assert!(j.contains("tool_call_end"));
    let evt2: StreamEvent = serde_json::from_str(&j).unwrap();
    if let StreamEvent::ToolCallEnd { id, .. } = evt2 {
        assert_eq!(id, "call-1");
    } else {
        panic!("Expected ToolCallEnd");
    }
}

#[test]
fn stream_event_error_serde() {
    let evt = StreamEvent::Error {
        message: "oom".into(),
    };
    let j = serde_json::to_string(&evt).unwrap();
    assert!(j.contains("error"));
    assert!(j.contains("oom"));
}

// ── ToolSchema ─────────────────────────────────────────────────────────────

#[test]
fn tool_schema_new() {
    let s = ToolSchema::new(
        "search",
        "Search the web",
        serde_json::json!({"type": "object"}),
    );
    assert_eq!(s.name, "search");
    assert_eq!(s.description, "Search the web");
}

// ── MemoryHit ──────────────────────────────────────────────────────────────

#[test]
fn memory_hit_new() {
    let h = MemoryHit::new(
        "id-1",
        "some text",
        0.95,
        serde_json::json!({"source": "doc"}),
    );
    assert_eq!(h.id, "id-1");
    assert_eq!(h.text, "some text");
    assert!((h.score - 0.95).abs() < f32::EPSILON);
}

#[test]
fn memory_hit_roundtrips() {
    let h = MemoryHit::new("id-2", "text", 0.8, serde_json::json!(null));
    let j = serde_json::to_string(&h).unwrap();
    let h2: MemoryHit = serde_json::from_str(&j).unwrap();
    assert_eq!(h2.id, "id-2");
}

// ── SearchHit ──────────────────────────────────────────────────────────────

#[test]
fn search_hit_new() {
    let msg = Message::user("hello");
    let h = SearchHit::new("session-1", msg);
    assert_eq!(h.session_id.as_str(), "session-1");
    assert_eq!(h.message.content, "hello");
}

#[test]
fn search_hit_roundtrips() {
    let h = SearchHit::new("s1", Message::assistant("reply"));
    let j = serde_json::to_string(&h).unwrap();
    let h2: SearchHit = serde_json::from_str(&j).unwrap();
    assert_eq!(h2.session_id.as_str(), "s1");
}

// ── AgentTask / Builder ────────────────────────────────────────────────────

#[test]
fn agent_task_new() {
    let t = AgentTask::new("summarize this");
    assert_eq!(t.instruction, "summarize this");
    assert!(t.context.is_empty());
    assert!(t.tool_allowlist.is_none());
    assert!(t.max_tokens.is_none());
}

#[test]
fn agent_task_builder_all_fields() {
    let id = uuid::Uuid::new_v4();
    let t = AgentTask::builder("do work")
        .id(id)
        .context(vec![Message::system("you are helpful")])
        .tool_allowlist(vec!["shell".into(), "search".into()])
        .max_tokens(1000)
        .build();
    assert_eq!(t.id, id);
    assert_eq!(t.instruction, "do work");
    assert_eq!(t.context.len(), 1);
    assert_eq!(t.tool_allowlist.as_ref().unwrap().len(), 2);
    assert_eq!(t.max_tokens, Some(1000));
}

#[test]
fn agent_task_roundtrips() {
    let t = AgentTask::new("test task");
    let j = serde_json::to_string(&t).unwrap();
    let t2: AgentTask = serde_json::from_str(&j).unwrap();
    assert_eq!(t2.instruction, "test task");
}

// ── AgentOutput ────────────────────────────────────────────────────────────

#[test]
fn agent_output_new() {
    let id = uuid::Uuid::new_v4();
    let o = AgentOutput::new(id, "bot-1", "result text");
    assert_eq!(o.task_id, id);
    assert_eq!(o.agent_id.as_str(), "bot-1");
    assert_eq!(o.text, "result text");
    assert!(!o.approved);
    assert!(o.tool_calls.is_empty());
}

#[test]
fn agent_output_with_methods() {
    let id = uuid::Uuid::new_v4();
    let tc = vec![ToolCall::new("c1", "shell", serde_json::json!({}))];
    let usage = TokenUsage::new(10, 20, 30);
    let o = AgentOutput::new(id, "bot", "text")
        .with_tool_calls(tc)
        .with_approved(true)
        .with_usage(usage);
    assert!(o.approved);
    assert_eq!(o.tool_calls.len(), 1);
    assert_eq!(o.usage.total_tokens, 30);
}

#[test]
fn agent_output_roundtrips() {
    let o = AgentOutput::new(uuid::Uuid::new_v4(), "bot-1", "out");
    let j = serde_json::to_string(&o).unwrap();
    let o2: AgentOutput = serde_json::from_str(&j).unwrap();
    assert_eq!(o2.text, "out");
}

// ── AgentRole ──────────────────────────────────────────────────────────────

#[test]
fn agent_role_variants_serde() {
    for role in [
        AgentRole::Orchestrator,
        AgentRole::Worker,
        AgentRole::Router,
        AgentRole::Critic,
        AgentRole::Planner,
    ] {
        let j = serde_json::to_string(&role).unwrap();
        let r2: AgentRole = serde_json::from_str(&j).unwrap();
        assert_eq!(format!("{r2:?}"), format!("{role:?}"));
    }
}

// ── AgentState ─────────────────────────────────────────────────────────────

#[test]
fn agent_state_default_is_idle() {
    assert_eq!(AgentState::default(), AgentState::Idle);
}

#[test]
fn agent_state_display() {
    assert_eq!(AgentState::Idle.to_string(), "Idle");
    assert_eq!(AgentState::Running.to_string(), "Running");
    assert_eq!(AgentState::Waiting.to_string(), "Waiting");
    assert_eq!(AgentState::Done.to_string(), "Done");
    assert_eq!(AgentState::Failed("err".into()).to_string(), "Failed: err");
}

#[test]
fn agent_state_eq() {
    assert_eq!(AgentState::Idle, AgentState::Idle);
    assert_ne!(AgentState::Idle, AgentState::Running);
    assert_eq!(
        AgentState::Failed("a".into()),
        AgentState::Failed("a".into())
    );
    assert_ne!(
        AgentState::Failed("a".into()),
        AgentState::Failed("b".into())
    );
}

#[test]
fn agent_state_serde_roundtrip() {
    for state in [
        AgentState::Idle,
        AgentState::Running,
        AgentState::Waiting,
        AgentState::Done,
        AgentState::Failed("oops".into()),
    ] {
        let j = serde_json::to_string(&state).unwrap();
        let s2: AgentState = serde_json::from_str(&j).unwrap();
        assert_eq!(state, s2);
    }
}

// ── ChannelId ──────────────────────────────────────────────────────────────

#[test]
fn channel_id_variants_serde() {
    let variants: Vec<ChannelId> = vec![
        ChannelId::Telegram(12345),
        ChannelId::Discord("d-1".into()),
        ChannelId::Slack("s-1".into()),
        ChannelId::Rest("r-1".into()),
        ChannelId::WebSocket("ws-1".into()),
        ChannelId::Webhook("wh-1".into()),
        ChannelId::Matrix("m-1".into()),
        ChannelId::Cli,
        ChannelId::Custom("custom-1".into()),
    ];
    for ch in &variants {
        let j = serde_json::to_string(ch).unwrap();
        let ch2: ChannelId = serde_json::from_str(&j).unwrap();
        assert_eq!(format!("{ch:?}"), format!("{ch2:?}"));
    }
}

// ── InboundMessage / Builder ───────────────────────────────────────────────

#[test]
fn inbound_message_cli() {
    let m = InboundMessage::cli("test");
    assert_eq!(m.content, "test");
    assert!(matches!(m.channel, ChannelId::Cli));
    assert_eq!(m.session_id.as_str(), "cli-default");
    assert_eq!(m.author.as_deref(), Some("user"));
}

#[test]
fn inbound_message_builder_all_fields() {
    let m = InboundMessage::builder(ChannelId::Rest("s1".into()), "hello")
        .id("msg-1")
        .session_id("session-42")
        .author("bob")
        .timestamp(chrono::Utc::now())
        .build();
    assert_eq!(m.id, "msg-1");
    assert_eq!(m.content, "hello");
    assert_eq!(m.session_id.as_str(), "session-42");
    assert_eq!(m.author.as_deref(), Some("bob"));
}

#[test]
fn inbound_message_roundtrips() {
    let m = InboundMessage::cli("round trip test");
    let j = serde_json::to_string(&m).unwrap();
    let m2: InboundMessage = serde_json::from_str(&j).unwrap();
    assert_eq!(m2.content, "round trip test");
}

// ── OutboundMessage ────────────────────────────────────────────────────────

#[test]
fn outbound_message_text() {
    let m = OutboundMessage::text("s1", "reply");
    assert_eq!(m.as_str(), "reply");
    assert_eq!(m.session_id.as_str(), "s1");
    assert!(m.reply_to.is_none());
    assert!(matches!(m.content, OutboundContent::Text(_)));
}

#[test]
fn outbound_content_markdown() {
    let c = OutboundContent::Markdown("**bold**".into());
    if let OutboundContent::Markdown(s) = &c {
        assert_eq!(s, "**bold**");
    }
}

#[test]
fn outbound_message_roundtrips() {
    let m = OutboundMessage::text("s1", "yo");
    let j = serde_json::to_string(&m).unwrap();
    let m2: OutboundMessage = serde_json::from_str(&j).unwrap();
    assert_eq!(m2.as_str(), "yo");
}

// ═══════════════════════════════════════════════════════════════════════════
// TRAITS DEFAULT METHOD COVERAGE
// ═══════════════════════════════════════════════════════════════════════════

/// Minimal Provider for testing default trait methods.
struct StubProvider;

#[async_trait]
impl Provider for StubProvider {
    fn name(&self) -> &'static str {
        "stub"
    }

    async fn complete(&self, _req: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        Ok(CompletionResponse::new(
            Message::assistant("ok"),
            StopReason::EndTurn,
            TokenUsage::default(),
            "stub",
            0,
        ))
    }

    async fn stream(
        &self,
        _req: CompletionRequest,
    ) -> Result<BoxStream<StreamChunk>, ProviderError> {
        Ok(Box::pin(stream::empty()))
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}

#[test]
fn provider_default_supports_streaming() {
    let p = StubProvider;
    assert!(p.supports_streaming());
}

#[test]
fn provider_default_supports_tools() {
    let p = StubProvider;
    assert!(p.supports_tools());
}

#[test]
fn provider_default_supports_vision() {
    let p = StubProvider;
    assert!(!p.supports_vision());
}

/// Minimal MemoryStore for testing default trait methods.
struct StubMemoryStore;

#[async_trait]
impl MemoryStore for StubMemoryStore {
    async fn push(&self, _session: &SessionId, _msg: Message) -> Result<(), MemoryError> {
        Ok(())
    }
    async fn history(
        &self,
        _session: &SessionId,
        _limit: usize,
    ) -> Result<Vec<Message>, MemoryError> {
        Ok(vec![])
    }
    async fn clear(&self, _session: &SessionId) -> Result<(), MemoryError> {
        Ok(())
    }
}

#[tokio::test]
async fn memory_store_default_sessions() {
    let store = StubMemoryStore;
    let sessions = store.sessions().await.unwrap();
    assert!(sessions.is_empty());
}

#[tokio::test]
async fn memory_store_default_search() {
    let store = StubMemoryStore;
    let hits = store.search("query", 10).await.unwrap();
    assert!(hits.is_empty());
}

/// Minimal MessageHandler for testing default handle_stream.
struct StubHandler;

#[async_trait]
impl MessageHandler for StubHandler {
    async fn handle(&self, _msg: InboundMessage) -> Result<Option<OutboundMessage>, HandlerError> {
        Ok(Some(OutboundMessage::text("s1", "streamed reply")))
    }
}

#[tokio::test]
async fn message_handler_default_handle_stream() {
    use tokio_stream::StreamExt;

    let handler = StubHandler;
    let msg = InboundMessage::cli("test");
    let mut stream = handler.handle_stream(msg).await.unwrap();

    let first = stream.next().await.unwrap().unwrap();
    assert!(matches!(first, StreamEvent::TokenDelta { .. }));

    let second = stream.next().await.unwrap().unwrap();
    assert!(matches!(second, StreamEvent::Done { .. }));

    assert!(stream.next().await.is_none());
}

/// Handler that returns None to test that code path.
struct NoneHandler;

#[async_trait]
impl MessageHandler for NoneHandler {
    async fn handle(&self, _msg: InboundMessage) -> Result<Option<OutboundMessage>, HandlerError> {
        Ok(None)
    }
}

#[tokio::test]
async fn message_handler_handle_stream_none() {
    use tokio_stream::StreamExt;

    let handler = NoneHandler;
    let msg = InboundMessage::cli("test");
    let mut stream = handler.handle_stream(msg).await.unwrap();

    let first = stream.next().await.unwrap().unwrap();
    assert!(matches!(first, StreamEvent::Done { .. }));

    assert!(stream.next().await.is_none());
}
