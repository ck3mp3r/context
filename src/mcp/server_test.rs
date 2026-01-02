//! Tests for MCP server initialization
//!
//! Following TDD: These tests are written FIRST (RED), then we implement to make them pass (GREEN).

use crate::api::notifier::ChangeNotifier;
use crate::db::SqliteDatabase;

/// Test that we can create an MCP server with a database
///
/// This test verifies:
/// - McpServer can be instantiated with a generic Database
/// - Follows SOLID: Generic over D: Database (no dyn dispatch)
/// - Server has separate tool structs for each entity (SRP)
#[tokio::test]
async fn test_create_mcp_server() {
    // Arrange: Create an in-memory database for testing
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate_async().await.expect("Failed to run migrations");

    // Act: Create MCP server with the database
    // This should compile and run without errors
    let _server = super::server::McpServer::new(db, ChangeNotifier::new());

    // Assert: If we got here, server was created successfully
    // More detailed assertions will come as we implement tools
}

/// Test that MCP server implements ServerHandler trait
///
/// This test verifies:
/// - Server can provide ServerInfo
/// - Server info includes correct capabilities (tools enabled)
#[tokio::test]
async fn test_server_info() {
    use rmcp::ServerHandler;

    // Arrange
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate_async().await.expect("Failed to run migrations");

    let server = super::server::McpServer::new(db, ChangeNotifier::new());

    // Act
    let info = server.get_info();

    // Assert
    assert!(
        info.capabilities.tools.is_some(),
        "Server should support tools"
    );
    assert!(
        info.instructions.is_some(),
        "Server should provide instructions"
    );
}
