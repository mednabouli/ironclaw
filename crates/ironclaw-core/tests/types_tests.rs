use ironclaw_core::*;

#[test]
fn message_user_role() {
    assert_eq!(Message::user("hi").role, Role::User);
}
#[test]
fn message_system_role() {
    assert_eq!(Message::system("s").role, Role::System);
}
#[test]
fn message_assistant_role() {
    assert_eq!(Message::assistant("a").role, Role::Assistant);
}

#[test]
fn message_tool_result_has_call_id() {
    let m = Message::tool_result("call-99", serde_json::json!({"r":1}));
    assert_eq!(m.role, Role::Tool);
    assert_eq!(m.tool_result.unwrap().call_id, "call-99");
}

#[test]
fn message_roundtrips_json() {
    let m = Message::assistant("hello world");
    let j = serde_json::to_string(&m).unwrap();
    let m2: Message = serde_json::from_str(&j).unwrap();
    assert_eq!(m.content, m2.content);
}

#[test]
fn completion_request_simple_structure() {
    let r = CompletionRequest::simple("tell me a joke");
    assert_eq!(r.messages.len(), 1);
    assert_eq!(r.messages[0].role, Role::User);
    assert!(r.tools.is_empty());
    assert!(!r.stream);
}

#[test]
fn outbound_message_as_str() {
    let m = OutboundMessage::text("s1", "reply here");
    assert_eq!(m.as_str(), "reply here");
    assert_eq!(m.session_id, "s1");
}

#[test]
fn stop_reason_serde() {
    assert_eq!(
        serde_json::to_string(&StopReason::EndTurn).unwrap(),
        r#""end_turn""#
    );
    assert_eq!(
        serde_json::to_string(&StopReason::ToolUse).unwrap(),
        r#""tool_use""#
    );
}

#[test]
fn token_usage_default_zero() {
    assert_eq!(TokenUsage::default().total_tokens, 0);
}

#[test]
fn agent_task_new_empty_context() {
    let t = AgentTask::new("summarize");
    assert_eq!(t.instruction, "summarize");
    assert!(t.context.is_empty());
    assert!(t.tool_allowlist.is_none());
}

#[test]
fn inbound_message_cli_helper() {
    let m = InboundMessage::cli("hello");
    assert_eq!(m.content, "hello");
    assert!(matches!(m.channel, ChannelId::Cli));
}

#[test]
fn tool_call_delta_roundtrips_json() {
    let d = ToolCallDelta {
        index: 0,
        id: Some("call-1".into()),
        name: Some("shell".into()),
        arguments_delta: r#"{"cmd":"ls"#.into(),
    };
    let j = serde_json::to_string(&d).unwrap();
    let d2: ToolCallDelta = serde_json::from_str(&j).unwrap();
    assert_eq!(d2.index, 0);
    assert_eq!(d2.id.as_deref(), Some("call-1"));
    assert_eq!(d2.arguments_delta, r#"{"cmd":"ls"#);
}

#[test]
fn stream_chunk_defaults_empty_tool_calls() {
    let json = r#"{"delta":"hi","done":false}"#;
    let chunk: StreamChunk = serde_json::from_str(json).unwrap();
    assert!(chunk.tool_calls.is_empty());
    assert!(chunk.stop_reason.is_none());
}

#[test]
fn stream_chunk_with_tool_calls_roundtrips() {
    let chunk = StreamChunk {
        delta: String::new(),
        done: false,
        tool_calls: vec![ToolCallDelta {
            index: 0,
            id: Some("tc-1".into()),
            name: Some("search".into()),
            arguments_delta: "{}".into(),
        }],
        stop_reason: None,
    };
    let j = serde_json::to_string(&chunk).unwrap();
    let chunk2: StreamChunk = serde_json::from_str(&j).unwrap();
    assert_eq!(chunk2.tool_calls.len(), 1);
    assert_eq!(chunk2.tool_calls[0].name.as_deref(), Some("search"));
}

#[test]
fn stream_event_token_delta_serde() {
    let evt = StreamEvent::TokenDelta {
        delta: "hello".into(),
    };
    let j = serde_json::to_string(&evt).unwrap();
    assert!(j.contains(r#""type":"token_delta""#));
    assert!(j.contains(r#""delta":"hello""#));
}

#[test]
fn stream_event_tool_call_start_serde() {
    let evt = StreamEvent::ToolCallStart {
        id: "call-1".into(),
        name: "shell".into(),
        arguments: serde_json::json!({"command": "ls"}),
    };
    let j = serde_json::to_string(&evt).unwrap();
    assert!(j.contains(r#""type":"tool_call_start""#));
}

#[test]
fn stream_event_done_serde() {
    let evt = StreamEvent::Done {
        usage: Some(TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        }),
    };
    let j = serde_json::to_string(&evt).unwrap();
    assert!(j.contains(r#""type":"done""#));
    assert!(j.contains(r#""total_tokens":15"#));
}

#[test]
fn stream_event_error_serde() {
    let evt = StreamEvent::Error {
        message: "provider timeout".into(),
    };
    let j = serde_json::to_string(&evt).unwrap();
    assert!(j.contains(r#""type":"error""#));
    assert!(j.contains("provider timeout"));
}
