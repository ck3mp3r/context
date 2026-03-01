//! Tests for MCP server initialization
//!
//! Following TDD: These tests are written FIRST (RED), then we implement to make them pass (GREEN).

use crate::api::notifier::ChangeNotifier;
use crate::db::SqliteDatabase;
use tempfile::TempDir;

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
    let temp_dir = TempDir::new().unwrap();

    // Act: Create MCP server with the database
    // This should compile and run without errors
    let _server =
        super::server::McpServer::new(db, ChangeNotifier::new(), temp_dir.path().join("skills"));

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

    let temp_dir = TempDir::new().unwrap();
    let server =
        super::server::McpServer::new(db, ChangeNotifier::new(), temp_dir.path().join("skills"));

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

/// Test that update_skill tool is registered and callable via MCP server
///
/// This test verifies:
/// - update_skill method exists on McpServer
/// - Tool is accessible via the MCP interface
/// - Follows TDD: RED (this test will fail until tool is registered) → GREEN (register tool)
#[tokio::test]
async fn test_update_skill_tool_registered() {
    use crate::db::{Database, Skill, SkillRepository};
    use crate::mcp::tools::skills::UpdateSkillParams;
    use rmcp::handler::server::wrapper::Parameters;

    // Arrange: Create database and skill for testing
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate_async().await.expect("Failed to run migrations");

    let skill = Skill {
        id: String::new(),
        name: "test-skill".to_string(),
        description: "Test skill".to_string(),
        content: r#"---
name: test-skill
description: Test skill
---

# Test Skill
"#
        .to_string(),
        tags: vec!["old-tag".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: None,
        updated_at: None,
    };
    let created_skill = db.skills().create(&skill).await.unwrap();

    let temp_dir = TempDir::new().unwrap();
    let server =
        super::server::McpServer::new(db, ChangeNotifier::new(), temp_dir.path().join("skills"));

    // Act: Call update_skill via server (this will fail until tool is registered)
    let params = UpdateSkillParams {
        skill_id: created_skill.id.clone(),
        tags: Some(vec!["new-tag".to_string()]),
        project_ids: None,
    };

    let result = server.update_skill(Parameters(params)).await;

    // Assert: Tool should be callable and succeed
    assert!(
        result.is_ok(),
        "update_skill tool should be registered and callable"
    );
}
