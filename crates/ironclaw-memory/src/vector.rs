//! SQLite-backed vector store for RAG (Retrieval-Augmented Generation).
//!
//! Stores text chunks alongside their embedding vectors (as BLOBs) and
//! performs cosine similarity search entirely in Rust. No native SQLite
//! vector extensions are required — vectors are loaded into memory for
//! the similarity scan, which is practical for up to ~100k embeddings.
//!
//! For larger-scale deployments, replace this with a dedicated vector
//! database (Qdrant, Milvus, pgvector, etc.) behind the same
//! [`VectorStore`] trait.

use async_trait::async_trait;
use ironclaw_core::{MemoryError, MemoryHit, VectorStore};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::str::FromStr;
use tracing::debug;

/// Persistent SQLite-backed vector store.
///
/// Embeddings are stored as little-endian f32 BLOBs. Cosine similarity is
/// computed in Rust, not SQL, to avoid requiring native extensions.
pub struct SqliteVectorStore {
    pool: SqlitePool,
    dimensions: usize,
}

impl SqliteVectorStore {
    /// Open (or create) a SQLite vector database at `path`.
    ///
    /// `dimensions` is the expected embedding vector size
    /// (e.g. 384 for all-MiniLM-L6-v2, 1536 for OpenAI text-embedding-3-small).
    ///
    /// Pass `":memory:"` for an ephemeral in-memory database (useful in tests).
    pub async fn new(path: &str, dimensions: usize) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::from_str(path)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS embeddings (
                id        TEXT PRIMARY KEY,
                text      TEXT    NOT NULL,
                vector    BLOB    NOT NULL,
                metadata  TEXT    NOT NULL DEFAULT '{}',
                created   TEXT    NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool, dimensions })
    }

    /// Encode an f32 slice as a little-endian byte blob.
    fn encode_vector(v: &[f32]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(v.len() * 4);
        for &val in v {
            buf.extend_from_slice(&val.to_le_bytes());
        }
        buf
    }

    /// Decode a little-endian byte blob back into an f32 slice.
    fn decode_vector(blob: &[u8]) -> Vec<f32> {
        blob.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    /// Compute cosine similarity between two vectors.
    ///
    /// Returns a value in \[-1, 1\]; higher = more similar.
    /// Returns 0.0 if either vector has zero magnitude.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "vector dimension mismatch");

        let mut dot = 0.0_f32;
        let mut norm_a = 0.0_f32;
        let mut norm_b = 0.0_f32;

        for (x, y) in a.iter().zip(b.iter()) {
            dot += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }

        let denom = norm_a.sqrt() * norm_b.sqrt();
        if denom < f32::EPSILON {
            return 0.0;
        }
        dot / denom
    }
}

#[async_trait]
impl VectorStore for SqliteVectorStore {
    async fn upsert(
        &self,
        id: &str,
        text: &str,
        embedding: &[f32],
        metadata: serde_json::Value,
    ) -> Result<(), MemoryError> {
        let r: anyhow::Result<()> = (async {
            if embedding.len() != self.dimensions {
                anyhow::bail!(
                    "Embedding dimension mismatch: expected {}, got {}",
                    self.dimensions,
                    embedding.len()
                );
            }

            let blob = Self::encode_vector(embedding);
            let meta_json = serde_json::to_string(&metadata)?;
            let now = chrono::Utc::now().to_rfc3339();

            sqlx::query(
                "INSERT INTO embeddings (id, text, vector, metadata, created)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                text     = excluded.text,
                vector   = excluded.vector,
                metadata = excluded.metadata,
                created  = excluded.created",
            )
            .bind(id)
            .bind(text)
            .bind(&blob)
            .bind(&meta_json)
            .bind(&now)
            .execute(&self.pool)
            .await?;

            debug!(id, dims = embedding.len(), "Stored embedding");
            Ok(())
        })
        .await;
        r.map_err(Into::into)
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<MemoryHit>, MemoryError> {
        let r: anyhow::Result<Vec<MemoryHit>> = (async {
            if query_embedding.len() != self.dimensions {
                anyhow::bail!(
                    "Query embedding dimension mismatch: expected {}, got {}",
                    self.dimensions,
                    query_embedding.len()
                );
            }

            // Load all embeddings — this is the brute-force approach suitable
            // for small-to-medium corpora. For 100k+ vectors, swap in an ANN
            // index (HNSW via a dedicated vector DB).
            let rows = sqlx::query("SELECT id, text, vector, metadata FROM embeddings")
                .fetch_all(&self.pool)
                .await?;

            let mut scored: Vec<(f32, MemoryHit)> = Vec::with_capacity(rows.len());

            for row in &rows {
                let id: String = row.get("id");
                let text: String = row.get("text");
                let blob: Vec<u8> = row.get("vector");
                let meta_json: String = row.get("metadata");

                let stored = Self::decode_vector(&blob);
                let score = Self::cosine_similarity(query_embedding, &stored);

                let metadata: serde_json::Value =
                    serde_json::from_str(&meta_json).unwrap_or_default();

                scored.push((score, MemoryHit::new(id, text, score, metadata)));
            }

            // Sort by descending score
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit);

            Ok(scored.into_iter().map(|(_, hit)| hit).collect())
        })
        .await;
        r.map_err(Into::into)
    }

