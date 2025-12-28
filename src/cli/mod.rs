pub mod api_client;
mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "c5t")]
#[command(author, version, about = "Context management CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show version information
    Version,
}

pub fn run() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Version) => {
            println!("c5t {}", env!("CARGO_PKG_VERSION"));
        }
        None => {
            println!("c5t {}", env!("CARGO_PKG_VERSION"));
        }
    }
}
