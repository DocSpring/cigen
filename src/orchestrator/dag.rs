use anyhow::{Result, bail};
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

use crate::schema::{CigenConfig, Job, MatrixDimension};

/// A concrete job instance after matrix expansion
#[derive(Debug, Clone, PartialEq)]
pub struct ConcreteJob {
    /// Original job ID from cigen.yml
    pub job_id: String,
    /// Unique instance ID (job_id for non-matrix, job_id-value1-value2 for matrix)
    pub instance_id: String,
    /// Matrix variable values for this instance (empty for non-matrix jobs)
    pub matrix_values: HashMap<String, String>,
    /// The job definition
    pub job: Job,
}

/// DAG builder and manager for cigen jobs
#[derive(Debug)]
pub struct JobDAG {
    /// The directed graph of job dependencies
    graph: DiGraph<String, ()>,
    /// Map from instance ID to graph node index
    node_map: HashMap<String, NodeIndex>,
    /// Map from instance ID to concrete job
    jobs: HashMap<String, ConcreteJob>,
}

impl JobDAG {
    /// Build a DAG from cigen configuration
    pub fn build(config: &CigenConfig) -> Result<Self> {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();
        let mut jobs = HashMap::new();

        // First pass: expand matrix jobs into concrete instances
        for (job_id, job) in &config.jobs {
            let instances = expand_matrix_job(job_id, job)?;

            for instance in instances {
                let instance_id = instance.instance_id.clone();
                let node = graph.add_node(instance_id.clone());
                node_map.insert(instance_id.clone(), node);
                jobs.insert(instance_id, instance);
            }
        }

        // Second pass: add dependency edges
        for (instance_id, concrete_job) in &jobs {
            let dependent_node = node_map[instance_id];

            for needed_job_id in &concrete_job.job.needs {
                // Find all instances of the needed job
                let needed_instances: Vec<_> = jobs
                    .iter()
                    .filter(|(_, job)| job.job_id == *needed_job_id)
                    .collect();

                if needed_instances.is_empty() {
                    bail!(
                        "Job '{instance_id}' depends on '{needed_job_id}', but that job doesn't exist"
                    );
                }

                // Add edge from each instance of the needed job to this job
                for (needed_instance_id, _) in needed_instances {
                    let needed_node = node_map[needed_instance_id];
                    // Add edge: needed_job -> dependent_job
                    graph.add_edge(needed_node, dependent_node, ());
                }
            }
        }

        let dag = Self {
            graph,
            node_map,
            jobs,
        };

        // Check for cycles
        if dag.has_cycles() {
            let cycles = dag.find_cycles();
            bail!("Circular dependencies detected in job graph: {:?}", cycles);
        }

        Ok(dag)
    }

    /// Check if the graph has any cycles
    pub fn has_cycles(&self) -> bool {
        is_cyclic_directed(&self.graph)
    }

    /// Get a topologically sorted list of job instances
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

    /// Get all concrete job instances
    pub fn jobs(&self) -> &HashMap<String, ConcreteJob> {
        &self.jobs
    }

    /// Get a specific concrete job by instance ID
    pub fn get_job(&self, instance_id: &str) -> Option<&ConcreteJob> {
        self.jobs.get(instance_id)
    }

