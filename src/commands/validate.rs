use anyhow::Result;
use cigen::loader::ConfigLoader;

pub fn validate_command(config_path: &str) -> Result<()> {
    println!("Validating configuration directory: {config_path}");

    // The loader runs all validation as part of loading
    let loader = ConfigLoader::new(config_path)?;
    let _loaded = loader.load_all()?;

    println!("\n✅ All validations passed!");
    Ok(())
}
