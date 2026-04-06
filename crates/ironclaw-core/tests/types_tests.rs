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
