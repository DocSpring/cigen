use anyhow::Result;
use cigen::loader::ConfigLoader;

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
) -> Result<()> {
    // Load everything
    let loader = ConfigLoader::new(config_path)?;
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
