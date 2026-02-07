//! API server command - starts REST API + MCP + embedded frontend

use std::net::IpAddr;
use std::path::PathBuf;

use miette::{IntoDiagnostic, Result};

use crate::api::{self, Config};
use crate::db::Database;
use crate::db::sqlite::SqliteDatabase;
use crate::sync::{get_db_path, set_base_path};

/// Run the API server
pub async fn run(
    host: IpAddr,
    port: u16,
    home: Option<PathBuf>,
    skills_dir: Option<PathBuf>,
    verbosity: u8,
    enable_docs: bool,
) -> Result<()> {
    // Set the global base path if provided (API startup singleton pattern)
    if let Some(home_path) = home {
        set_base_path(home_path);
    }

    // Use the singleton to get db path
    let db_path = get_db_path();

    println!("Opening database at {:?}", db_path);

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).into_diagnostic()?;
    }

    let db = SqliteDatabase::open(&db_path).await?;

    // Run migrations before starting the server
    db.migrate()?;
    println!("Database migrations complete");

    // Print startup banner BEFORE starting server (before logging is initialized)
    println!();
    println!("🚀 c5t API server starting...");
    println!("   API:      http://{}:{}/api/v1", host, port);
    println!("   MCP:      http://{}:{}/mcp", host, port);
    println!("   Frontend: http://{}:{}/", host, port);
    if enable_docs {
        println!("   Docs:     http://{}:{}/docs", host, port);
    }
    println!();
    println!("   Database: {}", db_path.display());
    println!();

    // Pass the abstract Database to the API layer
    api::run(
        Config {
            host,
            port,
            verbosity,
            enable_docs,
            skills_dir: match skills_dir {
                Some(dir) => dir,
                None => match std::env::var("C5T_SKILLS_DIR") {
                    Ok(dir) => PathBuf::from(dir),
                    Err(_) => crate::sync::get_data_dir().join("skills"),
                },
            },
        },
        db,
    )
    .await
    .into_diagnostic()?;

    Ok(())
}
