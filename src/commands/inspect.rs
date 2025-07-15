use anyhow::Result;
use cigen::loader::ConfigLoader;

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum InspectType {
    Config,
    Job,
    Command,
}

pub fn inspect_command(config_path: &str, object_type: InspectType, path: &str) -> Result<()> {
    // Load everything
    let loader = ConfigLoader::new(config_path)?;
    let loaded = loader.load_all()?;

    match object_type {
        InspectType::Config => {
            println!("{:#?}", loaded.config);
        }
        InspectType::Job => {
            if let Some(job) = loaded.jobs.get(path) {
                job.pretty_print();
            } else {
                anyhow::bail!("Job not found: {}", path);
            }
        }
        InspectType::Command => {
            if let Some(command) = loaded.commands.get(path) {
                println!("{command:#?}");
            } else {
                anyhow::bail!("Command not found: {}", path);
            }
        }
    }

    Ok(())
}
