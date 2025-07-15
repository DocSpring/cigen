use anyhow::Result;
use petgraph::visit::EdgeRef;
use std::io::Write;
use std::process::{Command, Stdio};
use which::which;

use cigen::graph::DependencyGraph;
use cigen::loader::ConfigLoader;

pub fn graph_command(
    config_path: &str,
    workflow_filter: Option<String>,
    output_path: Option<String>,
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

    let dot_output = generate_dot(&graph);
    if let Some(path) = output_path {
        if path.ends_with(".dot") {
            std::fs::write(&path, &dot_output)?;
            println!("Graph saved to: {path}");
        } else {
            render_dot_to_png(&dot_output, &path)?;
        }
    } else if can_display_images() {
        display_dot_as_image(&dot_output)?;
    } else {
        println!("{dot_output}");
    }

    Ok(())
}

fn generate_dot(graph: &DependencyGraph) -> String {
    let mut output = String::new();
    output.push_str("digraph JobDependencies {\n");
    output.push_str("  bgcolor=transparent;\n");
    output.push_str("  rankdir=LR;\n");
    output.push_str("  dpi=120;\n");
    output.push_str("  size=\"15,10\";\n");
    output.push_str("  ratio=fill;\n");
    output.push_str("  nodesep=1.2;\n");
    output.push_str("  ranksep=1.8;\n");
    output.push_str(
        "  node [shape=box, style=\"rounded,filled\", fontname=\"Arial\", fontsize=20, color=white, fontcolor=white, fillcolor=transparent, penwidth=2, margin=50];\n",
    );
    output.push_str(
        "  edge [fontname=\"Arial\", fontsize=18, color=white, fontcolor=white, penwidth=2, minlen=2];\n",
    );
    output.push('\n');

    // Group by workflow
    let internal_graph = graph.graph();
    let mut workflows = std::collections::HashMap::new();

    for node_idx in internal_graph.node_indices() {
        let job_name = &internal_graph[node_idx];
        let workflow = job_name.split('/').next().unwrap_or("unknown");
        workflows
            .entry(workflow.to_string())
            .or_insert_with(Vec::new)
            .push(job_name.clone());
    }

    // Create subgraphs for each workflow
    for (workflow, jobs) in workflows {
        output.push_str(&format!("  subgraph cluster_{workflow} {{\n"));
        output.push_str(&format!("    label=<<B>{workflow}</B>>;\n"));
        output.push_str("    style=\"filled,rounded\";\n");
        output.push_str("    color=white;\n");
        output.push_str("    fontcolor=white;\n");
        output.push_str("    fontname=\"Arial\";\n");
        output.push_str("    fontsize=22;\n");
        output.push_str("    fillcolor=transparent;\n");
        output.push_str("    penwidth=2;\n");
        output.push_str("    margin=60;\n");
        output.push_str("    node [margin=0.4,style=filled,color=white,fontcolor=white,fillcolor=transparent,penwidth=2];\n");

        for job in jobs {
            let short_name = job.split('/').nth(1).unwrap_or(&job);
            output.push_str(&format!("    \"{job}\" [label=\"{short_name}\"];\n"));
        }

        output.push_str("  }\n\n");
    }

    // Add edges
    for edge in internal_graph.edge_references() {
        let source = &internal_graph[edge.source()];
        let target = &internal_graph[edge.target()];
        output.push_str(&format!("  \"{source}\" -> \"{target}\";\n"));
    }

    output.push_str("}\n");
    output
}

fn can_display_images() -> bool {
    // Check for iTerm2
    if std::env::var("TERM_PROGRAM")
        .map(|v| v == "iTerm.app")
        .unwrap_or(false)
    {
        return true;
    }

    // Check for Kitty
    if std::env::var("TERM")
        .map(|v| v.contains("kitty"))
        .unwrap_or(false)
    {
        return true;
    }

    false
}

fn check_graphviz() -> Result<()> {
    if which("dot").is_err() {
        eprintln!("\nGraphviz is not installed. To render graphs as images, please install it:\n");

        #[cfg(target_os = "macos")]
        eprintln!("  brew install graphviz");

        #[cfg(target_os = "linux")]
        {
            eprintln!("  # Ubuntu/Debian:");
            eprintln!("  sudo apt-get install graphviz");
            eprintln!("\n  # Fedora/RHEL:");
            eprintln!("  sudo dnf install graphviz");
            eprintln!("\n  # Arch:");
            eprintln!("  sudo pacman -S graphviz");
        }

        #[cfg(target_os = "windows")]
        eprintln!("  Download from: https://graphviz.org/download/");

        anyhow::bail!("Graphviz not found");
    }
    Ok(())
}

fn render_dot_to_png(dot_content: &str, output_path: &str) -> Result<()> {
    check_graphviz()?;

    let mut child = Command::new("dot")
        .args(["-Tpng", "-o", output_path])
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(dot_content.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("Failed to render graph with Graphviz");
    }

    println!("Graph saved to: {output_path}");
    Ok(())
}

fn display_dot_as_image(dot_content: &str) -> Result<()> {
    check_graphviz()?;

    // Create temporary file
    let temp_path = std::env::temp_dir().join("cigen_graph.png");
    let temp_path_str = temp_path.to_string_lossy();

    // Render to PNG
    render_dot_to_png(dot_content, &temp_path_str)?;

    // Display in terminal
    if std::env::var("TERM_PROGRAM")
        .map(|v| v == "iTerm.app")
        .unwrap_or(false)
    {
        // iTerm2 inline images protocol
        display_iterm2_image(&temp_path)?;
    } else if std::env::var("TERM")
        .map(|v| v.contains("kitty"))
        .unwrap_or(false)
    {
        // Kitty graphics protocol
        Command::new("kitty")
            .args(["+kitten", "icat", &temp_path_str])
            .status()?;
    }

    // Clean up
    std::fs::remove_file(&temp_path).ok();

    Ok(())
}

fn display_iterm2_image(path: &std::path::Path) -> Result<()> {
    use base64::{Engine as _, engine::general_purpose};

    let image_data = std::fs::read(path)?;
    let base64_data = general_purpose::STANDARD.encode(&image_data);

    // iTerm2 inline images protocol
    print!("\x1b]1337;File=inline=1:{base64_data};\x07");
    println!(); // New line after image

    Ok(())
}
