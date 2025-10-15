use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(
    name = "cigen",
    about = "A CLI tool that generates CI pipeline configurations",
    version,
    author
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose output (use -vv for debug output)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate CI configuration (default command)
    Generate {
        /// Path to cigen.yml (default: ./cigen.yml or .cigen/cigen.yml)
        #[arg(short, long)]
        file: Option<String>,

        /// Output directory for generated files (default: .)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Compute hashes for file patterns or jobs
    Hash {
        #[command(flatten)]
        args: commands::HashArgs,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    match cli.command {
        Some(Commands::Generate { file, output }) => {
            commands::generate_command(file, output)?;
        }
        Some(Commands::Hash { args }) => {
            commands::hash_command(args)?;
        }
        None => {
            // Default to generate command
            commands::generate_command(None, None)?;
        }
    }

    Ok(())
}

fn init_logging(verbose: u8) {
    use tracing_subscriber::EnvFilter;

    let filter = match verbose {
        0 => EnvFilter::new("cigen=warn"),
        1 => EnvFilter::new("cigen=info"),
        _ => EnvFilter::new("cigen=debug"),
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}
