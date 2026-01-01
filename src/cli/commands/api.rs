//! API server command - starts REST API + MCP + embedded frontend

use miette::{IntoDiagnostic, Result};

use crate::api::{self, AppState};
use crate::db::Database;
use crate::db::sqlite::SqliteDatabase;
use crate::sync::RealGit;

/// Run the API server
pub async fn run(
    host: String,
    port: u16,
    enable_docs: bool,
    db_path: Option<String>,
) -> Result<()> {
    // Initialize database - check env var if not provided via CLI
    let db_path = db_path
        .or_else(|| std::env::var("C5T_DB_PATH").ok())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").expect("HOME environment variable not set");
            format!("{}/.c5t/c5t.db", home)
        });

    let db = SqliteDatabase::open(&db_path).await?;
    db.migrate()?;

    let sync_manager = crate::sync::SyncManager::new(RealGit::new());

    let state = AppState::new(db, sync_manager);
    let router = api::routes::create_router(state, enable_docs);

    // Print startup banner
    println!("🚀 c5t API server starting...");
    println!("   API:      http://{}:{}/api/v1", host, port);
    println!("   MCP:      http://{}:{}/mcp", host, port);
    println!("   Frontend: http://{}:{}/", host, port);
    if enable_docs {
        println!("   Docs:     http://{}:{}/docs", host, port);
    }
    println!();
    println!("   Database: {}", db_path);
    println!();

    // Start server
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .into_diagnostic()?;

    axum::serve(listener, router).await.into_diagnostic()?;

    Ok(())
}
