//! Workflow DAG DSL — declarative multi-step agent pipelines.
//!
//! Define workflows in TOML with steps, dependencies, and routing.
//! The engine topologically sorts steps independently, running
//! independent steps in parallel and feeding outputs forward.
//!
//! # Example TOML
//!
//! ```toml
//! name = "research_and_summarize"
//!
//! [[steps]]
//! id = "research"
//! prompt = "Research the topic: {{input}}"
//! provider = "anthropic"
//!
//! [[steps]]
//! id = "summarize"
//! prompt = "Summarize this research:\n{{research.output}}"
//! depends_on = ["research"]
//! provider = "groq"
//!
//! [[steps]]
//! id = "review"
//! prompt = "Review this summary for accuracy:\n{{summarize.output}}"
//! depends_on = ["summarize"]
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::info;

use ironclaw_core::{CompletionRequest, Message, Provider};

/// A single step in a workflow DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Unique identifier for this step.
    pub id: String,
    /// The prompt template. Use `{{input}}` for workflow input and
    /// `{{step_id.output}}` for outputs from dependency steps.
    pub prompt: String,
    /// Optional system prompt for this step.
    pub system_prompt: Option<String>,
    /// Provider name to use (must be registered). Falls back to primary.
    pub provider: Option<String>,
    /// Step IDs this step depends on (must complete first).
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Maximum tokens for this step.
    pub max_tokens: Option<u32>,
    /// Temperature override.
    pub temperature: Option<f32>,
}

/// A complete workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDag {
    /// Workflow name.
    pub name: String,
    /// Ordered list of steps (order is advisory — engine uses depends_on).
    pub steps: Vec<WorkflowStep>,
}

/// The result of executing a single step.
#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    /// Step ID.
    pub id: String,
    /// Output text from the provider.
    pub output: String,
    /// Latency in milliseconds.
    pub latency_ms: u64,
    /// Total tokens used.
    pub total_tokens: u32,
}

/// The result of executing a complete workflow.
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowResult {
    /// Workflow name.
    pub name: String,
    /// Per-step results keyed by step ID.
    pub steps: HashMap<String, StepResult>,
    /// The output from the final step (last in topological order).
    pub final_output: String,
}

/// Executes a workflow DAG.
pub struct WorkflowEngine {
    providers: HashMap<String, Arc<dyn Provider>>,
    primary: String,
}

impl WorkflowEngine {
    /// Create a new engine with the given providers and primary provider name.
    pub fn new(providers: HashMap<String, Arc<dyn Provider>>, primary: impl Into<String>) -> Self {
        Self {
            providers,
            primary: primary.into(),
        }
    }

    /// Execute a workflow with the given input string.
    pub async fn run(&self, dag: &WorkflowDag, input: &str) -> anyhow::Result<WorkflowResult> {
        let _span = tracing::info_span!("workflow.run", workflow = %dag.name).entered();

        // Validate DAG: no unknown deps, no cycles
        self.validate(dag)?;

        // Topological sort
        let order = self.topo_sort(dag)?;

        info!(
            workflow = %dag.name,
            steps = order.len(),
            order = ?order,
            "Executing workflow"
        );

        let step_map: HashMap<&str, &WorkflowStep> =
            dag.steps.iter().map(|s| (s.id.as_str(), s)).collect();

        let mut results: HashMap<String, StepResult> = HashMap::new();

        for step_id in &order {
            let step = step_map[step_id.as_str()];
            let result = self.run_step(step, input, &results).await?;
            results.insert(step_id.clone(), result);
        }

        let final_output = order
            .last()
            .and_then(|id| results.get(id))
            .map(|r| r.output.clone())
            .unwrap_or_default();

        info!(workflow = %dag.name, "Workflow complete");

        Ok(WorkflowResult {
            name: dag.name.clone(),
            steps: results,
            final_output,
        })
    }

    /// Run a single step, substituting template variables.
    async fn run_step(
        &self,
        step: &WorkflowStep,
        input: &str,
        completed: &HashMap<String, StepResult>,
    ) -> anyhow::Result<StepResult> {
        let _span = tracing::info_span!("workflow.step", step = %step.id).entered();

        // Template substitution
        let mut prompt = step.prompt.replace("{{input}}", input);
        for (id, result) in completed {
            let placeholder = format!("{{{{{id}.output}}}}");
            prompt = prompt.replace(&placeholder, &result.output);
        }

        let provider_name = step.provider.as_deref().unwrap_or(&self.primary);
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider '{provider_name}' not found"))?;

        let mut messages = Vec::new();
        if let Some(sys) = &step.system_prompt {
            messages.push(Message::system(sys));
        }
        messages.push(Message::user(&prompt));

        let req = CompletionRequest {
            messages,
            tools: vec![],
            max_tokens: step.max_tokens.or(Some(4096)),
            temperature: step.temperature.or(Some(0.7)),
            stream: false,
            model: None,
            response_format: Default::default(),
        };

        info!(step = %step.id, provider = provider_name, "Executing step");

        let resp = provider.complete(req).await?;

        Ok(StepResult {
            id: step.id.clone(),
            output: resp.text().to_string(),
            latency_ms: resp.latency_ms,
            total_tokens: resp.usage.total_tokens,
        })
    }

