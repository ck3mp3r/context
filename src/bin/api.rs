//! Context API server binary.
//!
//! This binary creates the concrete database implementation and passes it
//! to the API server. The API layer remains agnostic of the storage backend.

use std::net::IpAddr;
use std::path::PathBuf;

use clap::Parser;
use context::api::{self, ApiError, Config};
use context::db::{Database, DbError, SqliteDatabase};
use context::sync::get_db_path;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
enum BinaryError {
    #[error("Database error: {0}")]
    #[diagnostic(code(c5t::binary::database))]
    Database(#[from] DbError),

    #[error("Failed to create data directory: {0}")]
    #[diagnostic(code(c5t::binary::io))]
    Io(#[from] std::io::Error),

    #[error("API server error: {0}")]
    #[diagnostic(code(c5t::binary::api))]
    Api(#[from] ApiError),
}

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

    /// Database file path (defaults to XDG data directory: ~/.local/share/c5t-test/context.db)
    #[arg(long)]
    db: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), BinaryError> {
    let cli = Cli::parse();

    // Create the concrete database implementation
    let db_path = cli.db.unwrap_or_else(get_db_path);

    println!("Opening database at {:?}", db_path);

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = SqliteDatabase::open(&db_path).await?;

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
    .await?;

    Ok(())
}
