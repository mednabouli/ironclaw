//! Redis-backed session store for distributed deployments.
//!
//! Each session is stored as a Redis list under `{prefix}session:{session_id}`.
//! Messages are JSON-encoded and trimmed to `max_history` on every push.

use async_trait::async_trait;
use ironclaw_core::{MemoryError, MemoryStore, Message, SearchHit, SessionId};
use redis::AsyncCommands;
use tracing::warn;

/// Persistent Redis-backed session store.
///
/// Uses Redis lists for ordered message history with automatic trimming.
/// Supports listing sessions via a tracking set and full-text search
/// across all stored messages.
pub struct RedisStore {
    client: redis::Client,
    prefix: String,
    max_history: usize,
}

impl RedisStore {
    /// Connect to Redis at the given URL.
    ///
    /// The `prefix` is prepended to all keys (e.g. `"ironclaw:"`).
    pub async fn new(url: &str, prefix: &str, max_history: usize) -> anyhow::Result<Self> {
        let client = redis::Client::open(url)
            .map_err(|e| anyhow::anyhow!("Cannot connect to Redis: {e}"))?;

        // Verify the connection works
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| anyhow::anyhow!("Redis connection failed: {e}"))?;

        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("Redis PING failed: {e}"))?;

        Ok(Self {
            client,
            prefix: prefix.to_string(),
            max_history,
        })
    }

    /// Build the Redis key for a session's message list.
    fn session_key(&self, session: &SessionId) -> String {
        format!("{}session:{}", self.prefix, session)
    }

    /// Build the Redis key for the sessions tracking set.
    fn sessions_set_key(&self) -> String {
        format!("{}sessions", self.prefix)
    }

    /// Get a multiplexed async connection.
    async fn conn(&self) -> anyhow::Result<redis::aio::MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| anyhow::anyhow!("Redis connection error: {e}"))
    }
}

#[async_trait]
impl MemoryStore for RedisStore {
    async fn push(&self, session: &SessionId, msg: Message) -> Result<(), MemoryError> {
        let r: anyhow::Result<()> = (async {
            let mut conn = self.conn().await?;
            let key = self.session_key(session);
            let json = serde_json::to_string(&msg)
                .map_err(|e| anyhow::anyhow!("Message serialization failed: {e}"))?;

            // RPUSH to append, LTRIM to cap
            let _: () = conn
                .rpush(&key, &json)
                .await
                .map_err(|e| anyhow::anyhow!("Redis RPUSH failed: {e}"))?;

            // Trim to keep only the last max_history messages
            let _: () = conn
                .ltrim(&key, -(self.max_history as isize), -1)
                .await
                .map_err(|e| anyhow::anyhow!("Redis LTRIM failed: {e}"))?;

            // Track this session in the sessions set with its latest timestamp
            let ts = msg.timestamp.timestamp_millis();
            let _: () = conn
                .zadd(self.sessions_set_key(), session.as_str(), ts as f64)
                .await
                .map_err(|e| anyhow::anyhow!("Redis ZADD failed: {e}"))?;

            Ok(())
        })
        .await;
        r.map_err(Into::into)
    }

    async fn history(
        &self,
        session: &SessionId,
        limit: usize,
    ) -> Result<Vec<Message>, MemoryError> {
        let r: anyhow::Result<Vec<Message>> = (async {
            let mut conn = self.conn().await?;
            let key = self.session_key(session);

            // Get the last `limit` messages
            let start = -(limit as isize);
            let items: Vec<String> = conn
                .lrange(&key, start, -1)
                .await
                .map_err(|e| anyhow::anyhow!("Redis LRANGE failed: {e}"))?;

            let mut messages = Vec::with_capacity(items.len());
            for json in &items {
                match serde_json::from_str::<Message>(json) {
                    Ok(msg) => messages.push(msg),
                    Err(e) => {
                        warn!(error = %e, "Skipping corrupt message in Redis");
                    }
                }
            }
            Ok(messages)
        })
        .await;
        r.map_err(Into::into)
    }

    async fn clear(&self, session: &SessionId) -> Result<(), MemoryError> {
        let r: anyhow::Result<()> = (async {
            let mut conn = self.conn().await?;
            let key = self.session_key(session);

            let _: () = conn
                .del(&key)
                .await
                .map_err(|e| anyhow::anyhow!("Redis DEL failed: {e}"))?;

            let _: () = conn
                .zrem(self.sessions_set_key(), session.as_str())
                .await
                .map_err(|e| anyhow::anyhow!("Redis ZREM failed: {e}"))?;

            Ok(())
        })
        .await;
        r.map_err(Into::into)
    }