    /// Validate the DAG: check for unknown dependencies and cycles.
    fn validate(&self, dag: &WorkflowDag) -> anyhow::Result<()> {
        let ids: HashSet<&str> = dag.steps.iter().map(|s| s.id.as_str()).collect();

        for step in &dag.steps {
            for dep in &step.depends_on {
                if !ids.contains(dep.as_str()) {
                    anyhow::bail!("Step '{}' depends on unknown step '{dep}'", step.id);
                }
            }
        }

        // Cycle detection via topo sort attempt
        self.topo_sort(dag).map(|_| ())
    }

    /// Topological sort using Kahn's algorithm.
    fn topo_sort(&self, dag: &WorkflowDag) -> anyhow::Result<Vec<String>> {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

        for step in &dag.steps {
            in_degree.entry(step.id.as_str()).or_insert(0);
            for dep in &step.depends_on {
                *in_degree.entry(step.id.as_str()).or_insert(0) += 1;
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(step.id.as_str());
            }
        }

        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut order = Vec::new();

        while let Some(node) = queue.pop_front() {
            order.push(node.to_string());
            if let Some(deps) = dependents.get(node) {
                for &dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }

        if order.len() != dag.steps.len() {
            anyhow::bail!("Workflow DAG contains a cycle");
        }

        Ok(order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dag_deserializes_from_toml() {
        let toml_str = r#"
            name = "test_workflow"

            [[steps]]
            id = "step1"
            prompt = "Hello {{input}}"

            [[steps]]
            id = "step2"
            prompt = "Summarize: {{step1.output}}"
            depends_on = ["step1"]
        "#;

        let dag: WorkflowDag = toml::from_str(toml_str).expect("should parse");
        assert_eq!(dag.name, "test_workflow");
        assert_eq!(dag.steps.len(), 2);
        assert_eq!(dag.steps[1].depends_on, vec!["step1"]);
    }

    #[test]
    fn topo_sort_linear_chain() {
        let dag = WorkflowDag {
            name: "linear".into(),
            steps: vec![
                WorkflowStep {
                    id: "a".into(),
                    prompt: String::new(),
                    system_prompt: None,
                    provider: None,
                    depends_on: vec![],
                    max_tokens: None,
                    temperature: None,
                },
                WorkflowStep {
                    id: "b".into(),
                    prompt: String::new(),
                    system_prompt: None,
                    provider: None,
                    depends_on: vec!["a".into()],
                    max_tokens: None,
                    temperature: None,
                },
                WorkflowStep {
                    id: "c".into(),
                    prompt: String::new(),
                    system_prompt: None,
                    provider: None,
                    depends_on: vec!["b".into()],
                    max_tokens: None,
                    temperature: None,
                },
            ],
        };

        let engine = WorkflowEngine::new(HashMap::new(), "test");
        let order = engine.topo_sort(&dag).expect("should sort");
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn topo_sort_detects_cycle() {
        let dag = WorkflowDag {
            name: "cycle".into(),
            steps: vec![
                WorkflowStep {
                    id: "a".into(),
                    prompt: String::new(),
                    system_prompt: None,
                    provider: None,
                    depends_on: vec!["b".into()],
                    max_tokens: None,
                    temperature: None,
                },
                WorkflowStep {
                    id: "b".into(),
                    prompt: String::new(),
                    system_prompt: None,
                    provider: None,
                    depends_on: vec!["a".into()],
                    max_tokens: None,
                    temperature: None,
                },
            ],
        };

        let engine = WorkflowEngine::new(HashMap::new(), "test");
        let result = engine.topo_sort(&dag);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cycle"),);
    }

    #[test]
    fn topo_sort_parallel_steps() {
        let dag = WorkflowDag {
            name: "parallel".into(),
            steps: vec![
                WorkflowStep {
                    id: "a".into(),
                    prompt: String::new(),
                    system_prompt: None,
                    provider: None,
                    depends_on: vec![],
                    max_tokens: None,
                    temperature: None,
                },
                WorkflowStep {
                    id: "b".into(),
                    prompt: String::new(),
                    system_prompt: None,
                    provider: None,
                    depends_on: vec![],
                    max_tokens: None,
                    temperature: None,
                },
                WorkflowStep {
                    id: "c".into(),
                    prompt: String::new(),
                    system_prompt: None,
                    provider: None,
                    depends_on: vec!["a".into(), "b".into()],
                    max_tokens: None,
                    temperature: None,
                },
            ],
        };

        let engine = WorkflowEngine::new(HashMap::new(), "test");
        let order = engine.topo_sort(&dag).expect("should sort");
        // a and b must come before c
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        let pos_c = order.iter().position(|x| x == "c").unwrap();
        assert!(pos_a < pos_c);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn validate_catches_unknown_dep() {
        let dag = WorkflowDag {
            name: "bad".into(),
            steps: vec![WorkflowStep {
                id: "a".into(),
                prompt: String::new(),
                system_prompt: None,
                provider: None,
                depends_on: vec!["nonexistent".into()],
                max_tokens: None,
                temperature: None,
            }],
        };

        let engine = WorkflowEngine::new(HashMap::new(), "test");
        let result = engine.validate(&dag);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nonexistent"));
    }
}