    /// Get all jobs that directly depend on the given job instance
    pub fn get_dependents(&self, instance_id: &str) -> Vec<String> {
        if let Some(&node) = self.node_map.get(instance_id) {
            self.graph
                .neighbors(node)
                .map(|n| self.graph[n].clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all jobs that the given job instance depends on
    pub fn get_dependencies(&self, instance_id: &str) -> Vec<String> {
        if let Some(&node) = self.node_map.get(instance_id) {
            self.graph
                .neighbors_directed(node, petgraph::Direction::Incoming)
                .map(|n| self.graph[n].clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Find cycles in the graph
    pub fn find_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let sccs = petgraph::algo::kosaraju_scc(&self.graph);

        for scc in sccs {
            if scc.len() > 1 {
                let cycle: Vec<String> = scc
                    .into_iter()
                    .map(|node| self.graph[node].clone())
                    .collect();
                cycles.push(cycle);
            } else if scc.len() == 1 {
                let node = scc[0];
                if self.graph.contains_edge(node, node) {
                    cycles.push(vec![self.graph[node].clone()]);
                }
            }
        }

        cycles
    }

    /// Get the underlying graph for visualization
    pub fn graph(&self) -> &DiGraph<String, ()> {
        &self.graph
    }
}

/// Expand a job with matrix into multiple concrete instances
fn expand_matrix_job(job_id: &str, job: &Job) -> Result<Vec<ConcreteJob>> {
    if job.matrix.is_empty() {
        // No matrix - single instance
        return Ok(vec![ConcreteJob {
            job_id: job_id.to_string(),
            instance_id: job_id.to_string(),
            matrix_values: HashMap::new(),
            job: job.clone(),
        }]);
    }

    // Extract matrix dimensions and sort by key for consistent ordering
    let mut dimensions: Vec<(String, Vec<String>)> = Vec::new();
    for (key, value) in &job.matrix {
        match value {
            MatrixDimension::List(values) => {
                dimensions.push((key.clone(), values.clone()));
            }
        }
    }
    // Sort by key name for consistent instance ID generation
    dimensions.sort_by(|a, b| a.0.cmp(&b.0));

    // Generate cartesian product of all matrix dimensions
    let combinations = cartesian_product(&dimensions);

    // Create a concrete job instance for each combination
    let mut instances = Vec::new();
    for combination in combinations {
        let matrix_values: HashMap<String, String> = combination.clone().into_iter().collect();

        // Generate instance ID: job_id-value1-value2-...
        let instance_id = if combination.is_empty() {
            job_id.to_string()
        } else {
            let values: Vec<_> = combination.iter().map(|(_, v)| v.as_str()).collect();
            format!("{}-{}", job_id, values.join("-"))
        };

        instances.push(ConcreteJob {
            job_id: job_id.to_string(),
            instance_id,
            matrix_values,
            job: job.clone(),
        });
    }

    Ok(instances)
}

/// Generate cartesian product of matrix dimensions
fn cartesian_product(dimensions: &[(String, Vec<String>)]) -> Vec<Vec<(String, String)>> {
    if dimensions.is_empty() {
        return vec![vec![]];
    }

    let (key, values) = &dimensions[0];
    let rest = cartesian_product(&dimensions[1..]);

    let mut result = Vec::new();
    for value in values {
        for combo in &rest {
            let mut new_combo = vec![(key.clone(), value.clone())];
            new_combo.extend(combo.clone());
            result.push(new_combo);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_simple_job() -> Job {
        Job {
            needs: vec![],
            matrix: HashMap::new(),
            packages: vec![],
            services: vec![],
            environment: HashMap::new(),
            checkout: None,
            steps: vec![],
            source_files: vec![],
            skip_if: None,
            trigger: None,
            image: "ubuntu-latest".to_string(),
            runner: None,
            artifacts: vec![],
            extra: HashMap::new(),
            workflow: None,
        }
    }

    #[test]
    fn test_single_job_no_dependencies() {
        let mut jobs = HashMap::new();
        jobs.insert("test".to_string(), create_simple_job());

        let config = CigenConfig {
            project: None,
            providers: vec![],
            packages: vec![],
            source_file_groups: HashMap::new(),
            jobs,
            commands: HashMap::new(),
            caches: HashMap::new(),
            runners: HashMap::new(),
            provider_config: HashMap::new(),
            workflows: HashMap::new(),
            raw: Default::default(),
        };

        let dag = JobDAG::build(&config).unwrap();
        assert_eq!(dag.jobs().len(), 1);
        assert!(dag.jobs().contains_key("test"));
        assert!(!dag.has_cycles());
    }

    #[test]
    fn test_simple_dependency_chain() {
        let mut jobs = HashMap::new();
        jobs.insert("setup".to_string(), create_simple_job());

        let mut test = create_simple_job();
        test.needs = vec!["setup".to_string()];
        jobs.insert("test".to_string(), test);

        let mut deploy = create_simple_job();
        deploy.needs = vec!["test".to_string()];
        jobs.insert("deploy".to_string(), deploy);

        let config = CigenConfig {
            project: None,
            providers: vec![],
            packages: vec![],
            source_file_groups: HashMap::new(),
            jobs,
            commands: HashMap::new(),
            caches: HashMap::new(),
            runners: HashMap::new(),
            provider_config: HashMap::new(),
            workflows: HashMap::new(),
            raw: Default::default(),
        };

        let dag = JobDAG::build(&config).unwrap();
        assert!(!dag.has_cycles());

        let sorted = dag.topological_sort().unwrap();
        let setup_pos = sorted.iter().position(|j| j == "setup").unwrap();
        let test_pos = sorted.iter().position(|j| j == "test").unwrap();
        let deploy_pos = sorted.iter().position(|j| j == "deploy").unwrap();

        assert!(setup_pos < test_pos);
        assert!(test_pos < deploy_pos);
    }

    #[test]
    fn test_matrix_expansion() {
        let mut matrix = HashMap::new();
        matrix.insert(
            "ruby".to_string(),
            MatrixDimension::List(vec!["3.2".to_string(), "3.3".to_string()]),
        );
        matrix.insert(
            "arch".to_string(),
            MatrixDimension::List(vec!["amd64".to_string(), "arm64".to_string()]),
        );

        let mut job = create_simple_job();
        job.matrix = matrix;

        let instances = expand_matrix_job("test", &job).unwrap();
        assert_eq!(instances.len(), 4);

        // Check instance IDs are unique and follow pattern
        // Note: dimensions are sorted alphabetically (arch before ruby)
        let ids: Vec<_> = instances.iter().map(|i| &i.instance_id).collect();
        assert!(ids.contains(&&"test-amd64-3.2".to_string()));
        assert!(ids.contains(&&"test-amd64-3.3".to_string()));
        assert!(ids.contains(&&"test-arm64-3.2".to_string()));
        assert!(ids.contains(&&"test-arm64-3.3".to_string()));
    }

    #[test]
    fn test_matrix_job_dependencies() {
        let setup = create_simple_job();
        let mut matrix = HashMap::new();
        matrix.insert(
            "ruby".to_string(),
            MatrixDimension::List(vec!["3.2".to_string(), "3.3".to_string()]),
        );

        let mut test = create_simple_job();
        test.needs = vec!["setup".to_string()];
        test.matrix = matrix;

        let mut jobs = HashMap::new();
        jobs.insert("setup".to_string(), setup);
        jobs.insert("test".to_string(), test);

        let config = CigenConfig {
            project: None,
            providers: vec![],
            packages: vec![],
            source_file_groups: HashMap::new(),
            jobs,
            commands: HashMap::new(),
            caches: HashMap::new(),
            runners: HashMap::new(),
            provider_config: HashMap::new(),
            workflows: HashMap::new(),
            raw: Default::default(),
        };

        let dag = JobDAG::build(&config).unwrap();

        // Should have 3 instances: setup, test-3.2, test-3.3
        assert_eq!(dag.jobs().len(), 3);

        // Both test instances should depend on setup
        let test_32_deps = dag.get_dependencies("test-3.2");
        let test_33_deps = dag.get_dependencies("test-3.3");

        assert_eq!(test_32_deps, vec!["setup"]);
        assert_eq!(test_33_deps, vec!["setup"]);
    }

    #[test]
    fn test_circular_dependency() {
        let mut job_a = create_simple_job();
        job_a.needs = vec!["b".to_string()];

        let mut job_b = create_simple_job();
        job_b.needs = vec!["c".to_string()];

        let mut job_c = create_simple_job();
        job_c.needs = vec!["a".to_string()];

        let mut jobs = HashMap::new();
        jobs.insert("a".to_string(), job_a);
        jobs.insert("b".to_string(), job_b);
        jobs.insert("c".to_string(), job_c);

        let config = CigenConfig {
            project: None,
            providers: vec![],
            packages: vec![],
            source_file_groups: HashMap::new(),
            jobs,
            commands: HashMap::new(),
            caches: HashMap::new(),
            runners: HashMap::new(),
            provider_config: HashMap::new(),
            workflows: HashMap::new(),
            raw: Default::default(),
        };

        let result = JobDAG::build(&config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Circular dependencies")
        );
    }

    #[test]
    fn test_unknown_dependency() {
        let mut job = create_simple_job();
        job.needs = vec!["nonexistent".to_string()];

        let mut jobs = HashMap::new();
        jobs.insert("test".to_string(), job);

        let config = CigenConfig {
            project: None,
            providers: vec![],
            packages: vec![],
            source_file_groups: HashMap::new(),
            jobs,
            commands: HashMap::new(),
            caches: HashMap::new(),
            runners: HashMap::new(),
            provider_config: HashMap::new(),
            workflows: HashMap::new(),
            raw: Default::default(),
        };

        let result = JobDAG::build(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("doesn't exist"));
    }

    #[test]
    fn test_cartesian_product() {
        let dimensions = vec![
            (
                "ruby".to_string(),
                vec!["3.2".to_string(), "3.3".to_string()],
            ),
            (
                "arch".to_string(),
                vec!["amd64".to_string(), "arm64".to_string()],
            ),
        ];

        let result = cartesian_product(&dimensions);
        assert_eq!(result.len(), 4);

        // Check all combinations exist
        let contains_combo = |ruby: &str, arch: &str| {
            result.iter().any(|combo| {
                combo.contains(&("ruby".to_string(), ruby.to_string()))
                    && combo.contains(&("arch".to_string(), arch.to_string()))
            })
        };

        assert!(contains_combo("3.2", "amd64"));
        assert!(contains_combo("3.2", "arm64"));
        assert!(contains_combo("3.3", "amd64"));
        assert!(contains_combo("3.3", "arm64"));
    }
}
