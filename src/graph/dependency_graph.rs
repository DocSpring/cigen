use anyhow::{Result, bail};
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

use crate::models::Job;

/// Manages job dependencies and provides topological sorting
pub struct DependencyGraph {
    /// The directed graph of job dependencies
    graph: DiGraph<String, ()>,
    /// Map from job name to graph node index
    node_map: HashMap<String, NodeIndex>,
}

impl DependencyGraph {
    /// Create a new dependency graph from jobs
    pub fn new(jobs: &HashMap<String, Job>) -> Result<Self> {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();

        // First pass: create all nodes
        for job_name in jobs.keys() {
            let node = graph.add_node(job_name.clone());
            node_map.insert(job_name.clone(), node);
        }

        // Second pass: add edges based on explicit requires
        for (job_name, job) in jobs {
            if let Some(requires) = &job.requires {
                let dependent_node = node_map[job_name];

                for required_job in requires.to_vec() {
                    // Handle both full paths (workflow/job) and relative job names
                    let full_job_name = if required_job.contains('/') {
                        required_job
                    } else {
                        // Same workflow - extract workflow from current job name
                        let workflow = job_name.split('/').next().unwrap_or("");
                        format!("{workflow}/{required_job}")
                    };

                    // Check if the required job exists
                    if let Some(&required_node) = node_map.get(&full_job_name) {
                        // Add edge: required_job -> dependent_job
                        graph.add_edge(required_node, dependent_node, ());
                    } else {
                        bail!(
                            "Job '{}' requires '{}', but that job doesn't exist",
                            job_name,
                            full_job_name
                        );
                    }
                }
            }
        }

        let dep_graph = Self { graph, node_map };

        // Check for cycles
        if dep_graph.has_cycles() {
            bail!("Circular dependencies detected in job graph");
        }

        Ok(dep_graph)
    }

    /// Check if the graph has any cycles
    pub fn has_cycles(&self) -> bool {
        is_cyclic_directed(&self.graph)
    }

    /// Get a topologically sorted list of jobs
    pub fn topological_sort(&self) -> Result<Vec<String>> {
        match toposort(&self.graph, None) {
            Ok(sorted_nodes) => {
                let sorted_jobs = sorted_nodes
                    .into_iter()
                    .map(|node| self.graph[node].clone())
                    .collect();
                Ok(sorted_jobs)
            }
            Err(_) => bail!("Cannot perform topological sort: graph contains cycles"),
        }
    }

    /// Get all jobs that directly depend on the given job
    pub fn get_dependents(&self, job_name: &str) -> Vec<String> {
        if let Some(&node) = self.node_map.get(job_name) {
            self.graph
                .neighbors(node)
                .map(|n| self.graph[n].clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all jobs that the given job depends on (its requirements)
    pub fn get_dependencies(&self, job_name: &str) -> Vec<String> {
        if let Some(&node) = self.node_map.get(job_name) {
            self.graph
                .neighbors_directed(node, petgraph::Direction::Incoming)
                .map(|n| self.graph[n].clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all jobs that transitively depend on the given job
    pub fn get_all_dependents(&self, job_name: &str) -> Vec<String> {
        let mut visited = HashMap::new();
        let mut result = Vec::new();

        if let Some(&start_node) = self.node_map.get(job_name) {
            self.visit_dependents(start_node, &mut visited, &mut result);
        }

        result
    }

    fn visit_dependents(
        &self,
        node: NodeIndex,
        visited: &mut HashMap<NodeIndex, bool>,
        result: &mut Vec<String>,
    ) {
        if visited.get(&node).copied().unwrap_or(false) {
            return;
        }

        visited.insert(node, true);

        for neighbor in self.graph.neighbors(node) {
            self.visit_dependents(neighbor, visited, result);
            result.push(self.graph[neighbor].clone());
        }
    }

    /// Find cycles in the graph and return them as a list of job paths
    pub fn find_cycles(&self) -> Vec<Vec<String>> {
        // This is a simplified cycle detection that returns basic cycle information
        // For more detailed cycle paths, we'd need to implement a proper DFS-based algorithm
        let mut cycles = Vec::new();

        // Use strongly connected components to find cycles
        let sccs = petgraph::algo::kosaraju_scc(&self.graph);

        for scc in sccs {
            if scc.len() > 1 {
                // This is a cycle
                let cycle: Vec<String> = scc
                    .into_iter()
                    .map(|node| self.graph[node].clone())
                    .collect();
                cycles.push(cycle);
            } else if scc.len() == 1 {
                // Check for self-loop
                let node = scc[0];
                if self.graph.contains_edge(node, node) {
                    cycles.push(vec![self.graph[node].clone()]);
                }
            }
        }

        cycles
    }

    /// Get the graph for visualization
    pub fn graph(&self) -> &DiGraph<String, ()> {
        &self.graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::job::JobRequires;

    fn create_test_job(requires: Option<Vec<&str>>) -> Job {
        Job {
            image: "test".to_string(),
            architectures: None,
            resource_class: None,
            source_files: None,
            parallelism: None,
            requires: requires.map(|deps| {
                if deps.len() == 1 {
                    JobRequires::Single(deps[0].to_string())
                } else {
                    JobRequires::Multiple(deps.into_iter().map(|s| s.to_string()).collect())
                }
            }),
            cache: None,
            restore_cache: None,
            services: None,
            steps: None,
        }
    }

    #[test]
    fn test_simple_dependency_chain() {
        let mut jobs = HashMap::new();
        jobs.insert("test/a".to_string(), create_test_job(None));
        jobs.insert("test/b".to_string(), create_test_job(Some(vec!["a"])));
        jobs.insert("test/c".to_string(), create_test_job(Some(vec!["b"])));

        let graph = DependencyGraph::new(&jobs).unwrap();
        assert!(!graph.has_cycles());

        let sorted = graph.topological_sort().unwrap();
        // a should come before b, b before c
        let a_pos = sorted.iter().position(|j| j == "test/a").unwrap();
        let b_pos = sorted.iter().position(|j| j == "test/b").unwrap();
        let c_pos = sorted.iter().position(|j| j == "test/c").unwrap();
        assert!(a_pos < b_pos);
        assert!(b_pos < c_pos);
    }

    #[test]
    fn test_circular_dependency() {
        let mut jobs = HashMap::new();
        jobs.insert("test/a".to_string(), create_test_job(Some(vec!["b"])));
        jobs.insert("test/b".to_string(), create_test_job(Some(vec!["c"])));
        jobs.insert("test/c".to_string(), create_test_job(Some(vec!["a"])));

        match DependencyGraph::new(&jobs) {
            Ok(_) => panic!("Expected error for circular dependency"),
            Err(e) => assert!(e.to_string().contains("Circular dependencies")),
        }
    }

    #[test]
    fn test_multiple_dependencies() {
        let mut jobs = HashMap::new();
        jobs.insert("test/setup".to_string(), create_test_job(None));
        jobs.insert(
            "test/install_gems".to_string(),
            create_test_job(Some(vec!["setup"])),
        );
        jobs.insert(
            "test/install_npm".to_string(),
            create_test_job(Some(vec!["setup"])),
        );
        jobs.insert(
            "test/test".to_string(),
            create_test_job(Some(vec!["install_gems", "install_npm"])),
        );

        let graph = DependencyGraph::new(&jobs).unwrap();
        assert!(!graph.has_cycles());

        let deps = graph.get_dependencies("test/test");
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"test/install_gems".to_string()));
        assert!(deps.contains(&"test/install_npm".to_string()));
    }
}
