//! Provider registry benchmarks — lookup latency, registration, iteration.

use std::sync::Arc;

use async_trait::async_trait;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ironclaw_core::{
    BoxStream, CompletionRequest, CompletionResponse, Message, Provider, ProviderError, StopReason,
    StreamChunk, TokenUsage,
};
use ironclaw_providers::ProviderRegistry;

/// Minimal stub provider for benchmarking registry operations.
struct StubProvider {
    label: &'static str,
}

#[async_trait]
impl Provider for StubProvider {
    fn name(&self) -> &'static str {
        self.label
    }

    async fn complete(&self, _req: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        Ok(CompletionResponse::new(
            Message::assistant("ok"),
            StopReason::EndTurn,
            TokenUsage::default(),
            self.label,
            0,
        ))
    }

    async fn stream(
        &self,
        _req: CompletionRequest,
    ) -> Result<BoxStream<StreamChunk>, ProviderError> {
        Ok(Box::pin(futures::stream::empty()))
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}

fn bench_registry_get(c: &mut Criterion) {
    let mut reg = ProviderRegistry::new();
    reg.register(Arc::new(StubProvider { label: "ollama" }));
    reg.register(Arc::new(StubProvider { label: "anthropic" }));
    reg.register(Arc::new(StubProvider { label: "openai" }));
    reg.register(Arc::new(StubProvider { label: "groq" }));

    c.bench_function("ProviderRegistry::get (hit)", |b| {
        b.iter(|| reg.get(black_box("anthropic")));
    });

    c.bench_function("ProviderRegistry::get (miss)", |b| {
        b.iter(|| reg.get(black_box("nonexistent")));
    });
}

fn bench_registry_register(c: &mut Criterion) {
    c.bench_function("ProviderRegistry::register", |b| {
        b.iter(|| {
            let mut reg = ProviderRegistry::new();
            reg.register(Arc::new(StubProvider { label: "bench" }));
        });
    });
}

criterion_group!(benches, bench_registry_get, bench_registry_register,);
criterion_main!(benches);
