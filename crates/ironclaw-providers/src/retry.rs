use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ironclaw_core::*;
use tracing::{debug, warn};

/// Configuration for retry behaviour.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (0 = no retries).
    pub max_retries: u32,
    /// Base delay between retries in milliseconds. Doubled on each attempt.
    pub base_delay_ms: u64,
    /// Maximum delay cap in milliseconds.
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 500,
            max_delay_ms: 30_000,
        }
    }
}

/// Wraps any `Provider` with automatic retry + exponential backoff on transient errors.
///
/// Retries `complete()` and `stream()` (initial connection only) on transient failures.
/// Does NOT retry `health_check()`.
pub struct RetryProvider {
    inner: Arc<dyn Provider>,
    config: RetryConfig,
}

impl std::fmt::Debug for RetryProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetryProvider")
            .field("inner", &self.inner.name())
            .field("max_retries", &self.config.max_retries)
            .finish()
    }
}

impl RetryProvider {
    /// Wrap a provider with retry logic.
    pub fn new(inner: Arc<dyn Provider>, config: RetryConfig) -> Self {
        Self { inner, config }
    }

    /// Compute delay for attempt `n` (0-indexed) with jitter.
    fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self
            .config
            .base_delay_ms
            .saturating_mul(1u64.wrapping_shl(attempt));
        let capped = base.min(self.config.max_delay_ms);
        // Simple jitter: 75%–100% of the computed delay
        let jitter_factor = 0.75 + (pseudo_random_fraction(attempt) * 0.25);
        Duration::from_millis((capped as f64 * jitter_factor) as u64)
    }
}

/// Simple deterministic pseudo-random fraction [0.0, 1.0) based on attempt number
/// and current time. Avoids pulling in a rand dependency.
fn pseudo_random_fraction(attempt: u32) -> f64 {
    // Mix attempt with nanosecond timestamp for cheap jitter
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let mixed = nanos
        .wrapping_mul(2654435761)
        .wrapping_add(attempt.wrapping_mul(0x9E3779B9));
    (mixed as f64) / (u32::MAX as f64)
}

/// Check if an error indicates a transient/retriable failure.
fn is_transient(err: &ProviderError) -> bool {
    match err {
        // Rate limiting is always transient
        ProviderError::RateLimit { .. } => true,
        // Stream drops are transient
        ProviderError::StreamTerminated => true,
        // Auth failures are NEVER transient
        ProviderError::Auth(_) => false,
        // Model not found is NEVER transient
        ProviderError::ModelNotFound(_) => false,
        // Invalid responses are not transient (same input → same parse failure)
        ProviderError::InvalidResponse(_) => false,
        // Request errors: check for transient HTTP status codes or network issues
        ProviderError::Request(msg) => {
            let msg = msg.to_lowercase();
            msg.contains("429")
                || msg.contains("rate limit")
                || msg.contains("503")
                || msg.contains("502")
                || msg.contains("504")
                || msg.contains("connection")
                || msg.contains("timeout")
                || msg.contains("timed out")
                || msg.contains("reset by peer")
                || msg.contains("broken pipe")
                || msg.contains("dns")
        }
        // Other errors: fall back to string matching for transient indicators
        ProviderError::Other(e) => {
            let msg = format!("{e:#}").to_lowercase();
            // 5xx status codes and network errors are transient
            (msg.contains("500")
                || msg.contains("502")
                || msg.contains("503")
                || msg.contains("504")
                || msg.contains("connection")
                || msg.contains("timeout")
                || msg.contains("timed out")
                || msg.contains("reset by peer")
                || msg.contains("broken pipe")
                || msg.contains("dns"))
                // But 4xx client errors are NOT transient
                && !msg.contains("401")
                && !msg.contains("403")
                && !msg.contains("404")
                && !msg.contains("422")
        }
        // Future variants: assume not transient (safe default)
        _ => false,
    }
}

#[async_trait]
impl Provider for RetryProvider {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn supports_streaming(&self) -> bool {
        self.inner.supports_streaming()
    }

