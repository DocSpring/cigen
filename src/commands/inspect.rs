use anyhow::Result;
use cigen::loader::ConfigLoader;
use std::collections::HashMap;

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum InspectType {
    Config,
    Job,
    Command,
}

pub fn inspect_command(
    config_path: &str,
    object_type: InspectType,
    path: Option<String>,
    cli_vars: &HashMap<String, String>,
) -> Result<()> {
    // Load everything
    let mut loader = ConfigLoader::new_with_vars(config_path, cli_vars)?;
    let loaded = loader.load_all()?;

    match object_type {
        InspectType::Config => {
            let json = serde_json::to_string_pretty(&loaded.config)?;
            println!("{json}");
        }
        InspectType::Job => {
            let job_path =
                path.ok_or_else(|| anyhow::anyhow!("Path required for job inspection"))?;
            if let Some(job) = loaded.jobs.get(&job_path) {
                let json = serde_json::to_string_pretty(&job)?;
                println!("{json}");
            } else {
                anyhow::bail!("Job not found: {}", job_path);
            }
        }
        InspectType::Command => {
            let cmd_name =
                path.ok_or_else(|| anyhow::anyhow!("Path required for command inspection"))?;
            if let Some(command) = loaded.commands.get(&cmd_name) {
                let json = serde_json::to_string_pretty(&command)?;
                println!("{json}");
            } else {
                anyhow::bail!("Command not found: {}", cmd_name);
            }
        }
    }

    Ok(())
}
