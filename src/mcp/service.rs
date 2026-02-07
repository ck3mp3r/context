//! MCP Streamable HTTP service creation
//!
//! This module provides functions to create the MCP service
//! that can be integrated with an Axum router.

use std::sync::Arc;

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;

use crate::db::Database;

use super::server::McpServer;

/// Create MCP Streamable HTTP service
///
/// This function creates a StreamableHttpService that can be nested into an Axum router.
///
/// # Arguments
/// * `db` - Database instance implementing the Database trait
/// * `cancellation_token` - Token for graceful shutdown
///
/// # Returns
/// A StreamableHttpService that implements tower::Service
///
/// # Example
/// ```no_run
/// use axum::Router;
/// use tokio_util::sync::CancellationToken;
/// # use context::db::SqliteDatabase;
/// # use context::mcp::create_mcp_service;
/// # use context::api::notifier::ChangeNotifier;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let db = SqliteDatabase::in_memory().await?;
///
/// let ct = CancellationToken::new();
/// let notifier = ChangeNotifier::new();
/// let mcp_service = create_mcp_service(db, notifier, ct);
///
/// let app: Router = Router::new()
///     .nest_service("/mcp", mcp_service);
/// # Ok(())
/// # }
/// ```
pub fn create_mcp_service<D: Database + 'static>(
    db: impl Into<Arc<D>>,
    notifier: crate::api::notifier::ChangeNotifier,
    skills_dir: std::path::PathBuf,
    cancellation_token: CancellationToken,
) -> StreamableHttpService<McpServer<D>> {
    let db = db.into();

    // Service factory: creates new McpServer instance per session
    // Note: Returns io::Error to match rmcp's expected signature
    let service_factory = move || -> Result<McpServer<D>, std::io::Error> {
        let server = McpServer::new(Arc::clone(&db), notifier.clone(), skills_dir.clone());
        Ok(server)
    };

    // Configure Streamable HTTP server
    let config = StreamableHttpServerConfig {
        sse_keep_alive: None, // Use default (15s)
        sse_retry: None,      // Use default retry behavior
        stateful_mode: true,  // Enable session management
        cancellation_token,
    };

    // Create service with local session manager
    StreamableHttpService::new(
        service_factory,
        LocalSessionManager::default().into(),
        config,
    )
}