    fn supports_tools(&self) -> bool {
        self.inner.supports_tools()
    }

    fn supports_vision(&self) -> bool {
        self.inner.supports_vision()
    }

    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        let mut last_err = None;
        for attempt in 0..=self.config.max_retries {
            match self.inner.complete(req.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    if attempt < self.config.max_retries && is_transient(&e) {
                        let delay = self.delay_for_attempt(attempt);
                        warn!(
                            provider = self.inner.name(),
                            attempt = attempt + 1,
                            max_retries = self.config.max_retries,
                            delay_ms = delay.as_millis() as u64,
                            error = %e,
                            "Transient error, retrying"
                        );
                        tokio::time::sleep(delay).await;
                        last_err = Some(e);
                    } else {
                        debug!(
                            provider = self.inner.name(),
                            attempt = attempt + 1,
                            error = %e,
                            "Non-transient error or max retries reached"
                        );
                        return Err(e);
                    }
                }
            }
        }
        Err(last_err.unwrap_or_else(|| {
            ProviderError::Other(anyhow::anyhow!("Retry exhausted with no error captured"))
        }))
    }

    async fn stream(
        &self,
        req: CompletionRequest,
    ) -> Result<BoxStream<StreamChunk>, ProviderError> {
        // Retry the initial stream connection, not mid-stream errors
        let mut last_err = None;
        for attempt in 0..=self.config.max_retries {
            match self.inner.stream(req.clone()).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    if attempt < self.config.max_retries && is_transient(&e) {
                        let delay = self.delay_for_attempt(attempt);
                        warn!(
                            provider = self.inner.name(),
                            attempt = attempt + 1,
                            max_retries = self.config.max_retries,
                            delay_ms = delay.as_millis() as u64,
                            error = %e,
                            "Transient stream error, retrying"
                        );
                        tokio::time::sleep(delay).await;
                        last_err = Some(e);
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        Err(last_err.unwrap_or_else(|| {
            ProviderError::Other(anyhow::anyhow!("Retry exhausted with no error captured"))
        }))
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        // Health checks are not retried — the registry handles fallback
        self.inner.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A provider that fails N times then succeeds.
    struct FailNProvider {
        fail_count: AtomicU32,
        max_failures: u32,
        transient: bool,
    }

    impl FailNProvider {
        fn new(max_failures: u32, transient: bool) -> Self {
            Self {
                fail_count: AtomicU32::new(0),
                max_failures,
                transient,
            }
        }
    }

    #[async_trait]
    impl Provider for FailNProvider {
        fn name(&self) -> &'static str {
            "fail-n"
        }
        fn supports_vision(&self) -> bool {
            false
        }

        async fn complete(
            &self,
            _req: CompletionRequest,
        ) -> Result<CompletionResponse, ProviderError> {
            let n = self.fail_count.fetch_add(1, Ordering::SeqCst);
            if n < self.max_failures {
                if self.transient {
                    return Err(ProviderError::Request(
                        "HTTP 503 Service Unavailable".into(),
                    ));
                } else {
                    return Err(ProviderError::Auth("HTTP 401 Unauthorized".into()));
                }
            }
            Ok(CompletionResponse::new(
                Message::assistant("ok"),
                StopReason::EndTurn,
                TokenUsage::default(),
                "test",
                0,
            ))
        }

        async fn stream(
            &self,
            _req: CompletionRequest,
        ) -> Result<BoxStream<StreamChunk>, ProviderError> {
            let n = self.fail_count.fetch_add(1, Ordering::SeqCst);
            if n < self.max_failures {
                if self.transient {
                    return Err(ProviderError::Request("connection timeout".into()));
                } else {
                    return Err(ProviderError::Auth("HTTP 403 Forbidden".into()));
                }
            }
            Ok(Box::pin(futures::stream::empty()))
        }

        async fn health_check(&self) -> Result<(), ProviderError> {
            Ok(())
        }
    }

    fn test_config() -> RetryConfig {
        RetryConfig {
            max_retries: 3,
            base_delay_ms: 1, // 1ms for fast tests
            max_delay_ms: 10,
        }
    }

    fn test_request() -> CompletionRequest {
        CompletionRequest::builder(vec![Message::user("hi")])
            .max_tokens(100)
            .temperature(0.7)
            .build()
    }

    #[tokio::test]
    async fn retries_transient_complete_then_succeeds() {
        let inner = Arc::new(FailNProvider::new(2, true));
        let provider = RetryProvider::new(inner.clone(), test_config());
        let result = provider.complete(test_request()).await;
        assert!(result.is_ok(), "Should succeed after 2 transient failures");
        assert_eq!(
            inner.fail_count.load(Ordering::SeqCst),
            3,
            "Should have been called 3 times (2 fails + 1 success)"
        );
    }

    #[tokio::test]
    async fn does_not_retry_non_transient_complete() {
        let inner = Arc::new(FailNProvider::new(5, false));
        let provider = RetryProvider::new(inner.clone(), test_config());
        let result = provider.complete(test_request()).await;
        assert!(result.is_err(), "Should fail immediately on 401");
        assert_eq!(
            inner.fail_count.load(Ordering::SeqCst),
            1,
            "Should have been called only once"
        );
    }

    #[tokio::test]
    async fn retries_transient_stream_then_succeeds() {
        let inner = Arc::new(FailNProvider::new(1, true));
        let provider = RetryProvider::new(inner.clone(), test_config());
        let mut req = test_request();
        req.stream = true;
        let result = provider.stream(req).await;
        assert!(result.is_ok(), "Should succeed after 1 transient failure");
    }

    #[tokio::test]
    async fn exhausts_retries_and_returns_last_error() {
        let inner = Arc::new(FailNProvider::new(100, true));
        let provider = RetryProvider::new(inner.clone(), test_config());
        let result = provider.complete(test_request()).await;
        assert!(result.is_err(), "Should fail after exhausting retries");
        assert_eq!(
            inner.fail_count.load(Ordering::SeqCst),
            4,
            "Should have been called 4 times (1 initial + 3 retries)"
        );
    }

    #[tokio::test]
    async fn health_check_is_not_retried() {
        // Just verify it delegates directly
        let inner = Arc::new(FailNProvider::new(0, false));
        let provider = RetryProvider::new(inner, test_config());
        assert!(provider.health_check().await.is_ok());
    }

    #[test]
    fn delay_respects_max_cap() {
        let provider = RetryProvider::new(
            Arc::new(FailNProvider::new(0, false)),
            RetryConfig {
                max_retries: 10,
                base_delay_ms: 1000,
                max_delay_ms: 5000,
            },
        );
        // After several doublings, delay should be capped
        let d = provider.delay_for_attempt(10);
        assert!(
            d.as_millis() <= 5000,
            "Delay should be capped at max_delay_ms"
        );
    }

    #[test]
    fn is_transient_detects_retriable_errors() {
        assert!(is_transient(&ProviderError::Request(
            "HTTP 503 Service Unavailable".into()
        )));
        assert!(is_transient(&ProviderError::Request(
            "HTTP 429 Too Many Requests".into()
        )));
        assert!(is_transient(&ProviderError::Request(
            "rate limit exceeded".into()
        )));
        assert!(is_transient(&ProviderError::Request(
            "connection reset by peer".into()
        )));
        assert!(is_transient(&ProviderError::Request(
            "request timed out".into()
        )));
        assert!(!is_transient(&ProviderError::Auth(
            "HTTP 401 Unauthorized".into()
        )));
        assert!(!is_transient(&ProviderError::Other(anyhow::anyhow!(
            "HTTP 400 Bad Request"
        ))));
        assert!(!is_transient(&ProviderError::InvalidResponse(
            "invalid json".into()
        )));
    }

    #[test]
    fn provider_name_delegates() {
        let inner = Arc::new(FailNProvider::new(0, false));
        let provider = RetryProvider::new(inner, test_config());
        assert_eq!(provider.name(), "fail-n");
    }
}
