//! Context API server binary.
//!
//! This binary creates the concrete database implementation and passes it
//! to the API server. The API layer remains agnostic of the storage backend.

use std::net::IpAddr;
use std::path::PathBuf;

use clap::Parser;
use context::api::{self, Config};
use context::db::{Database, SqliteDatabase};

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

    /// Database file path (uses in-memory if not specified)
    #[arg(long)]
    db: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Create the concrete database implementation
    let db = match &cli.db {
        Some(path) => {
            println!("Opening database at {:?}", path);
            SqliteDatabase::open(path).await?
        }
        None => {
            println!("Using in-memory database");
            SqliteDatabase::in_memory().await?
        }
    };

    // Run migrations before starting the server
    db.migrate()?;
    println!("Database migrations complete");

    // Pass the abstract Database to the API layer
    api::run(
        Config {
            host: cli.host,
            port: cli.port,
        },
        db,
    )
    .await
}
