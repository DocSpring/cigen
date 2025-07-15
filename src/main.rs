use anyhow::Result;
use clap::{Parser, Subcommand};
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
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to the cigen configuration directory
    #[arg(short, long, default_value = ".cigen", global = true)]
    config: String,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new cigen project
    Init {
        /// Project template to use
        #[arg(short, long)]
        template: Option<String>,
    },

    /// Validate configuration files
    Validate,

    /// Generate CI configuration (default command)
    Generate {
        /// Output directory for generated files
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on verbose flag
    init_logging(cli.verbose);

    match cli.command {
        Some(Commands::Init { template }) => {
            init_command(&cli.config, template)?;
        }
        Some(Commands::Validate) => {
            validate_command(&cli.config)?;
        }
        Some(Commands::Generate { output }) => {
            generate_command(&cli.config, output)?;
        }
        None => {
            // Default to generate command
            generate_command(&cli.config, None)?;
        }
    }

    Ok(())
}

fn init_logging(verbose: bool) {
    use tracing_subscriber::EnvFilter;

    let filter = if verbose {
        EnvFilter::new("cigen=debug,info")
    } else {
        // Only show warnings and errors by default
        EnvFilter::new("cigen=warn,error")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}

fn init_command(config_path: &str, template: Option<String>) -> Result<()> {
    println!("Initializing new cigen project in: {config_path}");
    if let Some(template) = template {
        println!("Using template: {template}");
    }
    // TODO: Implement project initialization
    anyhow::bail!("Init command not yet implemented");
}

fn generate_command(config_path: &str, output: Option<String>) -> Result<()> {
    println!("Generating CI configuration from: {config_path}");
    if let Some(output) = output {
        println!("Output directory: {output}");
    }
    // TODO: Implement CI generation
    anyhow::bail!("Generate command not yet implemented");
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
