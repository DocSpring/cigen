use anyhow::{Context, Result};
use cigen::loader::ConfigLoader;
use cigen::loader::context::original_dir_path;
use cigen::providers::get_provider;
use cigen::templating::TemplateEngine;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn generate_command(
    workflow: Option<String>,
    output: Option<String>,
    cli_vars: &HashMap<String, String>,
) -> Result<()> {
    let current_dir = std::env::current_dir()?;

    // Check if we should use template-based multi-output generation
    let config_path = find_config_path()?;

    if let Some(config_path) = config_path {
        let config_str = std::fs::read_to_string(&config_path)?;
        let config: cigen::models::Config = serde_yaml::from_str(&config_str)?;

        // If outputs are defined, use template-based generation
        if let Some(outputs) = &config.outputs {
            println!(
                "Generating CI configuration from templates in: {}",
                current_dir.display()
            );
            return generate_with_templates(outputs, output, cli_vars);
        }
    }

    // Fall back to the original job-based generation
    generate_from_jobs(workflow, output, cli_vars)
}

fn generate_from_jobs(
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

    // Determine output path - relative to the original directory, not the config directory
    let output_path = if let Some(output) = output {
        Path::new(&output).to_path_buf()
    } else {
        // Use config's output_path or default to provider's default
        if let Some(config_output) = &loaded_config.config.output_path {
            original_dir_path(Path::new(config_output))
        } else {
            original_dir_path(Path::new(provider.default_output_path()))
        }
    };

    if let Some(workflow_name) = workflow {
        // Generate specific workflow
        println!("Generating workflow: {workflow_name}");

        // Filter jobs for this workflow
        let workflow_prefix = format!("{workflow_name}/");
        let mut workflow_jobs: HashMap<String, cigen::models::Job> = loaded_config
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

        // Apply package deduplication if any jobs have packages
        let has_packages = workflow_jobs.values().any(|job| job.packages.is_some());
        if has_packages {
            // If we're in a .cigen dir, look for package files in parent
            let project_root = if current_dir.ends_with(".cigen") {
                current_dir.parent().and_then(|p| p.to_str()).unwrap_or(".")
            } else {
                current_dir.to_str().unwrap_or(".")
            };
            let deduplicator =
                cigen::packages::deduplicator::PackageDeduplicator::new(project_root);
            deduplicator
                .process_jobs(&mut workflow_jobs)
                .map_err(|e| anyhow::anyhow!("Failed to process package deduplication: {}", e))?;
        }

        // Check if this workflow has its own config overrides
        let workflow_config_path = PathBuf::from(format!("workflows/{workflow_name}/config.yml"));
        let workflow_config = if workflow_config_path.exists() {
            // Load workflow-specific config
            let workflow_config_str = std::fs::read_to_string(&workflow_config_path)?;

            // Parse as a partial Config to get only the override fields
            let workflow_overrides: serde_yaml::Value = serde_yaml::from_str(&workflow_config_str)?;

            // Start with the main config
            let merged_config = loaded_config.config.clone();

            // Apply overrides from workflow config
            if let Some(obj) = workflow_overrides.as_mapping() {
                // Use serde_json to convert and merge - this handles all fields automatically
                let mut base_value = serde_json::to_value(&merged_config)?;
                let override_value = serde_json::to_value(obj)?;

                if let (Some(base_obj), Some(override_obj)) =
                    (base_value.as_object_mut(), override_value.as_object())
                {
                    for (key, value) in override_obj {
                        if !value.is_null() {
                            base_obj.insert(key.clone(), value.clone());
                        }
                    }
                }

                // Convert back to Config
                serde_json::from_value(base_value)?
            } else {
                merged_config
            }
        } else {
            loaded_config.config.clone()
        };

        // Determine workflow-specific output path
        let workflow_output_path = if let Some(workflow_output) = &workflow_config.output_path {
            original_dir_path(Path::new(workflow_output))
        } else {
            output_path.clone()
        };

        provider
            .generate_workflow(
                &workflow_config,
                &workflow_name,
                &workflow_jobs,
                &loaded_config.commands,
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
            // Extract workflow name from path (e.g., "test/rspec" -> "test")
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() == 2 {
                let workflow_name = parts[0];
                let job_name = parts[1];
                workflows
                    .entry(workflow_name.to_string())
                    .or_default()
                    .insert(job_name.to_string(), job);
            }
        }

        // Apply package deduplication to each workflow
        for workflow_jobs in workflows.values_mut() {
            let has_packages = workflow_jobs.values().any(|job| job.packages.is_some());
            if has_packages {
                // If we're in a .cigen dir, look for package files in parent
                let project_root = if current_dir.ends_with(".cigen") {
                    current_dir.parent().and_then(|p| p.to_str()).unwrap_or(".")
                } else {
                    current_dir.to_str().unwrap_or(".")
                };
                let deduplicator =
                    cigen::packages::deduplicator::PackageDeduplicator::new(project_root);
                deduplicator.process_jobs(workflow_jobs).map_err(|e| {
                    anyhow::anyhow!("Failed to process package deduplication: {}", e)
                })?;
            }
        }

        // Build merged per-workflow configs
        let mut merged_workflow_configs: HashMap<String, cigen::models::Config> = HashMap::new();
        for workflow_name in workflows.keys() {
            let workflow_config_path =
                PathBuf::from(format!("workflows/{workflow_name}/config.yml"));
            let base_conf = loaded_config.config.clone();
            let merged = if workflow_config_path.exists() {
                let s = std::fs::read_to_string(&workflow_config_path)?;
                let overrides: serde_yaml::Value = serde_yaml::from_str(&s)?;
                if let Some(obj) = overrides.as_mapping() {
                    let mut base_value = serde_json::to_value(&base_conf)?;
                    let override_value = serde_json::to_value(obj)?;
                    if let (Some(base_obj), Some(override_obj)) =
                        (base_value.as_object_mut(), override_value.as_object())
                    {
                        for (k, v) in override_obj {
                            if !v.is_null() {
                                base_obj.insert(k.clone(), v.clone());
                            }
                        }
                    }
                    serde_json::from_value(base_value)?
                } else {
                    base_conf
                }
            } else {
                base_conf
            };
            merged_workflow_configs.insert(workflow_name.clone(), merged);
        }

        let has_setup = merged_workflow_configs
            .values()
            .any(|c| c.setup.unwrap_or(false));

        if has_setup {
            // Split outputs with inferred filenames
            for (workflow_name, mut jobs_copy) in workflows.clone() {
                if jobs_copy.values().any(|job| job.packages.is_some()) {
                    let project_root = if current_dir.ends_with(".cigen") {
                        current_dir.parent().and_then(|p| p.to_str()).unwrap_or(".")
                    } else {
                        current_dir.to_str().unwrap_or(".")
                    };
                    let deduplicator =
                        cigen::packages::deduplicator::PackageDeduplicator::new(project_root);
                    deduplicator.process_jobs(&mut jobs_copy).map_err(|e| {
                        anyhow::anyhow!("Failed to process package deduplication: {}", e)
                    })?;
                }

                let mut wf_conf = merged_workflow_configs.remove(&workflow_name).unwrap();

                // Infer default filename if not explicitly set
                let filename = wf_conf.output_filename.clone().unwrap_or_else(|| {
                    if wf_conf.setup.unwrap_or(false) || workflow_name == "setup" {
                        "config.yml".to_string()
                    } else if workflow_name == "main" {
                        "main.yml".to_string()
                    } else {
                        format!("{}_config.yml", workflow_name)
                    }
                });
                wf_conf.output_filename = Some(filename);

                let wf_output_path = if let Some(wf_path) = &wf_conf.output_path {
                    original_dir_path(Path::new(wf_path))
                } else {
                    output_path.clone()
                };

                provider
                    .generate_workflow(
                        &wf_conf,
                        &workflow_name,
                        &jobs_copy,
                        &loaded_config.commands,
                        &wf_output_path,
                    )
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to generate workflow '{}': {}", workflow_name, e)
                    })?;

                println!(
                    "✅ Generated {} workflow '{}' to {}",
                    provider.name(),
                    workflow_name,
                    wf_output_path.display()
                );
            }
        } else {
            // Single combined file
            provider
                .generate_all(
                    &loaded_config.config,
                    &workflows,
                    &loaded_config.commands,
                    &output_path,
                )
                .map_err(|e| anyhow::anyhow!("Failed to generate all workflows: {}", e))?;

            println!(
                "✅ Generated {} configuration for all workflows to {}",
                provider.name(),
                output_path.display()
            );

            // If dynamic mode is enabled but no explicit setup workflow exists, synthesize one
            if loaded_config.config.dynamic.unwrap_or(false)
                && !merged_workflow_configs.keys().any(|k| k == "setup")
            {
                println!("Generating synthesized setup workflow");
                // Setup should go to provider default output path (config.yml) unless overridden
                let setup_output = if let Some(setup_out) = &loaded_config.config.output_path {
                    original_dir_path(Path::new(setup_out))
                } else {
                    original_dir_path(Path::new(provider.default_output_path()))
                };
                // Call provider-specific synthesized setup if available (only CircleCI for now)
                // Provider-agnostic support: synthesize CircleCI setup when using CircleCI provider
                if loaded_config.config.provider == "circleci" {
                    cigen::providers::circleci::CircleCIProvider::new()
                        .generator
                        .generate_synthesized_setup(&loaded_config.config, &setup_output)
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to generate synthesized setup: {}", e)
                        })?;
                }
            }
        }
    }

    Ok(())
}

