use clap::Parser;

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

    /// Path to the cigen configuration file
    #[arg(short, long, default_value = ".cigen/cigen.yml")]
    config: String,
}

fn main() {
    let cli = Cli::parse();

    if cli.validate {
        println!("Validating configuration: {}", cli.config);
        // TODO: Implement validation logic
        std::process::exit(0);
    }
}
