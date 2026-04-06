use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use ironclaw_core::*;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// A node in the swarm DAG.
///
/// Each node wraps an agent and declares dependencies on other nodes
/// (by node name). A node only executes once all its dependencies have completed.
#[derive(Clone)]
pub struct SwarmNode {
    /// Unique name for this node within the swarm.
    pub name: String,
    /// The agent that will execute when this node runs.
    pub agent: Arc<dyn Agent>,
    /// Names of nodes that must complete before this node can run.
    pub depends_on: Vec<String>,
}

impl SwarmNode {
    /// Create a new swarm node with no dependencies.
    pub fn new(name: impl Into<String>, agent: Arc<dyn Agent>) -> Self {
        Self {
            name: name.into(),
            agent,
            depends_on: vec![],
        }
    }

    /// Add a dependency on another node.
    pub fn depends_on(mut self, node_name: impl Into<String>) -> Self {
        self.depends_on.push(node_name.into());
        self
    }
}

/// DAG-based agent orchestrator.
///
/// Nodes are agents with declared dependencies. The engine performs a
/// topological sort and executes nodes level-by-level: all nodes in a level
/// run concurrently (they share no unmet dependencies).
///
/// Results from upstream nodes are injected into the task context of
/// downstream nodes, enabling data flow through the DAG.
pub struct SwarmEngine {
    id: AgentId,
    nodes: Vec<SwarmNode>,
}

impl SwarmEngine {
    /// Create a new swarm engine.
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string().into(),
            nodes: vec![],
        }
    }

    /// Add a node to the swarm DAG.
    pub fn add_node(mut self, node: SwarmNode) -> Self {
        self.nodes.push(node);
        self
    }

    /// Compute execution levels via topological sort (Kahn's algorithm).
    ///
    /// Returns `Ok(levels)` where each level is a vec of node indices that
    /// can execute concurrently, or `Err` if the graph has a cycle.
    fn topological_levels(&self) -> anyhow::Result<Vec<Vec<usize>>> {
        let name_to_idx: HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.name.as_str(), i))
            .collect();

        let n = self.nodes.len();
        let mut in_degree = vec![0usize; n];
        let mut dependents: Vec<Vec<usize>> = vec![vec![]; n];

        for (i, node) in self.nodes.iter().enumerate() {
            for dep_name in &node.depends_on {
                let dep_idx = name_to_idx.get(dep_name.as_str()).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Node '{}' depends on unknown node '{}'",
                        node.name,
                        dep_name
                    )
                })?;
                dependents[*dep_idx].push(i);
                in_degree[i] += 1;
            }
        }

        let mut queue: VecDeque<usize> = VecDeque::new();
        for (i, &deg) in in_degree.iter().enumerate() {
            if deg == 0 {
                queue.push_back(i);
            }
        }

        let mut levels: Vec<Vec<usize>> = vec![];
        let mut visited = 0usize;

        while !queue.is_empty() {
            let level: Vec<usize> = queue.drain(..).collect();
            visited += level.len();

            for &idx in &level {
                for &dep_idx in &dependents[idx] {
                    in_degree[dep_idx] -= 1;
                    if in_degree[dep_idx] == 0 {
                        queue.push_back(dep_idx);
                    }
                }
            }
            levels.push(level);
        }

        if visited != n {
            anyhow::bail!("Cycle detected in swarm DAG");
        }

        Ok(levels)
    }
}

impl Default for SwarmEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for SwarmEngine {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn role(&self) -> AgentRole {
        AgentRole::Orchestrator
    }

