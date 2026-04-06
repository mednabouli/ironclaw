//! Core benchmarks — serialization, message construction, token usage arithmetic.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ironclaw_core::{CompletionRequest, CompletionResponse, Message, StopReason, TokenUsage};

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

criterion_group!(
    benches,
    bench_message_construction,
    bench_completion_request_simple,
    bench_completion_request_serde,
    bench_completion_response_serde,
    bench_token_usage_default,
);
criterion_main!(benches);
