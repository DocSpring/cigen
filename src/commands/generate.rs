use anyhow::{Context, Result};
use cigen::loader::ConfigLoader;
use cigen::providers::get_provider;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn generate_command(
    workflow: Option<String>,
    output: Option<String>,
    cli_vars: &HashMap<String, String>,
) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    println!(
        "Generating CI configuration from: {}",
        current_dir.display()
    );

    // Create loader with CLI variables
    let mut loader = ConfigLoader::new_with_vars(cli_vars)?;

    // Load all configuration
    let loaded_config = loader.load_all().context("Failed to load configuration")?;

    // Get the appropriate provider
    let provider = get_provider(&loaded_config.config.provider).map_err(|e| {
        anyhow::anyhow!(
            "Unsupported provider '{}': {}",
            loaded_config.config.provider,
            e
        )
    })?;

    // Determine output path
    let output_path = if let Some(output) = output {
        Path::new(&output).to_path_buf()
    } else {
        // Use config's output_path or default to provider's default
        if let Some(config_output) = &loaded_config.config.output_path {
            current_dir.join(config_output)
        } else {
            current_dir.join(provider.default_output_path())
        }
    };

    if let Some(workflow_name) = workflow {
        // Generate specific workflow
        println!("Generating workflow: {workflow_name}");

        // Filter jobs for this workflow
        let workflow_prefix = format!("{workflow_name}/");
        let workflow_jobs: HashMap<String, cigen::models::Job> = loaded_config
            .jobs
            .iter()
            .filter_map(|(path, job)| {
                if path.starts_with(&workflow_prefix) {
                    // Extract job name from path (e.g., "setup/generate_config" -> "generate_config")
                    let job_name = path.strip_prefix(&workflow_prefix)?.to_string();
                    Some((job_name, job.clone()))
                } else {
                    None
                }
            })
            .collect();

        if workflow_jobs.is_empty() {
            anyhow::bail!("No jobs found for workflow '{}'", workflow_name);
        }

        // Check if this workflow has its own config overrides
        let workflow_config_path = PathBuf::from(format!("workflows/{workflow_name}/config.yml"));
        let workflow_config = if workflow_config_path.exists() {
            // Load workflow-specific config
            let workflow_config_str = std::fs::read_to_string(&workflow_config_path)?;
            let workflow_config_value: serde_yaml::Value =
                serde_yaml::from_str(&workflow_config_str)?;

            // Merge with main config (workflow config takes precedence)
            let mut merged_config = loaded_config.config.clone();

            // Override specific fields if present in workflow config
            if let Some(obj) = workflow_config_value.as_mapping() {
                if let Some(output_path) = obj.get("output_path") {
                    if let Some(path_str) = output_path.as_str() {
                        merged_config.output_path = Some(path_str.to_string());
                    }
                }
                if let Some(output_filename) = obj.get("output_filename") {
                    if let Some(filename_str) = output_filename.as_str() {
                        merged_config.output_filename = Some(filename_str.to_string());
                    }
                }
                if let Some(dynamic) = obj.get("dynamic") {
                    if let Some(dynamic_bool) = dynamic.as_bool() {
                        merged_config.dynamic = Some(dynamic_bool);
                    }
                }
            }

            merged_config
        } else {
            loaded_config.config.clone()
        };

        // Determine workflow-specific output path
        let workflow_output_path = if let Some(workflow_output) = &workflow_config.output_path {
            current_dir.join(workflow_output)
        } else {
            output_path.clone()
        };

        provider
            .generate_workflow(
                &workflow_config,
                &workflow_name,
                &workflow_jobs,
                &workflow_output_path,
            )
            .map_err(|e| anyhow::anyhow!("Failed to generate workflow: {}", e))?;

        println!(
            "✅ Generated {} configuration for workflow '{}' to {}",
            provider.name(),
            workflow_name,
            workflow_output_path.display()
        );
    } else {
        // Generate all workflows
        println!("Generating all workflows");

        // Group jobs by workflow
        let mut workflows: HashMap<String, HashMap<String, cigen::models::Job>> = HashMap::new();

        for (path, job) in loaded_config.jobs {
            // Extract workflow name from path (e.g., "test/jobs/rspec" -> "test")
            if let Some(workflow_name) = path.split('/').next() {
                if let Some(job_name) = path.split('/').nth(2) {
                    workflows
                        .entry(workflow_name.to_string())
                        .or_default()
                        .insert(job_name.to_string(), job);
                }
            }
        }

        provider
            .generate_all(&loaded_config.config, &workflows, &output_path)
            .map_err(|e| anyhow::anyhow!("Failed to generate all workflows: {}", e))?;

        println!(
            "✅ Generated {} configuration for all workflows to {}",
            provider.name(),
            output_path.display()
        );
    }

    Ok(())
}