    async fn run(&self, task: AgentTask) -> Result<AgentOutput, AgentError> {
        let span = tracing::info_span!(
            "swarm.run",
            agent_id = %self.id,
            node_count = self.nodes.len(),
        );
        let _guard = span.enter();
        drop(_guard);

        let levels = self.topological_levels()?;
        info!(levels = levels.len(), "DAG sorted into levels");

        // Shared results map: node_name → output text
        let results: Arc<RwLock<HashMap<String, String>>> = Arc::new(RwLock::new(HashMap::new()));

        let mut total_usage = TokenUsage::default();
        let mut all_approved = true;

        for (level_idx, level) in levels.iter().enumerate() {
            debug!(level = level_idx, nodes = level.len(), "Executing level");

            let mut handles = Vec::new();

            for &node_idx in level {
                let node = &self.nodes[node_idx];
                let agent = Arc::clone(&node.agent);
                let node_name = node.name.clone();
                let deps = node.depends_on.clone();
                let results_ref = Arc::clone(&results);
                let base_instruction = task.instruction.clone();
                let base_context = task.context.clone();
                let tool_allowlist = task.tool_allowlist.clone();
                let max_tokens = task.max_tokens;

                handles.push(tokio::spawn(async move {
                    // Build context from upstream results
                    let mut context = base_context;
                    if !deps.is_empty() {
                        let rlock = results_ref.read().await;
                        let upstream_text: Vec<String> = deps
                            .iter()
                            .filter_map(|d| rlock.get(d).map(|v| format!("[{d}]: {v}")))
                            .collect();
                        if !upstream_text.is_empty() {
                            context.push(Message::system(format!(
                                "Results from upstream agents:\n{}",
                                upstream_text.join("\n")
                            )));
                        }
                    }

                    let mut builder = AgentTask::builder(base_instruction).context(context);
                    if let Some(allowlist) = tool_allowlist {
                        builder = builder.tool_allowlist(allowlist);
                    }
                    if let Some(n) = max_tokens {
                        builder = builder.max_tokens(n);
                    }
                    let sub_task = builder.build();

                    let output = agent.run(sub_task).await;
                    (node_name, output)
                }));
            }

            for handle in handles {
                let (node_name, result) = handle
                    .await
                    .map_err(|e| anyhow::anyhow!("Swarm node task panicked: {e}"))?;

                match result {
                    Ok(output) => {
                        total_usage.prompt_tokens += output.usage.prompt_tokens;
                        total_usage.completion_tokens += output.usage.completion_tokens;
                        total_usage.total_tokens += output.usage.total_tokens;
                        if !output.approved {
                            all_approved = false;
                        }
                        results.write().await.insert(node_name, output.text);
                    }
                    Err(e) => {
                        warn!(node = %node_name, error = %e, "Swarm node failed");
                        all_approved = false;
                        results
                            .write()
                            .await
                            .insert(node_name, format!("ERROR: {e}"));
                    }
                }
            }
        }

        // Combine all results ordered by node definition order
        let final_results = results.read().await;
        let combined: Vec<String> = self
            .nodes
            .iter()
            .filter_map(|n| {
                final_results
                    .get(&n.name)
                    .map(|text| format!("## {}\n{}", n.name, text))
            })
            .collect();

        info!(
            node_count = self.nodes.len(),
            result_len = combined.iter().map(|s| s.len()).sum::<usize>(),
            "Swarm complete"
        );

        Ok(
            AgentOutput::new(task.id, self.id.clone(), combined.join("\n\n"))
                .with_approved(all_approved)
                .with_usage(total_usage),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    struct NamedEchoAgent {
        id: AgentId,
        label: String,
    }

    #[async_trait]
    impl Agent for NamedEchoAgent {
        fn id(&self) -> &AgentId {
            &self.id
        }
        fn role(&self) -> AgentRole {
            AgentRole::Worker
        }
        async fn run(&self, task: AgentTask) -> Result<AgentOutput, AgentError> {
            // Include any upstream context in output so we can verify data flow
            let upstream: Vec<String> = task
                .context
                .iter()
                .filter(|m| m.role == Role::System && m.content.contains("upstream"))
                .map(|m| m.content.clone())
                .collect();
            let text = if upstream.is_empty() {
                format!("echo from {}", self.label)
            } else {
                format!("echo from {} (with upstream)", self.label)
            };
            Ok(AgentOutput::new(task.id, self.id.clone(), text).with_approved(true))
        }
    }

    fn make_echo(label: &str) -> Arc<dyn Agent> {
        Arc::new(NamedEchoAgent {
            id: label.into(),
            label: label.to_string(),
        })
    }

    #[test]
    fn topological_sort_linear_chain() {
        let engine = SwarmEngine::new()
            .add_node(SwarmNode::new("a", make_echo("a")))
            .add_node(SwarmNode::new("b", make_echo("b")).depends_on("a"))
            .add_node(SwarmNode::new("c", make_echo("c")).depends_on("b"));

        let levels = engine.topological_levels().unwrap();
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec![0]); // a
        assert_eq!(levels[1], vec![1]); // b
        assert_eq!(levels[2], vec![2]); // c
    }

    #[test]
    fn topological_sort_parallel_roots() {
        let engine = SwarmEngine::new()
            .add_node(SwarmNode::new("a", make_echo("a")))
            .add_node(SwarmNode::new("b", make_echo("b")))
            .add_node(
                SwarmNode::new("c", make_echo("c"))
                    .depends_on("a")
                    .depends_on("b"),
            );

        let levels = engine.topological_levels().unwrap();
        assert_eq!(levels.len(), 2);
        // First level has both a and b
        let first: HashSet<usize> = levels[0].iter().copied().collect();
        assert!(first.contains(&0));
        assert!(first.contains(&1));
        assert_eq!(levels[1], vec![2]);
    }

    #[test]
    fn topological_sort_detects_cycle() {
        let engine = SwarmEngine::new()
            .add_node(SwarmNode::new("a", make_echo("a")).depends_on("b"))
            .add_node(SwarmNode::new("b", make_echo("b")).depends_on("a"));

        let result = engine.topological_levels();
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("Cycle"),
            "Should detect cycle"
        );
    }

    #[test]
    fn topological_sort_unknown_dep_errors() {
        let engine = SwarmEngine::new()
            .add_node(SwarmNode::new("a", make_echo("a")).depends_on("nonexistent"));

        let result = engine.topological_levels();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown node"));
    }

    #[tokio::test]
    async fn swarm_executes_dag() {
        let engine = SwarmEngine::new()
            .add_node(SwarmNode::new("fetch", make_echo("fetch")))
            .add_node(SwarmNode::new("parse", make_echo("parse")).depends_on("fetch"))
            .add_node(SwarmNode::new("summarize", make_echo("summarize")).depends_on("parse"));

        let task = AgentTask::new("process data");
        let output = engine.run(task).await.unwrap();

        assert!(output.approved);
        assert!(output.text.contains("## fetch"));
        assert!(output.text.contains("## parse"));
        assert!(output.text.contains("## summarize"));
        // parse and summarize should have upstream context
        assert!(output.text.contains("with upstream"));
    }

    #[tokio::test]
    async fn swarm_concurrent_roots() {
        let engine = SwarmEngine::new()
            .add_node(SwarmNode::new("a", make_echo("a")))
            .add_node(SwarmNode::new("b", make_echo("b")))
            .add_node(
                SwarmNode::new("merge", make_echo("merge"))
                    .depends_on("a")
                    .depends_on("b"),
            );

        let task = AgentTask::new("merge data");
        let output = engine.run(task).await.unwrap();

        assert!(output.approved);
        assert!(output.text.contains("## a"));
        assert!(output.text.contains("## b"));
        assert!(output.text.contains("## merge"));
    }

    #[test]
    fn swarm_role_is_orchestrator() {
        let engine = SwarmEngine::new();
        assert!(matches!(engine.role(), AgentRole::Orchestrator));
    }
}