    async fn sessions(&self) -> Result<Vec<SessionId>, MemoryError> {
        let r: anyhow::Result<Vec<SessionId>> = (async {
            let mut conn = self.conn().await?;

            // ZREVRANGEBYSCORE returns most-recently-active first
            let ids: Vec<String> = conn
                .zrevrange(self.sessions_set_key(), 0, -1)
                .await
                .map_err(|e| anyhow::anyhow!("Redis ZREVRANGE failed: {e}"))?;

            Ok(ids.into_iter().map(SessionId::from).collect())
        })
        .await;
        r.map_err(Into::into)
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, MemoryError> {
        let r: anyhow::Result<Vec<SearchHit>> = (async {
            let mut conn = self.conn().await?;
            let q = query.to_lowercase();

            // Get all session IDs
            let session_ids: Vec<String> = conn
                .zrevrange(self.sessions_set_key(), 0, -1)
                .await
                .map_err(|e| anyhow::anyhow!("Redis ZREVRANGE failed: {e}"))?;

            let mut hits: Vec<SearchHit> = Vec::new();

            for sid in &session_ids {
                let sid_id = SessionId::from(sid.as_str());
                let key = self.session_key(&sid_id);
                let items: Vec<String> = conn
                    .lrange(&key, 0, -1)
                    .await
                    .map_err(|e| anyhow::anyhow!("Redis LRANGE failed: {e}"))?;

                for json in &items {
                    if let Ok(msg) = serde_json::from_str::<Message>(json) {
                        if msg.content.to_lowercase().contains(&q) {
                            hits.push(SearchHit::new(sid.clone(), msg));
                        }
                    }
                }

                if hits.len() >= limit {
                    break;
                }
            }

            hits.sort_by(|a, b| b.message.timestamp.cmp(&a.message.timestamp));
            hits.truncate(limit);
            Ok(hits)
        })
        .await;
        r.map_err(Into::into)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_key_has_prefix() {
        // We can't easily test the full store without a Redis server,
        // so test the key building logic.
        let store = RedisStore {
            client: redis::Client::open("redis://127.0.0.1:6379")
                .expect("client creation does not connect"),
            prefix: "test:".to_string(),
            max_history: 50,
        };
        assert_eq!(
            store.session_key(&SessionId::from("abc")),
            "test:session:abc"
        );
        assert_eq!(store.sessions_set_key(), "test:sessions");
    }

    #[test]
    fn default_prefix_format() {
        let store = RedisStore {
            client: redis::Client::open("redis://127.0.0.1:6379")
                .expect("client creation does not connect"),
            prefix: "ironclaw:".to_string(),
            max_history: 100,
        };
        assert_eq!(
            store.session_key(&SessionId::from("user-123")),
            "ironclaw:session:user-123"
        );
    }

    #[tokio::test]
    #[ignore = "requires running Redis server"]
    async fn redis_push_and_history() {
        let store = RedisStore::new("redis://127.0.0.1:6379", "ironclaw_test:", 50)
            .await
            .expect("Redis connection failed — is Redis running?");
        let sid = SessionId::from(format!("test-{}", uuid::Uuid::new_v4()));

        store.push(&sid, Message::user("hello")).await.unwrap();
        store.push(&sid, Message::assistant("world")).await.unwrap();

        let h = store.history(&sid, 10).await.unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].content, "hello");
        assert_eq!(h[1].content, "world");

        // Cleanup
        store.clear(&sid).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Redis server"]
    async fn redis_sessions_and_clear() {
        let store = RedisStore::new("redis://127.0.0.1:6379", "ironclaw_test:", 50)
            .await
            .expect("Redis connection failed — is Redis running?");
        let sid = SessionId::from(format!("test-{}", uuid::Uuid::new_v4()));

        store.push(&sid, Message::user("data")).await.unwrap();

        let sessions = store.sessions().await.unwrap();
        assert!(sessions.contains(&sid));

        store.clear(&sid).await.unwrap();
        let h = store.history(&sid, 10).await.unwrap();
        assert!(h.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires running Redis server"]
    async fn redis_search() {
        let store = RedisStore::new("redis://127.0.0.1:6379", "ironclaw_test:", 50)
            .await
            .expect("Redis connection failed — is Redis running?");
        let sid = SessionId::from(format!("test-{}", uuid::Uuid::new_v4()));

        store
            .push(&sid, Message::user("hello world"))
            .await
            .unwrap();
        store
            .push(&sid, Message::assistant("goodbye"))
            .await
            .unwrap();

        let hits = store.search("hello", 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message.content, "hello world");

        // Cleanup
        store.clear(&sid).await.unwrap();
    }
}
