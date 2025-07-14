use clap::Parser;

#[derive(Parser)]
#[command(
    name = "cigen",
    about = "A CLI tool that generates CI pipeline configurations from templates",
    version,
    author,
    long_about = None
)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
