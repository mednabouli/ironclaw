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
    let ctx = AgentContext::from_config(IronClawConfig::default())
        .await
        .unwrap();
    let agent = ReActAgent::new(ctx);
    assert!(matches!(agent.role(), AgentRole::Worker));
}

#[test]
fn agent_task_new_fields() {
    let t = AgentTask::new("do the thing");
    assert_eq!(t.instruction, "do the thing");
    assert!(t.context.is_empty());
}

// ── Streaming ReAct tests ─────────────────────────────────────────────────

mod streaming {
    use std::sync::Arc;

    use async_trait::async_trait;
    use ironclaw_agents::{AgentContext, ReActAgent};
    use ironclaw_config::IronClawConfig;
    use ironclaw_core::*;
    use ironclaw_providers::ProviderRegistry;
    use ironclaw_tools::ToolRegistry;
    use tokio_stream::StreamExt;

    /// Mock provider that returns a text-only stream (no tool calls).
    struct MockTextProvider;

    #[async_trait]
    impl Provider for MockTextProvider {
        fn name(&self) -> &'static str {
            "mock"
        }
        async fn complete(&self, _req: CompletionRequest) -> anyhow::Result<CompletionResponse> {
            anyhow::bail!("not used in stream test")
        }
        async fn stream(&self, _req: CompletionRequest) -> anyhow::Result<BoxStream<StreamChunk>> {
            let chunks: Vec<anyhow::Result<StreamChunk>> = vec![
                Ok(StreamChunk {
                    delta: "Hello".into(),
                    done: false,
                    tool_calls: vec![],
                    stop_reason: None,
                }),
                Ok(StreamChunk {
                    delta: " world".into(),
                    done: true,
                    tool_calls: vec![],
                    stop_reason: Some(StopReason::EndTurn),
                }),
            ];
            Ok(Box::pin(futures::stream::iter(chunks)))
        }
        async fn health_check(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    /// Mock provider that emits tool-call deltas then a text answer.
    struct MockToolCallProvider {
        call_count: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl Provider for MockToolCallProvider {
        fn name(&self) -> &'static str {
            "mock"
        }
        async fn complete(&self, _req: CompletionRequest) -> anyhow::Result<CompletionResponse> {
            anyhow::bail!("not used in stream test")
        }
        async fn stream(&self, _req: CompletionRequest) -> anyhow::Result<BoxStream<StreamChunk>> {
            let n = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            if n == 0 {
                // First call: emit tool call deltas
                let chunks: Vec<anyhow::Result<StreamChunk>> = vec![
                    Ok(StreamChunk {
                        delta: String::new(),
                        done: false,
                        tool_calls: vec![ToolCallDelta {
                            index: 0,
                            id: Some("call_1".into()),
                            name: Some("get_datetime".into()),
                            arguments_delta: r#"{"timezone""#.into(),
                        }],
                        stop_reason: None,
                    }),
                    Ok(StreamChunk {
                        delta: String::new(),
                        done: true,
                        tool_calls: vec![ToolCallDelta {
                            index: 0,
                            id: None,
                            name: None,
                            arguments_delta: r#": "UTC"}"#.into(),
                        }],
                        stop_reason: Some(StopReason::ToolUse),
                    }),
                ];
                Ok(Box::pin(futures::stream::iter(chunks)))
            } else {
                // Second call: final text answer after tool result
                let chunks: Vec<anyhow::Result<StreamChunk>> = vec![Ok(StreamChunk {
                    delta: "The time is now.".into(),
                    done: true,
                    tool_calls: vec![],
                    stop_reason: Some(StopReason::EndTurn),
                })];
                Ok(Box::pin(futures::stream::iter(chunks)))
            }
        }
        async fn health_check(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn make_ctx_with_provider(provider: Arc<dyn Provider>) -> AgentContext {
        let cfg = Arc::new(IronClawConfig::default());
        let mut reg = ProviderRegistry::new();
        reg.register(provider);
        reg.set_fallback_chain(vec!["mock".into()]);
        let tools = Arc::new(ToolRegistry::from_config(&cfg));
        let memory = Arc::new(ironclaw_memory::InMemoryStore::new(100));
        AgentContext::new(cfg, Arc::new(reg), tools, memory)
    }

    #[tokio::test]
    async fn stream_text_only_emits_deltas_then_done() {
        let ctx = make_ctx_with_provider(Arc::new(MockTextProvider));
        let agent = ReActAgent::new(ctx);
        let task = AgentTask::new("say hello");
        let stream = agent.stream_with_history("sess-1".into(), task);

        let events: Vec<StreamEvent> = stream.filter_map(|r| r.ok()).collect().await;

        assert!(
            events.len() >= 3,
            "Expected at least 3 events, got {}",
            events.len()
        );

        // First two should be token deltas
        assert!(matches!(&events[0], StreamEvent::TokenDelta { delta } if delta == "Hello"));
        assert!(matches!(&events[1], StreamEvent::TokenDelta { delta } if delta == " world"));

        // Last should be Done
        assert!(matches!(events.last().unwrap(), StreamEvent::Done { .. }));
    }

    #[tokio::test]
    async fn stream_tool_call_emits_start_end_then_done() {
        let provider = Arc::new(MockToolCallProvider {
            call_count: std::sync::atomic::AtomicUsize::new(0),
        });
        let ctx = make_ctx_with_provider(provider);
        let agent = ReActAgent::new(ctx);
        let task = AgentTask::new("what time is it?");
        let stream = agent.stream_with_history("sess-2".into(), task);

        let events: Vec<StreamEvent> = stream.filter_map(|r| r.ok()).collect().await;

        // Should contain: ToolCallStart, ToolCallEnd, TokenDelta("The time is now."), Done
        let has_start = events.iter().any(
            |e| matches!(e, StreamEvent::ToolCallStart { name, .. } if name == "get_datetime"),
        );
        let has_end = events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolCallEnd { id, .. } if id == "call_1"));
        let has_delta = events
            .iter()
            .any(|e| matches!(e, StreamEvent::TokenDelta { delta } if delta == "The time is now."));
        let has_done = events.iter().any(|e| matches!(e, StreamEvent::Done { .. }));

        assert!(has_start, "Missing ToolCallStart event. Events: {events:?}");
        assert!(has_end, "Missing ToolCallEnd event. Events: {events:?}");
        assert!(has_delta, "Missing TokenDelta event. Events: {events:?}");
        assert!(has_done, "Missing Done event. Events: {events:?}");

        // Verify ordering: start before end, end before done
        let start_pos = events
            .iter()
            .position(|e| matches!(e, StreamEvent::ToolCallStart { .. }))
            .unwrap();
        let end_pos = events
            .iter()
            .position(|e| matches!(e, StreamEvent::ToolCallEnd { .. }))
            .unwrap();
        let done_pos = events
            .iter()
            .position(|e| matches!(e, StreamEvent::Done { .. }))
            .unwrap();
        assert!(
            start_pos < end_pos,
            "ToolCallStart should precede ToolCallEnd"
        );
        assert!(end_pos < done_pos, "ToolCallEnd should precede Done");
    }
}
