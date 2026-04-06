use std::collections::VecDeque;

use async_trait::async_trait;
use dashmap::DashMap;
use ironclaw_core::{MemoryError, MemoryStore, Message, SearchHit, SessionId};

/// Thread-safe in-memory session store.
pub struct InMemoryStore {
    sessions: DashMap<SessionId, VecDeque<Message>>,
    max_history: usize,
}

impl InMemoryStore {
    /// Create a new in-memory store with the given maximum history per session.
    pub fn new(max_history: usize) -> Self {
        Self {
            sessions: DashMap::new(),
            max_history,
        }
    }
}

#[async_trait]
impl MemoryStore for InMemoryStore {
    async fn push(&self, session: &SessionId, msg: Message) -> Result<(), MemoryError> {
        let mut entry = self.sessions.entry(session.clone()).or_default();
        entry.push_back(msg);
        while entry.len() > self.max_history {
            entry.pop_front();
        }
        Ok(())
    }

    async fn history(
        &self,
        session: &SessionId,
        limit: usize,
    ) -> Result<Vec<Message>, MemoryError> {
        let msgs = self
            .sessions
            .get(session)
            .map(|e| e.iter().rev().take(limit).cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut msgs = msgs;
        msgs.reverse();
        Ok(msgs)
    }

    async fn clear(&self, session: &SessionId) -> Result<(), MemoryError> {
        self.sessions.remove(session);
        Ok(())
    }

    async fn sessions(&self) -> Result<Vec<SessionId>, MemoryError> {
        let mut pairs: Vec<(SessionId, chrono::DateTime<chrono::Utc>)> = self
            .sessions
            .iter()
            .filter_map(|entry| {
                let last_ts = entry.value().back().map(|m| m.timestamp)?;
                Some((entry.key().clone(), last_ts))
            })
            .collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(pairs.into_iter().map(|(id, _)| id).collect())
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, MemoryError> {
        let q = query.to_lowercase();
        let mut hits: Vec<SearchHit> = Vec::new();
        for entry in self.sessions.iter() {
            let session_id = entry.key().clone();
            for msg in entry.value().iter() {
                if msg.content.to_lowercase().contains(&q) {
                    hits.push(SearchHit::new(session_id.clone(), msg.clone()));
                }
            }
        }
        hits.sort_by(|a, b| b.message.timestamp.cmp(&a.message.timestamp));
        hits.truncate(limit);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn push_and_retrieve() {
        let store = InMemoryStore::new(10);
        let sid = SessionId::from("test-session");
        store.push(&sid, Message::user("hello")).await.unwrap();
        store.push(&sid, Message::assistant("world")).await.unwrap();
        let h = store.history(&sid, 10).await.unwrap();
        assert_eq!(h.len(), 2);
    }

    #[tokio::test]
    async fn respects_max_history() {
        let store = InMemoryStore::new(3);
        let sid = SessionId::from("s");
        for i in 0..5 {
            store
                .push(&sid, Message::user(format!("{i}")))
                .await
                .unwrap();
        }
        let h = store.history(&sid, 10).await.unwrap();
        assert_eq!(h.len(), 3);
    }

    #[tokio::test]
    async fn sessions_returns_most_recent_first() {
        let store = InMemoryStore::new(10);
        store
            .push(&SessionId::from("old"), Message::user("first"))
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        store
            .push(&SessionId::from("new"), Message::user("second"))
            .await
            .unwrap();
        let ids = store.sessions().await.unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].as_str(), "new");
        assert_eq!(ids[1].as_str(), "old");
    }

    #[tokio::test]
    async fn search_finds_matching_messages() {
        let store = InMemoryStore::new(10);
        let sid = SessionId::from("s1");
        store
            .push(&sid, Message::user("hello world"))
            .await
            .unwrap();
        store
            .push(&sid, Message::assistant("goodbye"))
            .await
            .unwrap();
        store
            .push(&SessionId::from("s2"), Message::user("HELLO again"))
            .await
            .unwrap();

        let hits = store.search("hello", 10).await.unwrap();
        assert_eq!(hits.len(), 2);

        let empty = store.search("nonexistent", 10).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn search_respects_limit() {
        let store = InMemoryStore::new(10);
        let sid = SessionId::from("s");
        for i in 0..5 {
            store
                .push(&sid, Message::user(format!("match {i}")))
                .await
                .unwrap();
        }
        let hits = store.search("match", 2).await.unwrap();
        assert_eq!(hits.len(), 2);
    }
}