fn generate_with_templates(
    outputs: &[cigen::models::OutputConfig],
    specific_output: Option<String>,
    cli_vars: &HashMap<String, String>,
) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let template_dir = current_dir.join("templates");

    if !template_dir.exists() {
        anyhow::bail!(
            "Template directory not found at: {}",
            template_dir.display()
        );
    }

    // Initialize template engine
    let mut engine = TemplateEngine::new();

    // Set up template base directory for includes
    engine.set_template_base(&template_dir)?;

    // Try to load full configuration, but don't fail if jobs/commands don't exist
    let config_path = current_dir.join("cigen.yml");
    let config_str = std::fs::read_to_string(&config_path)?;
    let config: cigen::models::Config = serde_yaml::from_str(&config_str)?;

    // Add configuration to template context
    engine.add_vars_section(config.vars.as_ref().unwrap_or(&HashMap::new()));
    engine.add_cli_vars(cli_vars);
    engine.add_env_vars()?;

    // Build template context with available data
    let mut context = HashMap::new();
    context.insert("config".to_string(), serde_json::to_value(&config)?);

    // Try to load jobs and commands if they exist
    let loader_result = ConfigLoader::new_with_vars(cli_vars);
    if let Ok(mut loader) = loader_result
        && let Ok(loaded) = loader.load_all()
    {
        context.insert("jobs".to_string(), serde_json::to_value(&loaded.jobs)?);
        context.insert(
            "commands".to_string(),
            serde_json::to_value(&loaded.commands)?,
        );
    }

    // Filter outputs if specific one requested
    let outputs_to_generate: Vec<&cigen::models::OutputConfig> =
        if let Some(ref specific) = specific_output {
            outputs.iter().filter(|o| &o.output == specific).collect()
        } else {
            outputs.iter().collect()
        };

    if outputs_to_generate.is_empty() && specific_output.is_some() {
        anyhow::bail!(
            "Output '{}' not found in configuration",
            specific_output.unwrap()
        );
    }

    // Generate each output
    for output_config in outputs_to_generate {
        let template_path = template_dir.join(&output_config.template);

        if !template_path.exists() {
            anyhow::bail!("Template not found: {}", template_path.display());
        }

        println!(
            "Generating {} from template {}...",
            output_config.output, output_config.template
        );

        let template_content = std::fs::read_to_string(&template_path)?;
        let rendered = engine.render_str(&template_content, &context)?;

        // Output path should be relative to original directory using context
        let output_path =
            cigen::loader::context::original_dir_path(std::path::Path::new(&output_config.output));

        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&output_path, rendered)?;

        if let Some(ref desc) = output_config.description {
            println!("✅ Generated: {} - {}", output_config.output, desc);
        } else {
            println!("✅ Generated: {}", output_config.output);
        }
    }

    Ok(())
}

/// Find the main config file in valid locations
fn find_config_path() -> Result<Option<PathBuf>> {
    // Check the 4 valid locations in order of preference:
    // 1. .cigen/config.yml
    // 2. ./cigen.yml (root)
    // 3. ./.cigen.yml (root)
    // 4. If in .cigen/ directory, look for config.yml

    if Path::new(".cigen/config.yml").exists() {
        return Ok(Some(PathBuf::from(".cigen/config.yml")));
    }

    if Path::new("cigen.yml").exists() {
        return Ok(Some(PathBuf::from("cigen.yml")));
    }

    if Path::new(".cigen.yml").exists() {
        return Ok(Some(PathBuf::from(".cigen.yml")));
    }

    // Check if we're in .cigen directory
    let current_dir = std::env::current_dir()?;
    if current_dir.file_name() == Some(std::ffi::OsStr::new(".cigen"))
        && Path::new("config.yml").exists()
    {
        return Ok(Some(PathBuf::from("config.yml")));
    }

    Ok(None)
}
