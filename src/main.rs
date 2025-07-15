use anyhow::Result;
use clap::{Parser, Subcommand};

use cigen::loader::ConfigLoader;

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

    /// Enable verbose output (use -vv for debug output)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
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

    /// Inspect internal object model
    Inspect {
        #[arg(value_enum)]
        object_type: InspectType,

        #[arg(help = "Path to the object (e.g., 'test/bootsnap' for a job)")]
        path: String,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum InspectType {
    Config,
    Job,
    Command,
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
        Some(Commands::Inspect { object_type, path }) => {
            inspect_command(&cli.config, object_type, &path)?;
        }
        None => {
            // Default to generate command
            generate_command(&cli.config, None)?;
        }
    }

    Ok(())
}

fn init_logging(verbose: u8) {
    use tracing_subscriber::EnvFilter;

    let filter = match verbose {
        0 => EnvFilter::new("cigen=warn"), // Default: warnings and errors only
        1 => EnvFilter::new("cigen=info"), // -v: info messages
        _ => EnvFilter::new("cigen=debug"), // -vv or more: full debug
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

fn inspect_command(config_path: &str, object_type: InspectType, path: &str) -> Result<()> {
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

fn validate_command(config_path: &str) -> Result<()> {
    println!("Validating configuration directory: {config_path}");

    // The loader runs all validation as part of loading
    let loader = ConfigLoader::new(config_path)?;
    let _loaded = loader.load_all()?;

    println!("\n✅ All validations passed!");
    Ok(())
}
