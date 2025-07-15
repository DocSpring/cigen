use anyhow::Result;
use cigen::loader::ConfigLoader;
use std::collections::HashMap;

pub fn validate_command(cli_vars: &HashMap<String, String>) -> Result<()> {
    println!(
        "Validating configuration directory: {}",
        std::env::current_dir()?.display()
    );

    // The loader runs all validation as part of loading
    let mut loader = ConfigLoader::new_with_vars(cli_vars)?;
    let _loaded = loader.load_all()?;

    println!("\n✅ All validations passed!");
    Ok(())
}
