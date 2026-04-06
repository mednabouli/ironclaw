//! Per-request token cost calculation based on provider pricing.
//!
//! Provides a [`CostCalculator`] that maps provider+model to pricing
//! and computes the dollar cost of a [`CompletionResponse`] from its
//! [`TokenUsage`].

use ironclaw_core::TokenUsage;
use std::collections::HashMap;

// ── Pricing ──────────────────────────────────────────────────────────────

/// Per-token pricing for a specific model.
#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    /// Cost per 1 million prompt (input) tokens in USD.
    pub input_per_million: f64,
    /// Cost per 1 million completion (output) tokens in USD.
    pub output_per_million: f64,
}

impl ModelPricing {
    /// Create a new pricing entry.
    pub const fn new(input_per_million: f64, output_per_million: f64) -> Self {
        Self {
            input_per_million,
            output_per_million,
        }
    }

    /// Calculate cost in USD for the given token usage.
    pub fn calculate(&self, usage: &TokenUsage) -> CostBreakdown {
        let input_cost = (f64::from(usage.prompt_tokens) / 1_000_000.0) * self.input_per_million;
        let output_cost =
            (f64::from(usage.completion_tokens) / 1_000_000.0) * self.output_per_million;

        CostBreakdown {
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
        }
    }
}

// ── Cost Breakdown ───────────────────────────────────────────────────────

/// Detailed cost breakdown for a single request.
#[derive(Debug, Clone)]
pub struct CostBreakdown {
    /// Cost for input/prompt tokens in USD.
    pub input_cost: f64,
    /// Cost for output/completion tokens in USD.
    pub output_cost: f64,
    /// Total cost in USD.
    pub total_cost: f64,
    /// Number of prompt tokens consumed.
    pub input_tokens: u32,
    /// Number of completion tokens consumed.
    pub output_tokens: u32,
}

// ── Cost Calculator ──────────────────────────────────────────────────────

/// Calculates per-request costs based on provider and model pricing tables.
///
/// Pricing is keyed by `"provider/model"` (e.g. `"anthropic/claude-sonnet-4-20250514"`).
/// Use [`CostCalculator::with_defaults`] for built-in pricing, or build
/// a custom table with [`CostCalculator::new`] + [`CostCalculator::add_model`].
#[derive(Debug, Clone)]
pub struct CostCalculator {
    pricing: HashMap<String, ModelPricing>,
}

impl CostCalculator {
    /// Create an empty cost calculator.
    pub fn new() -> Self {
        Self {
            pricing: HashMap::new(),
        }
    }

    /// Create a cost calculator pre-loaded with common model pricing.
    ///
    /// Pricing is approximate and may be outdated — override with
    /// [`add_model`](Self::add_model) for production accuracy.
    pub fn with_defaults() -> Self {
        let mut calc = Self::new();

        // Anthropic
        calc.add_model(
            "anthropic/claude-sonnet-4-20250514",
            ModelPricing::new(3.0, 15.0),
        );
        calc.add_model(
            "anthropic/claude-3-5-haiku-20241022",
            ModelPricing::new(1.0, 5.0),
        );

        // OpenAI
        calc.add_model("openai/gpt-4o", ModelPricing::new(2.50, 10.0));
        calc.add_model("openai/gpt-4o-mini", ModelPricing::new(0.15, 0.60));
        calc.add_model("openai/gpt-4-turbo", ModelPricing::new(10.0, 30.0));

        // Groq (hosted Llama/Mixtral)
        calc.add_model(
            "groq/llama-3.3-70b-versatile",
            ModelPricing::new(0.59, 0.79),
        );
        calc.add_model("groq/mixtral-8x7b-32768", ModelPricing::new(0.24, 0.24));
        calc.add_model("groq/llama-3.1-8b-instant", ModelPricing::new(0.05, 0.08));

        // Ollama (local — free)
        calc.add_model("ollama/llama3", ModelPricing::new(0.0, 0.0));
        calc.add_model("ollama/mistral", ModelPricing::new(0.0, 0.0));

        calc
    }

    /// Register or override pricing for a model.
    ///
    /// Key format: `"provider/model"` (e.g. `"openai/gpt-4o-mini"`).
    pub fn add_model(&mut self, key: impl Into<String>, pricing: ModelPricing) {
        self.pricing.insert(key.into(), pricing);
    }

    /// Calculate cost for the given provider, model, and usage.
    ///
    /// Returns `None` if the model is not in the pricing table.
    pub fn calculate(
        &self,
        provider: &str,
        model: &str,
        usage: &TokenUsage,
    ) -> Option<CostBreakdown> {
        let key = format!("{provider}/{model}");
        self.pricing.get(&key).map(|p| p.calculate(usage))
    }

    /// List all registered model keys.
    pub fn models(&self) -> Vec<&str> {
        self.pricing.keys().map(String::as_str).collect()
    }
}

impl Default for CostCalculator {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_models() {
        let calc = CostCalculator::with_defaults();
        assert!(!calc.models().is_empty());
    }

    #[test]
    fn calculates_openai_cost() {
        let calc = CostCalculator::with_defaults();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };

        let cost = calc.calculate("openai", "gpt-4o", &usage).unwrap();

        // 1000 / 1M * 2.50 = 0.0025  (input)
        // 500  / 1M * 10.0 = 0.005   (output)
        let expected_input = 0.0025;
        let expected_output = 0.005;

        assert!((cost.input_cost - expected_input).abs() < 1e-10);
        assert!((cost.output_cost - expected_output).abs() < 1e-10);
        assert!((cost.total_cost - (expected_input + expected_output)).abs() < 1e-10);
    }

    #[test]
    fn ollama_is_free() {
        let calc = CostCalculator::with_defaults();
        let usage = TokenUsage {
            prompt_tokens: 10_000,
            completion_tokens: 5000,
            total_tokens: 15_000,
        };

        let cost = calc.calculate("ollama", "llama3", &usage).unwrap();
        assert!((cost.total_cost).abs() < 1e-10);
    }

    #[test]
    fn unknown_model_returns_none() {
        let calc = CostCalculator::with_defaults();
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };

        assert!(calc.calculate("unknown", "model", &usage).is_none());
    }

    #[test]
    fn custom_model_pricing() {
        let mut calc = CostCalculator::new();
        calc.add_model("custom/my-model", ModelPricing::new(1.0, 2.0));

        let usage = TokenUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 1_000_000,
            total_tokens: 2_000_000,
        };

        let cost = calc.calculate("custom", "my-model", &usage).unwrap();
        assert!((cost.input_cost - 1.0).abs() < 1e-10);
        assert!((cost.output_cost - 2.0).abs() < 1e-10);
        assert!((cost.total_cost - 3.0).abs() < 1e-10);
    }

    #[test]
    fn zero_tokens_zero_cost() {
        let calc = CostCalculator::with_defaults();
        let usage = TokenUsage::default();
        let cost = calc.calculate("openai", "gpt-4o", &usage).unwrap();
        assert!((cost.total_cost).abs() < 1e-10);
    }
}
