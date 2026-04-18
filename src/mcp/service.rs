//! MCP Streamable HTTP service creation
//!
//! This module provides functions to create the MCP service
//! that can be integrated with an Axum router.

use std::sync::Arc;

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;

use crate::a6s::store::surrealdb;
use crate::db::Database;

use super::server::McpServer;

/// Create MCP Streamable HTTP service
///
/// This function creates a StreamableHttpService that can be nested into an Axum router.
///
/// # Arguments
/// * `db` - Database instance implementing the Database trait
/// * `notifier` - Change notifier for WebSocket broadcasts
/// * `skills_dir` - Directory where skill attachments are extracted
/// * `analysis_db` - Shared SurrealDB connection for code analysis
/// * `cancellation_token` - Token for graceful shutdown
///
/// # Returns
/// A StreamableHttpService that implements tower::Service
///
/// # Example
/// ```ignore
/// use axum::Router;
/// use tokio_util::sync::CancellationToken;
/// use tempfile::TempDir;
/// use context::db::SqliteDatabase;
/// use context::mcp::create_mcp_service;
/// use context::api::notifier::ChangeNotifier;
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let db = SqliteDatabase::in_memory().await?;
///
/// let ct = CancellationToken::new();
/// let notifier = ChangeNotifier::new();
/// let temp_dir = TempDir::new().unwrap();
/// let skills_dir = temp_dir.path().join("skills");
/// let analysis_db = Arc::new(context::a6s::surrealdb::init_shared_db().await?);
/// let mcp_service = create_mcp_service(db, notifier, skills_dir, analysis_db, ct);
/// }
/// ```
pub fn create_mcp_service<D: Database + 'static>(
    db: impl Into<Arc<D>>,
    notifier: crate::api::notifier::ChangeNotifier,
    skills_dir: std::path::PathBuf,
    analysis_db: Arc<surrealdb::SurrealDbConnection>,
    cancellation_token: CancellationToken,
) -> StreamableHttpService<McpServer<D>, LocalSessionManager> {
    let db = db.into();

    // Service factory: creates new McpServer instance per session
    // Note: Returns io::Error to match rmcp's expected signature
    let service_factory = move || -> Result<McpServer<D>, std::io::Error> {
        let server = McpServer::new(
            Arc::clone(&db),
            notifier.clone(),
            skills_dir.clone(),
            Arc::clone(&analysis_db),
        );
        Ok(server)
    };

    // Configure Streamable HTTP server
    // rmcp 1.5+ enforces DNS rebinding protection via allowed_hosts.
    // Default only permits localhost/127.0.0.1/::1, but c5t binds to 0.0.0.0
    // so we must include it to avoid 403 Forbidden on MCP client connections.
    let config = StreamableHttpServerConfig::default()
        .with_allowed_hosts(["localhost", "127.0.0.1", "::1", "0.0.0.0"])
        .with_sse_keep_alive(None)
        .with_sse_retry(None)
        .with_stateful_mode(true)
        .with_cancellation_token(cancellation_token);

    // Create service with local session manager
    StreamableHttpService::new(
        service_factory,
        LocalSessionManager::default().into(),
        config,
    )
}
