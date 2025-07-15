use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

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
    /// Initialize a new cigen config directory
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

    /// List available resources
    List {
        /// Optional resource type to list (if not specified, lists all)
        #[arg(value_enum)]
        resource_type: Option<commands::ListType>,
    },

    /// Inspect internal object model
    Inspect {
        #[arg(value_enum)]
        object_type: commands::InspectType,

        #[arg(
            help = "Path to the object (e.g., 'test/bootsnap' for a job). Not required for 'config'."
        )]
        path: Option<String>,
    },

    /// Display job dependency graph
    Graph {
        /// Workflow name to display (shows all workflows if not specified)
        workflow: Option<String>,

        /// Save graph to PNG file
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
            commands::init_command(&cli.config, template)?;
        }
        Some(Commands::Validate) => {
            commands::validate_command(&cli.config)?;
        }
        Some(Commands::Generate { output }) => {
            commands::generate_command(&cli.config, output)?;
        }
        Some(Commands::List { resource_type }) => {
            commands::list_command(&cli.config, resource_type)?;
        }
        Some(Commands::Inspect { object_type, path }) => {
            commands::inspect_command(&cli.config, object_type, path)?;
        }
        Some(Commands::Graph { workflow, output }) => {
            commands::graph_command(&cli.config, workflow, output)?;
        }
        None => {
            // Default to generate command
            commands::generate_command(&cli.config, None)?;
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
