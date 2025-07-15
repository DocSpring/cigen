use anyhow::Result;
use clap::Parser;
use std::path::Path;

use cigen::validation::Validator;

#[derive(Parser)]
#[command(
    name = "cigen",
    about = "A CLI tool that generates CI pipeline configurations from templates",
    version,
    author,
    long_about = None
)]
struct Cli {
    /// Validate configuration files without generating output
    #[arg(short, long)]
    validate: bool,

    /// Path to the cigen configuration directory or file
    #[arg(short, long, default_value = ".cigen")]
    config: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.validate {
        validate_command(&cli.config)?;
    }

    Ok(())
}

fn validate_command(config_path: &str) -> Result<()> {
    let path = Path::new(config_path);

    // Create validator
    let validator = Validator::new()?;

    // Determine if path is a directory or file
    if path.is_dir() {
        // Validate all configs in directory
        println!("Validating configuration directory: {config_path}");
        validator.validate_all(path)?;
    } else if path.is_file()
        && path
            .extension()
            .map(|e| e == "yml" || e == "yaml")
            .unwrap_or(false)
    {
        // Validate single file
        println!("Validating configuration file: {config_path}");
        validator.validate_config(path)?;
    } else {
        anyhow::bail!(
            "Invalid config path: {} (must be a directory or YAML file)",
            config_path
        );
    }

    println!("\n✅ All validations passed!");
    Ok(())
}
