// Phase 2.5: Service Dependency Management
//
// This module implements a dependency graph for managing task dependencies
// with support for hard dependencies (Requires), soft ordering (After),
// and mutual exclusion (Conflicts).

use anyhow::{Result, anyhow};
use petgraph::Direction;
use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};

/// Dependency type defines the relationship between tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyType {
    /// Hard dependency: dependent task MUST start after this task is running
    /// (e.g., database service must be running before web server starts)
    Requires,

    /// Soft ordering preference: start after this task if it exists
    /// (e.g., logging service should start before app, but app can run without it)
    After,

    /// Mutual exclusion: tasks cannot run together
    /// (e.g., production and development environments)
    Conflicts,
}

/// Dependency graph for managing task dependencies
pub struct DependencyGraph {
    /// Directed graph: nodes are task names, edges are dependencies
    /// Edge direction: A -> B means "A depends on B" (B must start before A)
    graph: DiGraph<String, DependencyType>,

    /// Quick lookup: task name -> node index
    node_map: HashMap<String, NodeIndex>,

    /// Conflict pairs: (task_a, task_b) where tasks cannot run together
    /// Stored separately because conflicts are bidirectional
    conflicts: HashSet<(String, String)>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
            conflicts: HashSet::new(),
        }
    }

    /// Add a task node to the graph (idempotent)
    ///
    /// # Arguments
    /// * `task_name` - Unique task identifier
    ///
    /// # Returns
    /// NodeIndex for the task (existing or newly created)
    pub fn add_node(&mut self, task_name: String) -> NodeIndex {
        if let Some(&idx) = self.node_map.get(&task_name) {
            idx
        } else {
            let idx = self.graph.add_node(task_name.clone());
            self.node_map.insert(task_name, idx);
            idx
        }
    }

    /// Add a dependency edge between tasks
    ///
    /// # Arguments
    /// * `from` - Dependent task (will wait for `to`)
    /// * `to` - Dependency target (must start before `from`)
    /// * `dep_type` - Type of dependency relationship
    ///
    /// # Returns
    /// * `Ok(())` - Edge added successfully
    /// * `Err(_)` - Task not found or invalid dependency
    ///
    /// # Example
    /// ```
    /// // "webapp" requires "database" to be running first
    /// graph.add_edge("webapp", "database", DependencyType::Requires)?;
    /// ```
    pub fn add_edge(&mut self, from: &str, to: &str, dep_type: DependencyType) -> Result<()> {
        let from_idx = self
            .node_map
            .get(from)
            .ok_or_else(|| anyhow!("Task '{}' not found in graph", from))?;
        let to_idx = self
            .node_map
            .get(to)
            .ok_or_else(|| anyhow!("Task '{}' not found in graph", to))?;

        // Handle conflicts separately (bidirectional)
        if dep_type == DependencyType::Conflicts {
            self.add_conflict(from, to);
            return Ok(());
        }

        // Add directed edge: to -> from (dependency -> dependent)
        // This ensures topological sort places dependencies before dependents
        self.graph.add_edge(*to_idx, *from_idx, dep_type);
        Ok(())
    }

    /// Add a mutual exclusion relationship (internal helper)
    fn add_conflict(&mut self, task_a: &str, task_b: &str) {
        // Store both orderings to simplify lookup
        let pair1 = (task_a.to_string(), task_b.to_string());
        let pair2 = (task_b.to_string(), task_a.to_string());
        self.conflicts.insert(pair1);
        self.conflicts.insert(pair2);
    }

    /// Perform topological sort to determine task startup order
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - Task names in valid startup order
    /// * `Err(_)` - Circular dependency detected
    ///
    /// # Algorithm
    /// Uses Tarjan's strongly connected components (SCC) algorithm:
    /// - Each SCC with >1 node indicates a cycle
    /// - SCCs are already in reverse topological order
    ///
    /// # Notes
    /// - Tasks with no dependencies can start in any order (appear first)
    /// - Tasks with dependencies start after their dependencies
    /// - Only considers Requires/After edges (not Conflicts)
    pub fn topological_sort(&self) -> Result<Vec<String>> {
        // Use Tarjan's SCC algorithm (returns SCCs in reverse topological order)
        let sccs = tarjan_scc(&self.graph);

        // Check for cycles: any SCC with >1 node is a cycle
        for scc in &sccs {
            if scc.len() > 1 {
                let cycle_tasks: Vec<String> =
                    scc.iter().map(|&idx| self.graph[idx].clone()).collect();
                return Err(anyhow!(
                    "Circular dependency detected: {}",
                    cycle_tasks.join(" -> ")
                ));
            }
        }

        // SCCs are in reverse topological order, so reverse again
        // (tasks with no dependencies come first)
        let sorted: Vec<String> = sccs
            .into_iter()
            .rev()
            .flat_map(|scc| scc.into_iter().map(|idx| self.graph[idx].clone()))
            .collect();

        Ok(sorted)
    }

    /// Detect circular dependencies (explicit cycle detection)
    ///
    /// # Returns
    /// * `Some(Vec<String>)` - Task names forming a cycle
    /// * `None` - No cycles detected
    ///
    /// # Notes
    /// This is a wrapper around topological_sort for explicit cycle checking.
    /// Prefer using topological_sort directly in production code.
    pub fn detect_cycles(&self) -> Option<Vec<String>> {
        match self.topological_sort() {
            Ok(_) => None,
            Err(e) => {
                // Extract cycle from error message
                let msg = e.to_string();
                if msg.contains("Circular dependency detected:") {
                    // Parse cycle from error message
                    let cycle_str = msg.split(": ").nth(1)?;
                    let cycle: Vec<String> =
                        cycle_str.split(" -> ").map(|s| s.to_string()).collect();
                    Some(cycle)
                } else {
                    None
                }
            }
        }
    }

    /// Check if a task conflicts with any currently running tasks
    ///
    /// # Arguments
    /// * `task` - Task name to check
    /// * `running_tasks` - Set of currently running task names
    ///
    /// # Returns
    /// * `Ok(())` - No conflicts detected, safe to start
    /// * `Err(_)` - Conflict detected with running task
    ///
    /// # Example
    /// ```
    /// let running = ["prod-server"].iter().map(|s| s.to_string()).collect();
    /// graph.check_conflicts("dev-server", &running)?; // May fail if they conflict
    /// ```
    pub fn check_conflicts(&self, task: &str, running_tasks: &HashSet<String>) -> Result<()> {
        for running_task in running_tasks {
            let pair = (task.to_string(), running_task.clone());
            if self.conflicts.contains(&pair) {
                return Err(anyhow!(
                    "Task '{}' conflicts with running task '{}'",
                    task,
                    running_task
                ));
            }
        }
        Ok(())
    }

    /// Get all tasks that the given task depends on (Requires only)
    ///
    /// # Arguments
    /// * `task` - Task name
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - Names of required dependencies
    /// * `Err(_)` - Task not found
    ///
    /// # Notes
    /// Only returns hard dependencies (Requires), not soft ordering (After)
    pub fn get_required_dependencies(&self, task: &str) -> Result<Vec<String>> {
        let idx = self
            .node_map
            .get(task)
            .ok_or_else(|| anyhow!("Task '{}' not found", task))?;

        // Edge direction: to -> from (dependency -> dependent)
        // To get dependencies of 'task', we need incoming edges (edges pointing TO task)
        let deps: Vec<String> = self
            .graph
            .edges_directed(*idx, Direction::Incoming)
            .filter(|edge| *edge.weight() == DependencyType::Requires)
            .map(|edge| self.graph[edge.source()].clone())
            .collect();

        Ok(deps)
    }

    /// Check if all required dependencies are satisfied (running)
    ///
    /// # Arguments
    /// * `task` - Task name to check
    /// * `running_tasks` - Set of currently running task names
    ///
    /// # Returns
    /// * `Ok(())` - All required dependencies are running
    /// * `Err(_)` - Missing required dependency
    pub fn check_dependencies_satisfied(
        &self,
        task: &str,
        running_tasks: &HashSet<String>,
    ) -> Result<()> {
        let required = self.get_required_dependencies(task)?;

        for dep in required {
            if !running_tasks.contains(&dep) {
                return Err(anyhow!("Task '{}' requires '{}' to be running", task, dep));
            }
        }

        Ok(())
    }

    /// Get all tasks that depend on the given task (reverse lookup)
    ///
    /// # Arguments
    /// * `task` - Task name
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - Names of tasks that depend on this task
    /// * `Err(_)` - Task not found
    ///
    /// # Notes
    /// Useful for cascading shutdowns (if A depends on B, stopping B should stop A)
    pub fn get_dependents(&self, task: &str) -> Result<Vec<String>> {
        let idx = self
            .node_map
            .get(task)
            .ok_or_else(|| anyhow!("Task '{}' not found", task))?;

        // Edge direction: to -> from (dependency -> dependent)
        // To get dependents of 'task', we need outgoing edges (edges FROM task)
        let dependents: Vec<String> = self
            .graph
            .edges_directed(*idx, Direction::Outgoing)
            .map(|edge| self.graph[edge.target()].clone())
            .collect();

        Ok(dependents)
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_dependency() {
        let mut graph = DependencyGraph::new();
        graph.add_node("webapp".to_string());
        graph.add_node("database".to_string());
        graph
            .add_edge("webapp", "database", DependencyType::Requires)
            .unwrap();

        // Topological sort: database should come before webapp
        let sorted = graph.topological_sort().unwrap();
        assert_eq!(sorted, vec!["database", "webapp"]);
    }

    #[test]
    fn test_circular_dependency() {
        let mut graph = DependencyGraph::new();
        graph.add_node("a".to_string());
        graph.add_node("b".to_string());
        graph.add_node("c".to_string());

        // Create cycle: a -> b -> c -> a
        graph.add_edge("a", "b", DependencyType::Requires).unwrap();
        graph.add_edge("b", "c", DependencyType::Requires).unwrap();
        graph.add_edge("c", "a", DependencyType::Requires).unwrap();

        // Should detect cycle
        assert!(graph.topological_sort().is_err());
        let cycle = graph.detect_cycles();
        assert!(cycle.is_some());
    }

    #[test]
    fn test_conflict_detection() {
        let mut graph = DependencyGraph::new();
        graph.add_node("prod".to_string());
        graph.add_node("dev".to_string());
        graph
            .add_edge("prod", "dev", DependencyType::Conflicts)
            .unwrap();

        // Check conflict with running task
        let mut running = HashSet::new();
        running.insert("prod".to_string());

        assert!(graph.check_conflicts("dev", &running).is_err());

        // No conflict with empty set
        let empty = HashSet::new();
        assert!(graph.check_conflicts("dev", &empty).is_ok());
    }

    #[test]
    fn test_complex_dependency_chain() {
        let mut graph = DependencyGraph::new();

        // Add nodes
        for name in &["app", "api", "db", "cache", "logs"] {
            graph.add_node(name.to_string());
        }

        // Add dependencies: app -> api -> db
        //                    app -> cache
        //                    api -> logs
        graph
            .add_edge("app", "api", DependencyType::Requires)
            .unwrap();
        graph
            .add_edge("api", "db", DependencyType::Requires)
            .unwrap();
        graph
            .add_edge("app", "cache", DependencyType::Requires)
            .unwrap();
        graph
            .add_edge("api", "logs", DependencyType::After)
            .unwrap();

        // Topological sort should put db/cache/logs first, then api, then app
        let sorted = graph.topological_sort().unwrap();

        // Check ordering constraints
        let app_pos = sorted.iter().position(|s| s == "app").unwrap();
        let api_pos = sorted.iter().position(|s| s == "api").unwrap();
        let db_pos = sorted.iter().position(|s| s == "db").unwrap();
        let cache_pos = sorted.iter().position(|s| s == "cache").unwrap();

        assert!(db_pos < api_pos, "db must come before api");
        assert!(api_pos < app_pos, "api must come before app");
        assert!(cache_pos < app_pos, "cache must come before app");
    }

    #[test]
    fn test_required_dependencies() {
        let mut graph = DependencyGraph::new();
        graph.add_node("webapp".to_string());
        graph.add_node("database".to_string());
        graph.add_node("cache".to_string());

        graph
            .add_edge("webapp", "database", DependencyType::Requires)
            .unwrap();
        graph
            .add_edge("webapp", "cache", DependencyType::After)
            .unwrap();

        // Should only return Requires dependencies
        let deps = graph.get_required_dependencies("webapp").unwrap();
        assert_eq!(deps.len(), 1);
        assert!(deps.contains(&"database".to_string()));
    }

    #[test]
    fn test_dependencies_satisfied() {
        let mut graph = DependencyGraph::new();
        graph.add_node("webapp".to_string());
        graph.add_node("database".to_string());
        graph
            .add_edge("webapp", "database", DependencyType::Requires)
            .unwrap();

        // Dependency not satisfied
        let empty = HashSet::new();
        assert!(
            graph
                .check_dependencies_satisfied("webapp", &empty)
                .is_err()
        );

        // Dependency satisfied
        let mut running = HashSet::new();
        running.insert("database".to_string());
        assert!(
            graph
                .check_dependencies_satisfied("webapp", &running)
                .is_ok()
        );
    }

    #[test]
    fn test_get_dependents() {
        let mut graph = DependencyGraph::new();
        graph.add_node("database".to_string());
        graph.add_node("webapp".to_string());
        graph.add_node("api".to_string());

        graph
            .add_edge("webapp", "database", DependencyType::Requires)
            .unwrap();
        graph
            .add_edge("api", "database", DependencyType::Requires)
            .unwrap();

        // Both webapp and api depend on database
        let dependents = graph.get_dependents("database").unwrap();
        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&"webapp".to_string()));
        assert!(dependents.contains(&"api".to_string()));
    }
}
