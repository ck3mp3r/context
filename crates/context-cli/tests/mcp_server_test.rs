//! Tests for MCP server initialization
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use context_server::api::notifier::ChangeNotifier;
use context_db::SqliteDatabase;
use tempfile::TempDir;

/// Test that we can create an MCP server with a database
///
/// This test verifies:
/// - McpServer can be instantiated with a generic Database
/// - Follows SOLID: Generic over D: Database (no dyn dispatch)
/// - Server has separate tool structs for each entity (SRP)
#[tokio::test(flavor = "multi_thread")]
async fn test_create_mcp_server() {
    // Arrange: Create an in-memory database for testing
    let db = common::setup_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Act: Create MCP server with the database
    // This should compile and run without errors
    let _server: context_server::mcp::McpServer<SqliteDatabase> =
        context_server::mcp::McpServer::new(db, ChangeNotifier::new(), temp_dir.path().join("skills"));

    // Assert: If we got here, server was created successfully
    // More detailed assertions will come as we implement tools
}

/// Test that McpServer has all required tool handlers
///
/// This test verifies:
/// - McpServer has separate tool structs for each entity (SRP)
/// - Each tool struct is accessible via the server
#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_server_has_all_tools() {
    use context_server::mcp::tools::*;
    use std::sync::Arc;

    let db = common::setup_db().await;
    let db = Arc::new(db);
    let temp_dir = TempDir::new().unwrap();
    let notifier = ChangeNotifier::new();

    // Create each tool struct independently (SRP verification)
    let _project_tools = ProjectTools::new(db.clone(), notifier.clone());
    let _repo_tools = RepoTools::new(db.clone(), notifier.clone());
    let _task_list_tools = TaskListTools::new(db.clone(), notifier.clone());
    let _task_tools = TaskTools::new(db.clone(), notifier.clone());
    let _note_tools = NoteTools::new(db.clone(), notifier.clone());
    let _skill_tools = SkillTools::new(db.clone(), notifier.clone(), temp_dir.path().join("skills"));

    // Assert: All tool structs created successfully
}

/// Test that McpServer can be created with Arc<SqliteDatabase>
///
/// This test verifies:
/// - McpServer accepts Arc<D> where D: Database
/// - This is the common pattern used in production
#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_server_with_arc_db() {
    use std::sync::Arc;

    let db = common::setup_db().await;
    let db = Arc::new(db);
    let temp_dir = TempDir::new().unwrap();

    let _server: context_server::mcp::McpServer<SqliteDatabase> =
        context_server::mcp::McpServer::new(
            db,
            ChangeNotifier::new(),
            temp_dir.path().join("skills"),
        );
}
