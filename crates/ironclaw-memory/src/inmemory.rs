
use async_trait::async_trait;
use dashmap::DashMap;
use ironclaw_core::{MemoryStore, Message, SessionId};
use std::collections::VecDeque;

/// Thread-safe in-memory session store.
pub struct InMemoryStore {
    sessions:    DashMap<SessionId, VecDeque<Message>>,
    max_history: usize,
}

impl InMemoryStore {
    pub fn new(max_history: usize) -> Self {
        Self { sessions: DashMap::new(), max_history }
    }
}

#[async_trait]
impl MemoryStore for InMemoryStore {
    async fn push(&self, session: &SessionId, msg: Message) -> anyhow::Result<()> {
        let mut entry = self.sessions.entry(session.clone()).or_default();
        entry.push_back(msg);
        while entry.len() > self.max_history {
            entry.pop_front();
        }
        Ok(())
    }

    async fn history(&self, session: &SessionId, limit: usize) -> anyhow::Result<Vec<Message>> {
        let msgs = self.sessions.get(session)
            .map(|e| e.iter().rev().take(limit).cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut msgs = msgs;
        msgs.reverse();
        Ok(msgs)
    }

    async fn clear(&self, session: &SessionId) -> anyhow::Result<()> {
        self.sessions.remove(session);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn push_and_retrieve() {
        let store = InMemoryStore::new(10);
        let sid   = "test-session".to_string();
        store.push(&sid, Message::user("hello")).await.unwrap();
        store.push(&sid, Message::assistant("world")).await.unwrap();
        let h = store.history(&sid, 10).await.unwrap();
        assert_eq!(h.len(), 2);
    }

    #[tokio::test]
    async fn respects_max_history() {
        let store = InMemoryStore::new(3);
        let sid   = "s".to_string();
        for i in 0..5 { store.push(&sid, Message::user(format!("{i}"))).await.unwrap(); }
        let h = store.history(&sid, 10).await.unwrap();
        assert_eq!(h.len(), 3);
    }
}
