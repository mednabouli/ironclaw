//! Evaluation harness for automated prompt regression testing.
//!
//! Defines [`EvalCase`], [`EvalSuite`], and [`EvalRunner`] for running
//! structured evaluations against LLM providers.
//!
//! # Example TOML evaluation file
//!
//! ```toml
//! [[cases]]
//! name = "basic_math"
//! prompt = "What is 2+2?"
//! contains = ["4"]
//! max_tokens = 256
//!
//! [[cases]]
//! name = "capital_france"
//! prompt = "What is the capital of France?"
//! contains = ["Paris"]
//! not_contains = ["London"]
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use ironclaw_core::{CompletionRequest, Message, Provider};

/// A single evaluation test case with expected assertions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    /// Human-readable name for this case.
    pub name: String,
    /// The user prompt to send.
    pub prompt: String,
    /// Optional system prompt override.
    pub system_prompt: Option<String>,
    /// Substrings the response must contain (case-insensitive).
    #[serde(default)]
    pub contains: Vec<String>,
    /// Substrings the response must NOT contain (case-insensitive).
    #[serde(default)]
    pub not_contains: Vec<String>,
    /// Maximum tokens for this request.
    pub max_tokens: Option<u32>,
    /// Temperature override.
    pub temperature: Option<f32>,
}

/// A collection of evaluation cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSuite {
    /// Suite name.
    #[serde(default = "default_suite_name")]
    pub name: String,
    /// All test cases in the suite.
    pub cases: Vec<EvalCase>,
}

fn default_suite_name() -> String {
    "default".into()
}

/// Result of running a single eval case.
#[derive(Debug, Clone, Serialize)]
pub struct EvalResult {
    /// Case name.
    pub name: String,
    /// Whether all assertions passed.
    pub passed: bool,
    /// The actual response text from the provider.
    pub response: String,
    /// Assertion failures (empty if passed).
    pub failures: Vec<String>,
    /// Latency in milliseconds.
    pub latency_ms: u64,
    /// Tokens used.
    pub total_tokens: u32,
}

/// Summary of a full suite run.
#[derive(Debug, Clone, Serialize)]
pub struct EvalSummary {
    /// Suite name.
    pub suite: String,
    /// Provider used.
    pub provider: String,
    /// Total cases executed.
    pub total: usize,
    /// Number of passing cases.
    pub passed: usize,
    /// Number of failing cases.
    pub failed: usize,
    /// Per-case results.
    pub results: Vec<EvalResult>,
}

/// Runs an eval suite against a provider.
pub struct EvalRunner {
    provider: Arc<dyn Provider>,
}

impl EvalRunner {
    /// Create a new eval runner with the given provider.
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self { provider }
    }

    /// Run all cases in the suite and collect results.
    pub async fn run(&self, suite: &EvalSuite) -> EvalSummary {
        let provider_name = self.provider.name().to_string();
        info!(
            suite = %suite.name,
            provider = %provider_name,
            cases = suite.cases.len(),
            "Starting eval suite"
        );

        let mut results = Vec::with_capacity(suite.cases.len());

        for case in &suite.cases {
            let result = self.run_case(case).await;
            if result.passed {
                info!(case = %result.name, "PASS");
            } else {
                warn!(
                    case = %result.name,
                    failures = ?result.failures,
                    "FAIL"
                );
            }
            results.push(result);
        }

        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.len() - passed;

        info!(
            suite = %suite.name,
            passed,
            failed,
            total = results.len(),
            "Eval suite complete"
        );

        EvalSummary {
            suite: suite.name.clone(),
            provider: provider_name,
            total: results.len(),
            passed,
            failed,
            results,
        }
    }

    /// Run a single eval case.
    async fn run_case(&self, case: &EvalCase) -> EvalResult {
        let _span = tracing::info_span!("eval.case", case = %case.name).entered();

        let mut messages = Vec::new();
        if let Some(sys) = &case.system_prompt {
            messages.push(Message::system(sys));
        }
        messages.push(Message::user(&case.prompt));

        let req = CompletionRequest::builder(messages)
            .max_tokens(case.max_tokens.unwrap_or(1024))
            .temperature(case.temperature.unwrap_or(0.0))
            .build();

        match self.provider.complete(req).await {
            Ok(resp) => {
                let text = resp.text().to_string();
                let lower = text.to_lowercase();
                let mut failures = Vec::new();

                for needle in &case.contains {
                    if !lower.contains(&needle.to_lowercase()) {
                        failures.push(format!("expected response to contain '{needle}'"));
                    }
                }
                for needle in &case.not_contains {
                    if lower.contains(&needle.to_lowercase()) {
                        failures.push(format!("expected response NOT to contain '{needle}'"));
                    }
                }

                EvalResult {
                    name: case.name.clone(),
                    passed: failures.is_empty(),
                    response: text,
                    failures,
                    latency_ms: resp.latency_ms,
                    total_tokens: resp.usage.total_tokens,
                }
            }
            Err(e) => EvalResult {
                name: case.name.clone(),
                passed: false,
                response: String::new(),
                failures: vec![format!("Provider error: {e}")],
                latency_ms: 0,
                total_tokens: 0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_case_deserializes() {
        let toml_str = r#"
            [[cases]]
            name = "basic_math"
            prompt = "What is 2+2?"
            contains = ["4"]
            max_tokens = 256
        "#;

        let suite: EvalSuite = toml::from_str(toml_str).expect("should parse");
        assert_eq!(suite.cases.len(), 1);
        assert_eq!(suite.cases[0].name, "basic_math");
        assert_eq!(suite.cases[0].contains, vec!["4"]);
    }

    #[test]
    fn eval_case_with_not_contains() {
        let toml_str = r#"
            [[cases]]
            name = "capital_test"
            prompt = "What is the capital of France?"
            contains = ["Paris"]
            not_contains = ["London"]
        "#;

        let suite: EvalSuite = toml::from_str(toml_str).expect("should parse");
        assert_eq!(suite.cases[0].not_contains, vec!["London"]);
    }

    #[test]
    fn eval_suite_default_name() {
        let toml_str = r#"
            [[cases]]
            name = "test"
            prompt = "Hello"
        "#;

        let suite: EvalSuite = toml::from_str(toml_str).expect("should parse");
        assert_eq!(suite.name, "default");
    }
}
