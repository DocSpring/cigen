use anyhow::Result;
use clap::{Parser, Subcommand};
use std::collections::HashMap;

mod commands;

fn parse_var(s: &str) -> Result<(String, String), String> {
    match s.split_once('=') {
        Some((key, value)) => Ok((key.to_string(), value.to_string())),
        None => Err("Variables must be in format key=value".to_string()),
    }
}

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

    /// Set template variables (can be used multiple times)
    #[arg(long = "var", global = true, value_parser = parse_var)]
    vars: Vec<(String, String)>,

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
        /// Workflow name to generate (generates all if not specified)
        workflow: Option<String>,

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

        /// DPI for graph output (default: 120)
        #[arg(long = "graph-dpi")]
        graph_dpi: Option<u32>,

        /// Graph size in inches (default: "15,10")
        #[arg(long = "graph-size")]
        graph_size: Option<String>,

        /// Color for graph text and lines (default: "white")
        #[arg(long = "graph-color")]
        graph_color: Option<String>,
    },

    /// Render CI configs from cigen.yml (new plugin-based system)
    Render {
        /// Path to cigen.yml (default: ./cigen.yml or .cigen/cigen.yml)
        #[arg(short, long)]
        file: Option<String>,

        /// Output directory for generated files (default: .)
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() -> Result<()> {
    // Install miette error reporter for better error display
    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(true)
                .unicode(true)
                .context_lines(3)
                .build(),
        )
    }))
    .unwrap();

    let cli = Cli::parse();

    // Initialize logging based on verbose flag
    init_logging(cli.verbose);

    // Convert CLI vars to HashMap
    let cli_vars: HashMap<String, String> = cli.vars.into_iter().collect();

    match cli.command {
        Some(Commands::Init { template }) => {
            // Don't change working directory for init command
            commands::init_command(&cli.config, template)?;
        }
        _ => {
            // For all other commands, optionally change to config directory if it exists
            let original_dir = std::env::current_dir()?;
            let config_path = std::path::Path::new(&cli.config);
            let config_dir = if config_path.is_absolute() {
                config_path.to_path_buf()
            } else {
                original_dir.join(config_path)
            };

            if config_dir.exists() {
                // If config_dir is a .cigen directory, treat its parent as the project root (original_dir)
                let project_root = if config_dir.file_name() == Some(std::ffi::OsStr::new(".cigen"))
                {
                    config_dir
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or(original_dir.clone())
                } else {
                    original_dir.clone()
                };
                // Initialize context before changing directory
                cigen::loader::context::init_context(project_root, config_dir.clone());

                std::env::set_current_dir(&config_dir)?;
                tracing::debug!("Changed working directory to: {}", config_dir.display());
            } else {
                // Initialize context for current directory (inline config support)
                cigen::loader::context::init_context(original_dir.clone(), original_dir);
                tracing::debug!("Using current directory for inline config");
            }

            match cli.command {
                Some(Commands::Validate) => {
                    commands::validate_command(&cli_vars)?;
                }
                Some(Commands::Generate { workflow, output }) => {
                    commands::generate_command(workflow, output, &cli_vars)?;
                }
                Some(Commands::List { resource_type }) => {
                    commands::list_command(resource_type, &cli_vars)?;
                }
                Some(Commands::Inspect { object_type, path }) => {
                    commands::inspect_command(object_type, path, &cli_vars)?;
                }
                Some(Commands::Graph {
                    workflow,
                    output,
                    graph_dpi,
                    graph_size,
                    graph_color,
                }) => {
                    commands::graph_command(
                        workflow,
                        output,
                        graph_dpi,
                        graph_size,
                        graph_color,
                        &cli_vars,
                    )?;
                }
                Some(Commands::Render { file, output }) => {
                    commands::render_command(file, output)?;
                }
                None => {
                    // Default to generate command
                    commands::generate_command(None, None, &cli_vars)?;
                }
                _ => unreachable!(),
            }
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
