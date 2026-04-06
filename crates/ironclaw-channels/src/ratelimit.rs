//! Token-bucket rate limiter keyed by `(channel, user)`.
//!
//! Each user+channel combination gets an independent token bucket.
//! Tokens refill at a configurable rate. When the bucket is empty
//! the request is rejected with a human-readable error.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tracing::warn;

/// Configuration for the per-user token bucket.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum burst size (tokens in a full bucket).
    pub capacity: u32,
    /// How many tokens are added per refill interval.
    pub refill_tokens: u32,
    /// Duration between refills.
    pub refill_interval: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            capacity: 20,
            refill_tokens: 1,
            refill_interval: Duration::from_secs(3),
        }
    }
}

/// Internal bucket state for a single user+channel pair.
struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

/// A concurrent, shared rate limiter. Clone is cheap (Arc inside).
#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Arc<DashMap<String, Bucket>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(DashMap::new()),
        }
    }

    /// Build a composite key from the channel name and user identifier.
    fn key(channel: &str, user: &str) -> String {
        format!("{channel}:{user}")
    }

    /// Try to consume one token for the given channel+user.
    ///
    /// Returns `Ok(())` if a token was consumed, or `Err(wait)` with the
    /// estimated [`Duration`] until the next token is available.
    pub fn try_acquire(&self, channel: &str, user: &str) -> Result<(), Duration> {
        let key = Self::key(channel, user);
        let now = Instant::now();

        let mut entry = self.buckets.entry(key).or_insert_with(|| Bucket {
            tokens: self.config.capacity as f64,
            last_refill: now,
        });

        let bucket = entry.value_mut();

        // Refill tokens based on elapsed time
        let elapsed = now.duration_since(bucket.last_refill);
        let intervals = elapsed.as_secs_f64() / self.config.refill_interval.as_secs_f64();
        let refilled = intervals * self.config.refill_tokens as f64;
        bucket.tokens = (bucket.tokens + refilled).min(self.config.capacity as f64);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            // Estimate time until next token
            let deficit = 1.0 - bucket.tokens;
            let intervals_needed = deficit / self.config.refill_tokens as f64;
            let wait = Duration::from_secs_f64(
                intervals_needed * self.config.refill_interval.as_secs_f64(),
            );
            warn!(
                channel = %channel,
                user = %user,
                retry_after_ms = wait.as_millis() as u64,
                "Rate limited"
            );
            Err(wait)
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_capacity() {
        let limiter = RateLimiter::new(RateLimitConfig {
            capacity: 3,
            refill_tokens: 1,
            refill_interval: Duration::from_secs(60),
        });

        assert!(limiter.try_acquire("test", "user1").is_ok());
        assert!(limiter.try_acquire("test", "user1").is_ok());
        assert!(limiter.try_acquire("test", "user1").is_ok());
        // 4th call should be rejected
        assert!(limiter.try_acquire("test", "user1").is_err());
    }

    #[test]
    fn separate_users_have_separate_buckets() {
        let limiter = RateLimiter::new(RateLimitConfig {
            capacity: 1,
            refill_tokens: 1,
            refill_interval: Duration::from_secs(60),
        });

        assert!(limiter.try_acquire("test", "alice").is_ok());
        assert!(limiter.try_acquire("test", "bob").is_ok());
        // Both exhausted now
        assert!(limiter.try_acquire("test", "alice").is_err());
        assert!(limiter.try_acquire("test", "bob").is_err());
    }

    #[test]
    fn separate_channels_have_separate_buckets() {
        let limiter = RateLimiter::new(RateLimitConfig {
            capacity: 1,
            refill_tokens: 1,
            refill_interval: Duration::from_secs(60),
        });

        assert!(limiter.try_acquire("rest", "user1").is_ok());
        assert!(limiter.try_acquire("telegram", "user1").is_ok());
        assert!(limiter.try_acquire("rest", "user1").is_err());
    }

    #[test]
    fn returns_retry_duration_on_exhaustion() {
        let limiter = RateLimiter::new(RateLimitConfig {
            capacity: 1,
            refill_tokens: 1,
            refill_interval: Duration::from_secs(10),
        });

        assert!(limiter.try_acquire("ch", "u").is_ok());
        let err = limiter.try_acquire("ch", "u").unwrap_err();
        // Should be close to 10 seconds
        assert!(err.as_secs() <= 10);
        assert!(err.as_millis() > 0);
    }

    #[test]
    fn default_config_is_reasonable() {
        let cfg = RateLimitConfig::default();
        assert_eq!(cfg.capacity, 20);
        assert_eq!(cfg.refill_tokens, 1);
        assert_eq!(cfg.refill_interval, Duration::from_secs(3));
    }
}
