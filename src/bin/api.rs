use std::net::IpAddr;

use clap::Parser;
use context::api::{self, Config};

#[derive(Parser)]
#[command(name = "c5t-api")]
#[command(author, version, about = "Context API server", long_about = None)]
struct Cli {
    /// Host address to bind to
    #[arg(long, default_value = "0.0.0.0")]
    host: IpAddr,

    /// Port to listen on
    #[arg(short, long, default_value = "3000")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    api::run(Config {
        host: cli.host,
        port: cli.port,
    })
    .await
}
