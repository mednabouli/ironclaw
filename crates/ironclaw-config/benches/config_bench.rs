//! Config benchmarks — parsing, ArcSwap load latency, hot-reload swap.

use std::io::Write;
use std::sync::Arc;

use arc_swap::ArcSwap;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ironclaw_config::IronClawConfig;

fn bench_config_default(c: &mut Criterion) {
    c.bench_function("IronClawConfig::default", |b| {
        b.iter(IronClawConfig::default);
    });
}

fn bench_config_parse_toml(c: &mut Criterion) {
    let dir = std::env::temp_dir().join("ironclaw_bench_config");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ironclaw.toml");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(
        f,
        r#"
[agent]
name = "bench-agent"
system_prompt = "You are a helpful assistant."
max_tokens = 2048
temperature = 0.7

[providers]
primary = "ollama"

[channels]
enabled = ["cli"]

[memory]
backend = "inmemory"
max_history = 100

[telemetry]
level = "info"
"#
    )
    .unwrap();

    c.bench_function("IronClawConfig::from_file", |b| {
        b.iter(|| IronClawConfig::from_file(black_box(&path)).unwrap());
    });

    std::fs::remove_dir_all(dir).ok();
}

fn bench_arcswap_load(c: &mut Criterion) {
    let store = Arc::new(ArcSwap::from_pointee(IronClawConfig::default()));

    c.bench_function("ArcSwap<IronClawConfig>::load", |b| {
        b.iter(|| {
            let guard = store.load();
            black_box(&guard.agent.name);
        });
    });
}

fn bench_arcswap_store(c: &mut Criterion) {
    let store = Arc::new(ArcSwap::from_pointee(IronClawConfig::default()));

    c.bench_function("ArcSwap<IronClawConfig>::store", |b| {
        b.iter(|| {
            store.store(Arc::new(IronClawConfig::default()));
        });
    });
}

fn bench_config_serde_roundtrip(c: &mut Criterion) {
    let cfg = IronClawConfig::default();
    let json = serde_json::to_string(&cfg).unwrap();

    c.bench_function("IronClawConfig serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&cfg)).unwrap());
    });

    c.bench_function("IronClawConfig deserialize", |b| {
        b.iter(|| serde_json::from_str::<IronClawConfig>(black_box(&json)).unwrap());
    });
}

criterion_group!(
    benches,
    bench_config_default,
    bench_config_parse_toml,
    bench_arcswap_load,
    bench_arcswap_store,
    bench_config_serde_roundtrip,
);
criterion_main!(benches);
