//! OpenTelemetry initialization — export tracing spans over OTLP.
//!
//! Enabled via the `otel` feature flag. When active, all existing `tracing`
//! spans and events are bridged to the configured OTLP endpoint.
//!
//! # Environment Variables
//!
//! - `OTEL_EXPORTER_OTLP_ENDPOINT` — OTLP gRPC endpoint (default: `http://localhost:4317`)
//! - `OTEL_SERVICE_NAME` — reported service name (default: `ironclaw`)

use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime::Tokio, trace::TracerProvider, Resource};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing with an OpenTelemetry OTLP layer on top of the
/// standard `tracing-subscriber` formatter.
///
/// Returns a guard that shuts down the OTLP exporter on drop.
pub fn init_otel_tracing(log_level: &str) -> anyhow::Result<OtelGuard> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".into());

    let service_name = std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "ironclaw".into());

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&endpoint)
        .build()?;

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, Tokio)
        .with_resource(Resource::new(vec![KeyValue::new(
            "service.name",
            service_name,
        )]))
        .build();

    let tracer = provider.tracer("ironclaw");
    let otel_layer = OpenTelemetryLayer::new(tracer);

    let filter = EnvFilter::try_new(log_level).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().compact())
        .with(otel_layer)
        .init();

    tracing::info!(endpoint = %endpoint, "OpenTelemetry OTLP exporter initialized");

    Ok(OtelGuard { provider })
}

/// RAII guard — flushes and shuts down the OTLP tracer on drop.
pub struct OtelGuard {
    provider: TracerProvider,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Err(e) = self.provider.shutdown() {
            eprintln!("OpenTelemetry shutdown error: {e}");
        }
    }
}
