//! Circuit breaker wrapper for providers.
//!
//! Tracks consecutive failures and opens the circuit after a configurable
//! threshold, rejecting requests immediately until a recovery timeout elapses.
//!
//! States: **Closed** → (failures ≥ threshold) → **Open** → (timeout) → **HalfOpen**
//!   → (success) → **Closed**  |  (failure) → **Open**

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use ironclaw_core::*;
use tracing::{debug, info, warn};

/// Circuit breaker configuration.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Seconds to wait in Open state before allowing a probe request.
    pub recovery_timeout_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout_secs: 30,
        }
    }
}

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — requests pass through.
    Closed,
    /// Too many failures — requests are rejected immediately.
    Open,
    /// Recovery probe — one request allowed to test if the provider recovered.
    HalfOpen,
}

/// Wraps a [`Provider`] with circuit breaker logic.
pub struct CircuitBreakerProvider {
    inner: Arc<dyn Provider>,
    config: CircuitBreakerConfig,
    /// Consecutive failure count.
    failures: AtomicU32,
    /// Unix timestamp (seconds) when the circuit was opened.
    opened_at: AtomicU64,
}

impl std::fmt::Debug for CircuitBreakerProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitBreakerProvider")
            .field("inner", &self.inner.name())
            .field("state", &self.state())
            .field("failures", &self.failures.load(Ordering::Relaxed))
            .finish()
    }
}

impl CircuitBreakerProvider {
    /// Wrap a provider with circuit breaker protection.
    pub fn new(inner: Arc<dyn Provider>, config: CircuitBreakerConfig) -> Self {
        Self {
            inner,
            config,
            failures: AtomicU32::new(0),
            opened_at: AtomicU64::new(0),
        }
    }

    /// Current circuit state.
    pub fn state(&self) -> CircuitState {
        let failures = self.failures.load(Ordering::Relaxed);
        if failures < self.config.failure_threshold {
            return CircuitState::Closed;
        }
        let opened = self.opened_at.load(Ordering::Relaxed);
        let now = now_secs();
        if now.saturating_sub(opened) >= self.config.recovery_timeout_secs {
            CircuitState::HalfOpen
        } else {
            CircuitState::Open
        }
    }

    /// Record a success — reset failures and close the circuit.
    fn record_success(&self) {
        let prev = self.failures.swap(0, Ordering::Relaxed);
        if prev >= self.config.failure_threshold {
            info!(
                provider = self.inner.name(),
                "Circuit breaker closed (recovered)"
            );
        }
    }

    /// Record a failure — increment counter and potentially open the circuit.
    fn record_failure(&self) {
        let prev = self.failures.fetch_add(1, Ordering::Relaxed);
        let new = prev + 1;
        if new == self.config.failure_threshold {
            self.opened_at.store(now_secs(), Ordering::Relaxed);
            warn!(
                provider = self.inner.name(),
                failures = new,
                timeout_secs = self.config.recovery_timeout_secs,
                "Circuit breaker opened"
            );
        }
    }

