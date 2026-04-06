//! Property-based tests for ironclaw-core types using proptest.

use ironclaw_core::{
    AgentId, AgentState, CompletionRequest, CompletionResponse, Message, Role, SessionId,
    StopReason, StreamChunk, StreamEvent, TokenUsage, ToolCall, ToolResult, ToolSchema,
};
use proptest::prelude::*;

// ── Strategies ─────────────────────────────────────────────────────────────

fn arb_stop_reason() -> impl Strategy<Value = StopReason> {
    prop_oneof![
        Just(StopReason::EndTurn),
        Just(StopReason::ToolUse),
        Just(StopReason::MaxTokens),
        Just(StopReason::StopSequence),
    ]
}

fn arb_token_usage() -> impl Strategy<Value = TokenUsage> {
    (0..10_000u32, 0..10_000u32).prop_map(|(p, c)| TokenUsage::new(p, c, p + c))
}

fn arb_tool_call() -> impl Strategy<Value = ToolCall> {
    ("[a-z]{1,20}", "[a-z_]{1,20}")
        .prop_map(|(id, name)| ToolCall::new(id, name, serde_json::json!({"key": "value"})))
}

fn arb_tool_result() -> impl Strategy<Value = ToolResult> {
    "[a-z]{1,20}".prop_map(|id| ToolResult::new(id, serde_json::json!({"ok": true})))
}

// ── Message round-trip ─────────────────────────────────────────────────────

proptest! {
    #[test]
    fn message_user_roundtrips(content in ".*") {
        let msg = Message::user(&content);
        let json = serde_json::to_string(&msg).unwrap();
        let msg2: Message = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&msg.content, &msg2.content);
        prop_assert_eq!(&msg.role, &msg2.role);
    }

    #[test]
    fn message_system_roundtrips(content in ".*") {
        let msg = Message::system(&content);
        let json = serde_json::to_string(&msg).unwrap();
        let msg2: Message = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&msg.content, &msg2.content);
        prop_assert_eq!(msg2.role, Role::System);
    }

    #[test]
    fn message_assistant_roundtrips(content in ".*") {
        let msg = Message::assistant(&content);
        let json = serde_json::to_string(&msg).unwrap();
        let msg2: Message = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&msg.content, &msg2.content);
    }
}

// ── Completion request round-trip ──────────────────────────────────────────

proptest! {
    #[test]
    fn completion_request_simple_roundtrips(content in ".{1,200}") {
        let req = CompletionRequest::simple(&content);
        let json = serde_json::to_string(&req).unwrap();
        let req2: CompletionRequest = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(req.messages.len(), req2.messages.len());
        prop_assert_eq!(&req.messages[0].content, &req2.messages[0].content);
        prop_assert_eq!(req.stream, req2.stream);
    }

    #[test]
    fn completion_request_builder_roundtrips(
        sys in ".{1,100}",
        user_msg in ".{1,200}",
        max_tok in 1..8192u32,
        temp in 0.0f32..2.0,
    ) {
        let req = CompletionRequest::builder(vec![
            Message::system(&sys),
            Message::user(&user_msg),
        ])
        .max_tokens(max_tok)
        .temperature(temp)
        .build();
        let json = serde_json::to_string(&req).unwrap();
        let req2: CompletionRequest = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(req.messages.len(), req2.messages.len());
        prop_assert_eq!(req.max_tokens, req2.max_tokens);
    }
}

// ── completion response round-trip ─────────────────────────────────────────

proptest! {
    #[test]
    fn completion_response_roundtrips(
        text in ".{0,200}",
        stop in arb_stop_reason(),
        usage in arb_token_usage(),
        model in "[a-z0-9-]{1,30}",
        latency in 0..10_000u64,
    ) {
        let resp = CompletionResponse::new(
            Message::assistant(&text),
            stop,
            usage,
            &model,
            latency,
        );
        let json = serde_json::to_string(&resp).unwrap();
        let resp2: CompletionResponse = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(resp.text(), resp2.text());
        prop_assert_eq!(&resp.model, &resp2.model);
        prop_assert_eq!(resp.latency_ms, resp2.latency_ms);
    }
}

// ── StreamChunk round-trip ─────────────────────────────────────────────────

