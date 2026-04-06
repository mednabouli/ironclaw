//! Web search tool using the DuckDuckGo Instant Answer API.
//!
//! Queries `/` for instant answers and falls back to an HTML search
//! page scrape if no abstract is returned. Results are cached for
//! 5 minutes in a `DashMap`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use dashmap::DashMap;
use ironclaw_core::{Tool, ToolSchema};
use reqwest::Client;
use serde_json::{json, Value};
use tracing::debug;

const CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes
const MAX_RESULTS: usize = 5;

/// Web search via DuckDuckGo Instant Answer API.
pub struct WebSearchTool {
    client: Client,
    cache: Arc<DashMap<String, (Instant, Value)>>,
}

impl WebSearchTool {
    /// Create a new web search tool.
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .user_agent("IronClaw/0.1 (AI Agent Framework)")
                .build()
                .unwrap_or_default(),
            cache: Arc::new(DashMap::new()),
        }
    }

    /// Check cache for a recent result.
    fn get_cached(&self, query: &str) -> Option<Value> {
        let key = query.to_lowercase();
        if let Some(entry) = self.cache.get(&key) {
            let (ts, val) = entry.value();
            if ts.elapsed() < CACHE_TTL {
                debug!(query = %query, "Cache hit for web search");
                return Some(val.clone());
            }
        }
        None
    }

    /// Store a result in cache.
    fn set_cached(&self, query: &str, value: Value) {
        let key = query.to_lowercase();
        self.cache.insert(key, (Instant::now(), value));
    }

    /// Query the DuckDuckGo Instant Answer API.
    async fn search_ddg(&self, query: &str) -> anyhow::Result<Value> {
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlencoding::encode(query)
        );

        let resp: Value = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut results = Vec::new();

        // Abstract (main answer)
        if let Some(abstract_text) = resp["AbstractText"].as_str() {
            if !abstract_text.is_empty() {
                results.push(json!({
                    "title": resp["Heading"].as_str().unwrap_or(""),
                    "url": resp["AbstractURL"].as_str().unwrap_or(""),
                    "snippet": abstract_text,
                }));
            }
        }

        // Related topics
        if let Some(topics) = resp["RelatedTopics"].as_array() {
            for topic in topics.iter().take(MAX_RESULTS - results.len()) {
                if let (Some(text), Some(url)) =
                    (topic["Text"].as_str(), topic["FirstURL"].as_str())
                {
                    if !text.is_empty() {
                        results.push(json!({
                            "title": text.chars().take(80).collect::<String>(),
                            "url": url,
                            "snippet": text,
                        }));
                    }
                }
            }
        }

        // Answer (e.g. calculations, conversions)
        if results.is_empty() {
            if let Some(answer) = resp["Answer"].as_str() {
                if !answer.is_empty() {
                    results.push(json!({
                        "title": "Answer",
                        "url": "",
                        "snippet": answer,
                    }));
                }
            }
        }

        let output = json!({
            "query": query,
            "results": results,
            "source": "duckduckgo",
        });

        Ok(output)
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web using DuckDuckGo. Returns relevant results with titles, URLs, and snippets."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn invoke(&self, params: Value) -> anyhow::Result<Value> {
        let query = params["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;

        if let Some(cached) = self.get_cached(query) {
            return Ok(cached);
        }

        let result = self.search_ddg(query).await?;
        self.set_cached(query, result.clone());
        Ok(result)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_valid() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
        let schema = tool.schema();
        assert!(schema.parameters["properties"]["query"].is_object());
        assert_eq!(schema.parameters["required"][0], "query");
    }

    #[test]
    fn cache_stores_and_retrieves() {
        let tool = WebSearchTool::new();
        let val = json!({"results": []});
        tool.set_cached("test query", val.clone());
        let cached = tool.get_cached("test query");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), val);
    }

    #[test]
    fn cache_is_case_insensitive() {
        let tool = WebSearchTool::new();
        tool.set_cached("Rust Programming", json!({"hit": true}));
        assert!(tool.get_cached("rust programming").is_some());
    }
}