    /// Check if the circuit allows a request. Returns an error if open.
    fn check_circuit(&self) -> Result<(), ProviderError> {
        match self.state() {
            CircuitState::Closed => Ok(()),
            CircuitState::HalfOpen => {
                debug!(
                    provider = self.inner.name(),
                    "Circuit half-open, allowing probe request"
                );
                Ok(())
            }
            CircuitState::Open => Err(ProviderError::Other(anyhow::anyhow!(
                "Circuit breaker open for provider '{}' — rejecting request",
                self.inner.name()
            ))),
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[async_trait]
impl Provider for CircuitBreakerProvider {
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
        self.check_circuit()?;
        match self.inner.complete(req).await {
            Ok(resp) => {
                self.record_success();
                Ok(resp)
            }
            Err(e) => {
                self.record_failure();
                Err(e)
            }
        }
    }

    async fn stream(
        &self,
        req: CompletionRequest,
    ) -> Result<BoxStream<StreamChunk>, ProviderError> {
        self.check_circuit()?;
        match self.inner.stream(req).await {
            Ok(stream) => {
                self.record_success();
                Ok(stream)
            }
            Err(e) => {
                self.record_failure();
                Err(e)
            }
        }
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        // Health checks bypass the circuit breaker — the registry uses them
        // to determine if a provider is available.
        self.inner.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    struct AlwaysFailProvider;

    #[async_trait]
    impl Provider for AlwaysFailProvider {
        fn name(&self) -> &'static str {
            "always-fail"
        }
        async fn complete(
            &self,
            _req: CompletionRequest,
        ) -> Result<CompletionResponse, ProviderError> {
            Err(ProviderError::Other(anyhow::anyhow!("provider error")))
        }
        async fn stream(
            &self,
            _req: CompletionRequest,
        ) -> Result<BoxStream<StreamChunk>, ProviderError> {
            Err(ProviderError::Other(anyhow::anyhow!("provider error")))
        }
        async fn health_check(&self) -> Result<(), ProviderError> {
            Ok(())
        }
    }

    struct CountingProvider {
        calls: AtomicU32,
    }

    impl CountingProvider {
        fn new() -> Self {
            Self {
                calls: AtomicU32::new(0),
            }
        }
    }

    #[async_trait]
    impl Provider for CountingProvider {
        fn name(&self) -> &'static str {
            "counting"
        }
        async fn complete(
            &self,
            _req: CompletionRequest,
        ) -> Result<CompletionResponse, ProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
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
            Ok(Box::pin(futures::stream::empty()))
        }
        async fn health_check(&self) -> Result<(), ProviderError> {
            Ok(())
        }
    }

    fn test_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout_secs: 1,
        }
    }

    fn test_request() -> CompletionRequest {
        CompletionRequest::simple("test")
    }

    #[test]
    fn starts_closed() {
        let cb = CircuitBreakerProvider::new(Arc::new(AlwaysFailProvider), test_config());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn opens_after_threshold_failures() {
        let cb = CircuitBreakerProvider::new(Arc::new(AlwaysFailProvider), test_config());
        for _ in 0..3 {
            let _ = cb.complete(test_request()).await;
        }
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn rejects_when_open() {
        let cb = CircuitBreakerProvider::new(Arc::new(AlwaysFailProvider), test_config());
        for _ in 0..3 {
            let _ = cb.complete(test_request()).await;
        }
        let result = cb.complete(test_request()).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Circuit breaker open"));
    }

    #[tokio::test]
    async fn recovers_after_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout_secs: 0, // instant recovery for test
        };
        let inner = Arc::new(CountingProvider::new());
        let cb = CircuitBreakerProvider::new(inner.clone(), config);

        // Trigger failures manually
        cb.record_failure();
        cb.record_failure();

        // With recovery_timeout_secs = 0, it's immediately HalfOpen
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Successful request closes the circuit
        let _ = cb.complete(test_request()).await;
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn success_resets_failure_count() {
        let inner = Arc::new(CountingProvider::new());
        let cb = CircuitBreakerProvider::new(inner, test_config());
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed); // still below threshold
        let _ = cb.complete(test_request()).await; // success
        assert_eq!(cb.failures.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn health_check_bypasses_circuit() {
        let cb = CircuitBreakerProvider::new(Arc::new(AlwaysFailProvider), test_config());
        for _ in 0..5 {
            let _ = cb.complete(test_request()).await;
        }
        assert_eq!(cb.state(), CircuitState::Open);
        // health_check should still pass through
        assert!(cb.health_check().await.is_ok());
    }

    #[test]
    fn name_delegates() {
        let cb = CircuitBreakerProvider::new(Arc::new(AlwaysFailProvider), test_config());
        assert_eq!(cb.name(), "always-fail");
    }
}
