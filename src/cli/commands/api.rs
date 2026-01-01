//! API server command - starts REST API + MCP + embedded frontend

use miette::Result;
use std::sync::Arc;

use crate::api::{self, AppState};
use crate::db::sqlite::SqliteDatabase;
use crate::sync::RealGit;

/// Run the API server
pub async fn run(
    host: String,
    port: u16,
    enable_docs: bool,
    db_path: Option<String>,
) -> Result<()> {
    // Initialize database
    let db_path = db_path.unwrap_or_else(|| {
        let home = std::env::var("HOME").expect("HOME environment variable not set");
        format!("{}/.c5t/c5t.db", home)
    });

    let db = SqliteDatabase::new(&db_path).await?;
    db.run_migrations().await?;

    let db_arc = Arc::new(db);
    let sync_manager = crate::sync::SyncManager::new(RealGit::new());

    let state = AppState::new(db_arc, sync_manager);
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
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, router).await?;

    Ok(())
}
