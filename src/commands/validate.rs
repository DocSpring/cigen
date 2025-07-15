use anyhow::Result;
use cigen::loader::ConfigLoader;
use std::collections::HashMap;

pub fn validate_command(config_path: &str, cli_vars: &HashMap<String, String>) -> Result<()> {
    println!("Validating configuration directory: {config_path}");

    // The loader runs all validation as part of loading
    let mut loader = ConfigLoader::new_with_vars(config_path, cli_vars)?;
    let _loaded = loader.load_all()?;

    println!("\n✅ All validations passed!");
    Ok(())
}
