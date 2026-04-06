//! Memory benchmarks — push/history throughput for InMemoryStore.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ironclaw_core::{MemoryStore, Message};
use ironclaw_memory::InMemoryStore;

fn bench_push(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    c.bench_function("InMemoryStore::push", |b| {
        let store = InMemoryStore::new(1000);
        b.iter(|| {
            rt.block_on(async {
                store
                    .push(black_box(&"session-1".into()), Message::user("hello world"))
                    .await
                    .unwrap();
            });
        });
    });
}

fn bench_history(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // Pre-fill a session with 100 messages
    let store = InMemoryStore::new(1000);
    rt.block_on(async {
        for i in 0..100 {
            store
                .push(
                    &"session-bench".into(),
                    Message::user(format!("message {i}")),
                )
                .await
                .unwrap();
        }
    });

    c.bench_function("InMemoryStore::history(50)", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = store
                    .history(black_box(&"session-bench".into()), 50)
                    .await
                    .unwrap();
            });
        });
    });
}

fn bench_push_at_capacity(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // Pre-fill to capacity (100 messages, max_history = 100)
    let store = InMemoryStore::new(100);
    rt.block_on(async {
        for i in 0..100 {
            store
                .push(&"session-cap".into(), Message::user(format!("message {i}")))
                .await
                .unwrap();
        }
    });

    c.bench_function("InMemoryStore::push (at capacity)", |b| {
        b.iter(|| {
            rt.block_on(async {
                store
                    .push(
                        black_box(&"session-cap".into()),
                        Message::user("overflow message"),
                    )
                    .await
                    .unwrap();
            });
        });
    });
}

criterion_group!(benches, bench_push, bench_history, bench_push_at_capacity,);
criterion_main!(benches);
