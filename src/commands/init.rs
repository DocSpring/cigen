use anyhow::Result;

pub fn init_command(config_path: &str, template: Option<String>) -> Result<()> {
    println!("Initializing new cigen project in: {config_path}");
    if let Some(template) = template {
        println!("Using template: {template}");
    }
    // TODO: Implement project initialization
    anyhow::bail!("Init command not yet implemented");
}