    async fn delete(&self, id: &str) -> Result<(), MemoryError> {
        let r: anyhow::Result<()> = (async {
            sqlx::query("DELETE FROM embeddings WHERE id = ?1")
                .bind(id)
                .execute(&self.pool)
                .await?;
            Ok(())
        })
        .await;
        r.map_err(Into::into)
    }

    async fn count(&self) -> Result<usize, MemoryError> {
        let r: anyhow::Result<usize> = (async {
            let row = sqlx::query("SELECT COUNT(*) AS cnt FROM embeddings")
                .fetch_one(&self.pool)
                .await?;
            let cnt: i64 = row.get("cnt");
            Ok(cnt as usize)
        })
        .await;
        r.map_err(Into::into)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn test_store(dims: usize) -> SqliteVectorStore {
        SqliteVectorStore::new(":memory:", dims).await.unwrap()
    }

    #[test]
    fn encode_decode_roundtrip() {
        let original = vec![1.0_f32, -2.5, 0.0, 3.125];
        let blob = SqliteVectorStore::encode_vector(&original);
        let decoded = SqliteVectorStore::decode_vector(&blob);
        assert_eq!(original, decoded);
    }

    #[test]
    fn cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = SqliteVectorStore::cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = SqliteVectorStore::cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = SqliteVectorStore::cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_zero_vector() {
        let a = vec![1.0, 2.0];
        let b = vec![0.0, 0.0];
        let sim = SqliteVectorStore::cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[tokio::test]
    async fn upsert_and_count() {
        let store = test_store(3).await;
        store
            .upsert("a", "hello world", &[1.0, 0.0, 0.0], json!({}))
            .await
            .unwrap();
        store
            .upsert("b", "goodbye moon", &[0.0, 1.0, 0.0], json!({}))
            .await
            .unwrap();
        assert_eq!(store.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn upsert_replaces_existing() {
        let store = test_store(3).await;
        store
            .upsert("a", "original", &[1.0, 0.0, 0.0], json!({}))
            .await
            .unwrap();
        store
            .upsert("a", "updated", &[0.0, 1.0, 0.0], json!({}))
            .await
            .unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        let hits = store.search(&[0.0, 1.0, 0.0], 10).await.unwrap();
        assert_eq!(hits[0].text, "updated");
    }

    #[tokio::test]
    async fn search_returns_nearest() {
        let store = test_store(3).await;
        // Two embeddings: one very close to query, one far
        store
            .upsert(
                "close",
                "close text",
                &[0.9, 0.1, 0.0],
                json!({"tag": "near"}),
            )
            .await
            .unwrap();
        store
            .upsert("far", "far text", &[0.0, 0.0, 1.0], json!({"tag": "far"}))
            .await
            .unwrap();

        let hits = store.search(&[1.0, 0.0, 0.0], 10).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "close");
        assert!(hits[0].score > hits[1].score);
        assert_eq!(hits[0].metadata["tag"], "near");
    }

    #[tokio::test]
    async fn search_respects_limit() {
        let store = test_store(2).await;
        for i in 0..5 {
            store
                .upsert(
                    &format!("v{i}"),
                    &format!("text {i}"),
                    &[i as f32, 1.0],
                    json!({}),
                )
                .await
                .unwrap();
        }
        let hits = store.search(&[4.0, 1.0], 2).await.unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[tokio::test]
    async fn delete_removes_entry() {
        let store = test_store(3).await;
        store
            .upsert("x", "data", &[1.0, 0.0, 0.0], json!({}))
            .await
            .unwrap();
        assert_eq!(store.count().await.unwrap(), 1);
        store.delete("x").await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn dimension_mismatch_rejected() {
        let store = test_store(3).await;
        let result = store.upsert("bad", "text", &[1.0, 2.0], json!({})).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("dimension mismatch"));
    }

    #[tokio::test]
    async fn search_dimension_mismatch_rejected() {
        let store = test_store(3).await;
        let result = store.search(&[1.0, 2.0], 10).await;
        assert!(result.is_err());
    }
}
