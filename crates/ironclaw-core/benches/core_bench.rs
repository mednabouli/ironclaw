//! Core benchmarks — serialization, message construction, token usage arithmetic,
//! stream chunk serde, tool schema serde.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ironclaw_core::{
    CompletionRequest, CompletionResponse, Message, StopReason, StreamChunk, StreamEvent,
    TokenUsage, ToolCall, ToolCallDelta, ToolSchema,
};

fn bench_message_construction(c: &mut Criterion) {
    c.bench_function("Message::user", |b| {
        b.iter(|| Message::user(black_box("Hello, how are you?")));
    });

    c.bench_function("Message::system", |b| {
        b.iter(|| Message::system(black_box("You are a helpful assistant.")));
    });
}

fn bench_completion_request_simple(c: &mut Criterion) {
    c.bench_function("CompletionRequest::simple", |b| {
        b.iter(|| CompletionRequest::simple(black_box("What is 2+2?")));
    });
}

fn bench_completion_request_serde(c: &mut Criterion) {
    let req = CompletionRequest::builder(vec![
        Message::system("You are a helpful assistant."),
        Message::user("Explain Rust ownership in one paragraph."),
    ])
    .max_tokens(500)
    .temperature(0.7)
    .build();

    c.bench_function("CompletionRequest serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&req)).unwrap());
    });

    let json = serde_json::to_string(&req).unwrap();
    c.bench_function("CompletionRequest deserialize", |b| {
        b.iter(|| serde_json::from_str::<CompletionRequest>(black_box(&json)).unwrap());
    });
}

fn bench_completion_response_serde(c: &mut Criterion) {
    let resp = CompletionResponse::new(
        Message::assistant("Rust ownership is a system of rules..."),
        StopReason::EndTurn,
        TokenUsage::new(25, 50, 75),
        "test-model",
        42,
    );

    c.bench_function("CompletionResponse serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&resp)).unwrap());
    });

    let json = serde_json::to_string(&resp).unwrap();
    c.bench_function("CompletionResponse deserialize", |b| {
        b.iter(|| serde_json::from_str::<CompletionResponse>(black_box(&json)).unwrap());
    });
}

fn bench_token_usage_default(c: &mut Criterion) {
    c.bench_function("TokenUsage::default", |b| {
        b.iter(TokenUsage::default);
    });
}

fn bench_stream_chunk_serde(c: &mut Criterion) {
    let chunk = StreamChunk::new(
        "Hello world token",
        false,
        vec![ToolCallDelta::first(0, "tc-1", "search", r#"{"q":"rust"#)],
        None,
    );

    c.bench_function("StreamChunk serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&chunk)).unwrap());
    });

    let json = serde_json::to_string(&chunk).unwrap();
    c.bench_function("StreamChunk deserialize", |b| {
        b.iter(|| serde_json::from_str::<StreamChunk>(black_box(&json)).unwrap());
    });
}

fn bench_stream_event_serde(c: &mut Criterion) {
    let events = vec![
        StreamEvent::TokenDelta {
            delta: "token".into(),
        },
        StreamEvent::ToolCallStart {
            id: "call-1".into(),
            name: "shell".into(),
            arguments: serde_json::json!({"command": "ls -la"}),
        },
        StreamEvent::Done {
            usage: Some(TokenUsage::new(100, 200, 300)),
        },
    ];
    let json = serde_json::to_string(&events).unwrap();

    c.bench_function("StreamEvent[] serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&events)).unwrap());
    });

    c.bench_function("StreamEvent[] deserialize", |b| {
        b.iter(|| serde_json::from_str::<Vec<StreamEvent>>(black_box(&json)).unwrap());
    });
}

fn bench_tool_schema_serde(c: &mut Criterion) {
    let schema = ToolSchema::new(
        "file_search",
        "Search files by glob pattern",
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern" },
                "max_results": { "type": "integer", "default": 10 }
            },
            "required": ["pattern"]
        }),
    );

    c.bench_function("ToolSchema serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&schema)).unwrap());
    });

    let json = serde_json::to_string(&schema).unwrap();
    c.bench_function("ToolSchema deserialize", |b| {
        b.iter(|| serde_json::from_str::<ToolSchema>(black_box(&json)).unwrap());
    });
}

fn bench_tool_call_serde(c: &mut Criterion) {
    let call = ToolCall::new(
        "call-42",
        "web_search",
        serde_json::json!({"query": "rust async trait", "limit": 5}),
    );

    c.bench_function("ToolCall serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&call)).unwrap());
    });

    let json = serde_json::to_string(&call).unwrap();
    c.bench_function("ToolCall deserialize", |b| {
        b.iter(|| serde_json::from_str::<ToolCall>(black_box(&json)).unwrap());
    });
}

criterion_group!(
    benches,
    bench_message_construction,
    bench_completion_request_simple,
    bench_completion_request_serde,
    bench_completion_response_serde,
    bench_token_usage_default,
    bench_stream_chunk_serde,
    bench_stream_event_serde,
    bench_tool_schema_serde,
    bench_tool_call_serde,
);
criterion_main!(benches);
