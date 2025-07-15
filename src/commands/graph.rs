use anyhow::Result;
use clap::ValueEnum;
use colored::*;
use petgraph::visit::EdgeRef;

use cigen::graph::DependencyGraph;
use cigen::loader::ConfigLoader;

#[derive(Debug, Clone, ValueEnum)]
pub enum GraphFormat {
    /// Display as a tree (default)
    Tree,
    /// Display as a sorted list
    List,
    /// Display as DOT format for graphviz
    Dot,
}

pub fn graph_command(
    config_path: &str,
    workflow_filter: Option<String>,
    format: GraphFormat,
) -> Result<()> {
    // Load all configuration
    let loader = ConfigLoader::new(config_path)?;
    let loaded = loader.load_all()?;

    // Filter jobs by workflow if specified
    let filtered_jobs = if let Some(workflow) = &workflow_filter {
        loaded
            .jobs
            .into_iter()
            .filter(|(name, _)| name.starts_with(&format!("{workflow}/")))
            .collect()
    } else {
        loaded.jobs
    };

    if filtered_jobs.is_empty() {
        if workflow_filter.is_some() {
            println!("No jobs found in the specified workflow.");
        } else {
            println!("No jobs found.");
        }
        return Ok(());
    }

    // Create dependency graph
    let graph = DependencyGraph::new(&filtered_jobs)?;

    match format {
        GraphFormat::Tree => display_as_tree(&graph, &filtered_jobs),
        GraphFormat::List => display_as_list(&graph)?,
        GraphFormat::Dot => display_as_dot(&graph),
    }

    Ok(())
}

fn display_as_tree(
    graph: &DependencyGraph,
    jobs: &std::collections::HashMap<String, cigen::models::Job>,
) {
    // Group jobs by workflow
    let mut workflows: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for job_name in jobs.keys() {
        let workflow = job_name.split('/').next().unwrap_or("unknown");
        workflows
            .entry(workflow.to_string())
            .or_default()
            .push(job_name.clone());
    }

    // Sort workflows
    let mut workflow_names: Vec<_> = workflows.keys().cloned().collect();
    workflow_names.sort();

    println!("{}", "Job Dependency Graph".bold().cyan());
    println!();

    for (idx, workflow) in workflow_names.iter().enumerate() {
        if idx > 0 {
            println!(); // Empty line between workflows
        }

        println!("{} {}", "▶".yellow(), workflow.bold().yellow());

        let workflow_jobs = &workflows[workflow];

        // Find leaf nodes (jobs with no dependents) in this workflow
        let mut leaves = Vec::new();
        for job_name in workflow_jobs {
            if graph.get_dependents(job_name).is_empty() {
                leaves.push(job_name.clone());
            }
        }

        // If no leaves, take all jobs (handles cycles or all interdependent)
        if leaves.is_empty() {
            leaves = workflow_jobs.clone();
        }

        leaves.sort();

        // Display each leaf and its dependencies
        let mut displayed = std::collections::HashSet::new();
        for (i, leaf) in leaves.iter().enumerate() {
            if !displayed.contains(leaf) {
                if i > 0 {
                    println!(); // Line between job trees
                }
                display_inverted_tree(leaf, graph, &mut displayed, 0, false, workflow_jobs);
            }
        }

        println!(); // Extra line after each workflow
    }
}

fn display_inverted_tree(
    job_name: &str,
    graph: &DependencyGraph,
    displayed: &mut std::collections::HashSet<String>,
    depth: usize,
    is_last: bool,
    workflow_jobs: &[String],
) {
    // Extract just the job name (without workflow prefix) for display
    let display_name = job_name.split('/').nth(1).unwrap_or(job_name);

    if depth == 0 {
        println!("  {display_name}");
    } else {
        let prefix = if depth == 1 {
            if is_last {
                "└── ".to_string()
            } else {
                "├── ".to_string()
            }
        } else {
            let mut p = String::new();
            for i in 1..depth {
                p.push_str(if i == depth - 1 {
                    if is_last { "    " } else { "│   " }
                } else {
                    "│   "
                });
            }
            p.push_str(if is_last { "└── " } else { "├── " });
            p
        };

        if displayed.contains(job_name) {
            println!(
                "  {}{}",
                prefix.bright_black(),
                format!("{display_name} (...)").bright_black()
            );
            return;
        }

        println!("  {}{}", prefix.bright_black(), display_name.white());
    }

    displayed.insert(job_name.to_string());

    // Get dependencies of this job (inverted view)
    let mut dependencies = graph.get_dependencies(job_name);

    // Only show dependencies within the same workflow
    dependencies.retain(|dep| workflow_jobs.contains(dep));
    dependencies.sort();

    for (i, dep) in dependencies.iter().enumerate() {
        let is_last_dep = i == dependencies.len() - 1;
        display_inverted_tree(dep, graph, displayed, depth + 1, is_last_dep, workflow_jobs);
    }
}

fn display_as_list(graph: &DependencyGraph) -> Result<()> {
    println!("Jobs in dependency order:");
    println!("========================");

    let sorted = graph.topological_sort()?;
    for (i, job) in sorted.iter().enumerate() {
        println!("{:3}. {}", i + 1, job);
    }

    Ok(())
}

fn display_as_dot(graph: &DependencyGraph) {
    println!("digraph JobDependencies {{");
    println!("  rankdir=LR;");
    println!("  node [shape=box];");

    // Get the internal graph and iterate over edges
    let internal_graph = graph.graph();
    for edge in internal_graph.edge_references() {
        let source = &internal_graph[edge.source()];
        let target = &internal_graph[edge.target()];
        println!("  \"{source}\" -> \"{target}\";");
    }

    println!("}}");
}
