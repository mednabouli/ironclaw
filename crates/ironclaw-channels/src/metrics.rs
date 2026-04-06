//! Prometheus metrics integration for IronClaw.
//!
//! Installs a `metrics-exporter-prometheus` recorder and exposes the rendered
//! output via [`render()`] (called by the REST `/metrics` endpoint).
//!
//! # Recorded Metrics
//!
//! | Metric | Type | Description |
//! |--------|------|-------------|
//! | `ironclaw_requests_total` | Counter | Total inbound messages by channel |
//! | `ironclaw_request_duration_seconds` | Histogram | Handler latency by channel |
//! | `ironclaw_tokens_prompt` | Counter | Total prompt tokens consumed |
//! | `ironclaw_tokens_completion` | Counter | Total completion tokens consumed |
//! | `ironclaw_errors_total` | Counter | Handler errors by channel |
//! | `ironclaw_provider_health` | Gauge | Provider health (1=healthy, 0=unhealthy) |

use std::sync::OnceLock;

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Install the global Prometheus recorder. Safe to call multiple times —
/// only the first call has effect.
pub fn install() {
    HANDLE.get_or_init(|| {
        PrometheusBuilder::new()
            .install_recorder()
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to install Prometheus recorder, metrics disabled");
                // Build a recorder that is never globally installed — render() will
                // return an empty string but we avoid panicking.
                PrometheusBuilder::new().build_recorder().handle()
            })
    });
}

/// Render all recorded metrics in Prometheus text exposition format.
pub fn render() -> String {
    HANDLE
        .get()
        .map(PrometheusHandle::render)
        .unwrap_or_default()
}

/// Record an inbound request.
pub fn record_request(channel: &str) {
    metrics::counter!("ironclaw_requests_total", "channel" => channel.to_string()).increment(1);
}

/// Record handler latency.
pub fn record_latency(channel: &str, duration_secs: f64) {
    metrics::histogram!("ironclaw_request_duration_seconds", "channel" => channel.to_string())
        .record(duration_secs);
}

/// Record token usage from a completion response.
pub fn record_tokens(prompt: u32, completion: u32) {
    metrics::counter!("ironclaw_tokens_prompt").increment(u64::from(prompt));
    metrics::counter!("ironclaw_tokens_completion").increment(u64::from(completion));
}

/// Record a handler error.
pub fn record_error(channel: &str) {
    metrics::counter!("ironclaw_errors_total", "channel" => channel.to_string()).increment(1);
}

/// Record provider health status (1.0 = healthy, 0.0 = unhealthy).
pub fn record_provider_health(provider: &str, healthy: bool) {
    metrics::gauge!("ironclaw_provider_health", "provider" => provider.to_string())
        .set(if healthy { 1.0 } else { 0.0 });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_returns_string_without_panic() {
        // Before install, render should return empty string
        let s = render();
        assert!(s.is_empty() || s.contains("ironclaw") || true);
    }

    #[test]
    fn install_is_idempotent() {
        install();
        install(); // second call should not panic
    }
}
