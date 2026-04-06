use std::str::FromStr;

use async_trait::async_trait;
use ironclaw_core::{
    MemoryError, MemoryStore, Message, Role, SearchHit, SessionId, ToolCall, ToolResult,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use uuid::Uuid;

/// Persistent SQLite-backed session store.
pub struct SqliteStore {
    pool: SqlitePool,
    max_history: usize,
}

impl SqliteStore {
    /// Open (or create) a SQLite database at `path` and run migrations.
    ///
    /// Use `":memory:"` for an ephemeral in-memory database (useful in tests).
    pub async fn new(path: &str, max_history: usize) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::from_str(path)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (
                id          TEXT PRIMARY KEY,
                session_id  TEXT    NOT NULL,
                role        TEXT    NOT NULL,
                content     TEXT    NOT NULL,
                tool_calls  TEXT    NOT NULL DEFAULT '[]',
                tool_result TEXT,
                timestamp   TEXT    NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_messages_session_ts
             ON messages (session_id, timestamp)",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool, max_history })
    }

    /// Trim the oldest messages for `session` so at most `max_history` remain.
    async fn trim(&self, session: &SessionId) -> anyhow::Result<()> {
        sqlx::query(
            "DELETE FROM messages WHERE id IN (
                SELECT id FROM messages
                WHERE session_id = ?1
                ORDER BY timestamp DESC
                LIMIT -1 OFFSET ?2
            )",
        )
        .bind(session.as_str())
        .bind(self.max_history as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Deserialize a database row into a [`Message`].
    fn row_to_message(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<Message> {
        let id_str: String = row.get("id");
        let role_str: String = row.get("role");
        let content: String = row.get("content");
        let tc_json: String = row.get("tool_calls");
        let tr_json: Option<String> = row.get("tool_result");
        let ts_str: String = row.get("timestamp");

        let id = Uuid::parse_str(&id_str)?;
        let role = match role_str.as_str() {
            "system" => Role::System,
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "tool" => Role::Tool,
            other => anyhow::bail!("Unknown role: {other}"),
        };
        let tool_calls: Vec<ToolCall> = serde_json::from_str(&tc_json)?;
        let tool_result: Option<ToolResult> = tr_json
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(serde_json::from_str)
            .transpose()?;
        let timestamp = chrono::DateTime::parse_from_rfc3339(&ts_str)?.to_utc();

        Ok(Message::with_all(
            id,
            role,
            content,
            tool_calls,
            tool_result,
            timestamp,
        ))
    }
}

#[async_trait]
impl MemoryStore for SqliteStore {
    async fn push(&self, session: &SessionId, msg: Message) -> Result<(), MemoryError> {
        let r: anyhow::Result<()> = (async {
        let role_str = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
            _ => "user",
        };
        let tc_json = serde_json::to_string(&msg.tool_calls)?;
        let tr_json = msg
            .tool_result
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let ts_str = msg.timestamp.to_rfc3339();

        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, tool_calls, tool_result, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(msg.id.to_string())
        .bind(session.as_str())
        .bind(role_str)
        .bind(&msg.content)
        .bind(&tc_json)
        .bind(&tr_json)
        .bind(&ts_str)
        .execute(&self.pool)
        .await?;

        self.trim(session).await?;
        Ok(())
        }).await;
        r.map_err(Into::into)
    }

    async fn history(
        &self,
        session: &SessionId,
        limit: usize,
    ) -> Result<Vec<Message>, MemoryError> {
        let r: anyhow::Result<Vec<Message>> = (async {
            let rows = sqlx::query(
                "SELECT * FROM (
                SELECT * FROM messages
                WHERE session_id = ?1
                ORDER BY timestamp DESC
                LIMIT ?2
            ) sub ORDER BY timestamp ASC",
            )
            .bind(session.as_str())
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;

            rows.iter().map(Self::row_to_message).collect()
        })
        .await;
        r.map_err(Into::into)
    }

    async fn clear(&self, session: &SessionId) -> Result<(), MemoryError> {
        let r: anyhow::Result<()> = (async {
            sqlx::query("DELETE FROM messages WHERE session_id = ?1")
                .bind(session.as_str())
                .execute(&self.pool)
                .await?;
            Ok(())
        })
        .await;
        r.map_err(Into::into)
    }

    async fn sessions(&self) -> Result<Vec<SessionId>, MemoryError> {
        let r: anyhow::Result<Vec<SessionId>> = (async {
            let rows = sqlx::query(
                "SELECT session_id, MAX(timestamp) AS last_ts
             FROM messages
             GROUP BY session_id
             ORDER BY last_ts DESC",
            )
            .fetch_all(&self.pool)
            .await?;

            Ok(rows
                .iter()
                .map(|r| {
                    let s: String = r.get("session_id");
                    SessionId::from(s)
                })
                .collect())
        })
        .await;
        r.map_err(Into::into)
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, MemoryError> {
        let r: anyhow::Result<Vec<SearchHit>> = (async {
            let pattern = format!("%{query}%");
            let rows = sqlx::query(
                "SELECT * FROM messages
             WHERE content LIKE ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
            )
            .bind(&pattern)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;

            rows.iter()
                .map(|row| {
                    let session_id: String = row.get("session_id");
                    let message = Self::row_to_message(row)?;
                    Ok(SearchHit::new(session_id, message))
                })
                .collect()
        })
        .await;
        r.map_err(Into::into)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    async fn test_store() -> SqliteStore {
        SqliteStore::new(":memory:", 50).await.unwrap()
    }

    #[tokio::test]
    async fn push_and_history() {
        let store = test_store().await;
        let sid = SessionId::from("s1");
        store.push(&sid, Message::user("hello")).await.unwrap();
        store.push(&sid, Message::assistant("hi")).await.unwrap();

        let h = store.history(&sid, 10).await.unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].content, "hello");
        assert_eq!(h[1].content, "hi");
    }

    #[tokio::test]
    async fn history_respects_limit() {
        let store = test_store().await;
        let sid = SessionId::from("s");
        for i in 0..10 {
            store
                .push(&sid, Message::user(format!("msg {i}")))
                .await
                .unwrap();
        }
        let h = store.history(&sid, 3).await.unwrap();
        assert_eq!(h.len(), 3);
        // Should return the 3 most recent, oldest-first
        assert_eq!(h[0].content, "msg 7");
    }

    #[tokio::test]
    async fn clear_removes_session() {
        let store = test_store().await;
        let sid = SessionId::from("s");
        store.push(&sid, Message::user("x")).await.unwrap();
        store.clear(&sid).await.unwrap();
        let h = store.history(&sid, 10).await.unwrap();
        assert!(h.is_empty());
    }

    #[tokio::test]
    async fn sessions_ordered_by_most_recent() {
        let store = test_store().await;
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
        let store = test_store().await;
        store
            .push(&SessionId::from("s1"), Message::user("hello world"))
            .await
            .unwrap();
        store
            .push(&SessionId::from("s1"), Message::assistant("goodbye"))
            .await
            .unwrap();
        store
            .push(&SessionId::from("s2"), Message::user("HELLO again"))
            .await
            .unwrap();

        // SQLite LIKE is case-insensitive for ASCII by default
        let hits = store.search("hello", 10).await.unwrap();
        assert_eq!(hits.len(), 2);

        let empty = store.search("nonexistent", 10).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn search_respects_limit() {
        let store = test_store().await;
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

    #[tokio::test]
    async fn trim_enforces_max_history() {
        let store = SqliteStore::new(":memory:", 3).await.unwrap();
        let sid = SessionId::from("s");
        for i in 0..6 {
            store
                .push(&sid, Message::user(format!("msg {i}")))
                .await
                .unwrap();
        }
        let h = store.history(&sid, 10).await.unwrap();
        assert_eq!(h.len(), 3);
        assert_eq!(h[0].content, "msg 3");
    }
}
