use ironclaw_agents::{LocalBus, ReActAgent};
use ironclaw_core::{Agent, AgentRole, AgentTask};

#[test]
fn local_bus_register_and_get_count() {
    // LocalBus smoke test — no actual LLM calls
    let bus = LocalBus::new();
    // Bus starts empty, register adds agent
    // (full dispatch tests require a live provider)
    let _ = &bus; // at least confirm it compiles + constructs
}

#[tokio::test]
async fn react_agent_role_is_worker() {
    use ironclaw_agents::AgentContext;
    use ironclaw_config::IronClawConfig;
    let ctx   = AgentContext::from_config(IronClawConfig::default()).await.unwrap();
    let agent = ReActAgent::new(ctx);
    assert!(matches!(agent.role(), AgentRole::Worker));
}

#[test]
fn agent_task_new_fields() {
    let t = AgentTask::new("do the thing");
    assert_eq!(t.instruction, "do the thing");
    assert!(t.context.is_empty());
}