proptest! {
    #[test]
    fn stream_chunk_delta_roundtrips(delta in ".{0,200}") {
        let chunk = StreamChunk::delta(&delta);
        let json = serde_json::to_string(&chunk).unwrap();
        let chunk2: StreamChunk = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&chunk.delta, &chunk2.delta);
        prop_assert!(!chunk2.done);
    }

    #[test]
    fn stream_chunk_done_roundtrips(stop in arb_stop_reason()) {
        let chunk = StreamChunk::done(stop);
        let json = serde_json::to_string(&chunk).unwrap();
        let chunk2: StreamChunk = serde_json::from_str(&json).unwrap();
        prop_assert!(chunk2.done);
        prop_assert!(chunk2.stop_reason.is_some());
    }
}

// ── ToolSchema round-trip ──────────────────────────────────────────────────

proptest! {
    #[test]
    fn tool_schema_roundtrips(
        name in "[a-z_]{1,30}",
        desc in ".{1,100}",
    ) {
        let schema = ToolSchema::new(
            &name,
            &desc,
            serde_json::json!({
                "type": "object",
                "properties": { "x": { "type": "string" } }
            }),
        );
        let json = serde_json::to_string(&schema).unwrap();
        let schema2: ToolSchema = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&schema.name, &schema2.name);
        prop_assert_eq!(&schema.description, &schema2.description);
    }
}

// ── ToolCall / ToolResult round-trip ───────────────────────────────────────

proptest! {
    #[test]
    fn tool_call_roundtrips(call in arb_tool_call()) {
        let json = serde_json::to_string(&call).unwrap();
        let call2: ToolCall = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&call.id, &call2.id);
        prop_assert_eq!(&call.name, &call2.name);
    }

    #[test]
    fn tool_result_roundtrips(result in arb_tool_result()) {
        let json = serde_json::to_string(&result).unwrap();
        let result2: ToolResult = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&result.call_id, &result2.call_id);
    }
}

// ── TokenUsage round-trip ──────────────────────────────────────────────────

proptest! {
    #[test]
    fn token_usage_roundtrips(usage in arb_token_usage()) {
        let json = serde_json::to_string(&usage).unwrap();
        let usage2: TokenUsage = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(usage.prompt_tokens, usage2.prompt_tokens);
        prop_assert_eq!(usage.completion_tokens, usage2.completion_tokens);
        prop_assert_eq!(usage.total_tokens, usage2.total_tokens);
    }
}

// ── SessionId / AgentId round-trip ─────────────────────────────────────────

proptest! {
    #[test]
    fn session_id_roundtrips(s in ".{1,100}") {
        let id = SessionId::new(&s);
        let json = serde_json::to_string(&id).unwrap();
        let id2: SessionId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(id.as_str(), id2.as_str());
    }

    #[test]
    fn agent_id_roundtrips(s in ".{1,100}") {
        let id = AgentId::new(&s);
        let json = serde_json::to_string(&id).unwrap();
        let id2: AgentId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(id.as_str(), id2.as_str());
    }
}

// ── StreamEvent round-trip ─────────────────────────────────────────────────

proptest! {
    #[test]
    fn stream_event_token_delta_roundtrips(delta in ".{0,200}") {
        let evt = StreamEvent::TokenDelta { delta: delta.clone() };
        let json = serde_json::to_string(&evt).unwrap();
        let evt2: StreamEvent = serde_json::from_str(&json).unwrap();
        if let StreamEvent::TokenDelta { delta: d2 } = evt2 {
            prop_assert_eq!(&delta, &d2);
        } else {
            prop_assert!(false, "Expected TokenDelta variant");
        }
    }

    #[test]
    fn stream_event_error_roundtrips(msg in ".{1,200}") {
        let evt = StreamEvent::Error { message: msg.clone() };
        let json = serde_json::to_string(&evt).unwrap();
        let evt2: StreamEvent = serde_json::from_str(&json).unwrap();
        if let StreamEvent::Error { message: m2 } = evt2 {
            prop_assert_eq!(&msg, &m2);
        } else {
            prop_assert!(false, "Expected Error variant");
        }
    }
}

// ── AgentState round-trip ──────────────────────────────────────────────────

proptest! {
    #[test]
    fn agent_state_failed_roundtrips(msg in ".{1,100}") {
        let state = AgentState::Failed(msg.clone());
        let json = serde_json::to_string(&state).unwrap();
        let state2: AgentState = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(state, state2);
    }
}
