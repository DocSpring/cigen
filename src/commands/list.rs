use anyhow::Result;
use cigen::loader::ConfigLoader;
use std::collections::HashSet;

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ListType {
    Workflows,
    Jobs,
    Commands,
    Services,
    Caches,
}

pub fn list_command(config_path: &str, resource_type: Option<ListType>) -> Result<()> {
    // Load everything
    let loader = ConfigLoader::new(config_path)?;
    let loaded = loader.load_all()?;

    match resource_type {
        None => {
            // List all resources
            println!("Configuration: {config_path}\n");

            list_workflows(&loaded)?;
            println!();
            list_jobs(&loaded)?;
            println!();
            list_commands(&loaded)?;
            println!();
            list_services(&loaded)?;
            println!();
            list_caches(&loaded)?;
        }
        Some(ListType::Workflows) => list_workflows(&loaded)?,
        Some(ListType::Jobs) => list_jobs(&loaded)?,
        Some(ListType::Commands) => list_commands(&loaded)?,
        Some(ListType::Services) => list_services(&loaded)?,
        Some(ListType::Caches) => list_caches(&loaded)?,
    }

    Ok(())
}

fn list_workflows(loaded: &cigen::loader::LoadedConfig) -> Result<()> {
    println!("Workflows:");

    // Extract workflow names from job paths (workflow/job format)
    let mut workflows = HashSet::new();
    for job_path in loaded.jobs.keys() {
        if let Some(workflow) = job_path.split('/').next() {
            workflows.insert(workflow);
        }
    }

    if workflows.is_empty() {
        println!("  (none)");
    } else {
        let mut workflows: Vec<_> = workflows.into_iter().collect();
        workflows.sort();
        for workflow in workflows {
            println!("  - {workflow}");
        }
    }

    Ok(())
}

fn list_jobs(loaded: &cigen::loader::LoadedConfig) -> Result<()> {
    println!("Jobs:");

    if loaded.jobs.is_empty() {
        println!("  (none)");
    } else {
        let mut job_paths: Vec<_> = loaded.jobs.keys().collect();
        job_paths.sort();

        let mut current_workflow = "";
        for path in job_paths {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() == 2 {
                let workflow = parts[0];
                let job = parts[1];

                if workflow != current_workflow {
                    println!("  {workflow}:");
                    current_workflow = workflow;
                }
                println!("    - {job}");
            }
        }
    }

    Ok(())
}

fn list_commands(loaded: &cigen::loader::LoadedConfig) -> Result<()> {
    println!("Commands:");

    if loaded.commands.is_empty() {
        println!("  (none)");
    } else {
        let mut command_names: Vec<_> = loaded.commands.keys().collect();
        command_names.sort();
        for name in command_names {
            println!("  - {name}");
        }
    }

    Ok(())
}

fn list_services(loaded: &cigen::loader::LoadedConfig) -> Result<()> {
    println!("Services:");

    if let Some(services) = &loaded.config.services {
        if services.is_empty() {
            println!("  (none)");
        } else {
            let mut service_names: Vec<_> = services.keys().collect();
            service_names.sort();
            for name in service_names {
                if let Some(service) = services.get(name) {
                    println!("  - {name} ({})", service.image);
                }
            }
        }
    } else {
        println!("  (none)");
    }

    Ok(())
}

fn list_caches(loaded: &cigen::loader::LoadedConfig) -> Result<()> {
    println!("Cache Backends:");

    if let Some(cache_config) = &loaded.config.caches {
        let mut has_caches = false;

        if let Some(artifacts) = &cache_config.artifacts {
            println!("  - artifacts (backend: {})", artifacts.backend);
            has_caches = true;
        }

        if let Some(job_status) = &cache_config.job_status {
            println!("  - job_status (backend: {})", job_status.backend);
            has_caches = true;
        }

        if !has_caches {
            println!("  (none)");
        }
    } else {
        println!("  (none)");
    }

    Ok(())
}
