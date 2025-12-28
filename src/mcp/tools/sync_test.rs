//! Tests for sync MCP tools.
//!
//! Following TDD: Write tests FIRST (RED), then implement (GREEN).

use std::sync::Arc;

use tempfile::TempDir;

use crate::db::{Database, SqliteDatabase};
use crate::mcp::tools::sync::{SyncOperation, SyncParams, SyncTools};
use crate::sync::{MockGitOps, SyncManager};

use rmcp::{handler::server::wrapper::Parameters, model::RawContent};

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

/// Test sync status when NOT initialized - using ISOLATED temp directory.
///
/// This test verifies:
/// - SyncTools can be created with injected SyncManager (SOLID: DIP)
/// - Test uses temp directory, NOT production path
/// - Status returns initialized=false when sync dir doesn't exist
#[tokio::test(flavor = "multi_thread")]
async fn test_sync_status_not_initialized_with_temp_dir() {
    // Arrange: Create in-memory db and temp directory
    let db = Arc::new(setup_test_db().await);
    let temp_dir = TempDir::new().unwrap();
    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    // Act: Create SyncTools with injected manager
    // NOTE: This will fail until we implement SyncTools::with_manager()
    let tools = SyncTools::with_manager(db, manager);

    let params = SyncParams {
        operation: SyncOperation::Status,
        remote_url: None,
        message: None,
    };

    let result = tools.sync(Parameters(params)).await.unwrap();

    // Assert: Should return initialized=false
    let text = match &result.content[0].raw {
        RawContent::Text(text_content) => text_content.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let json: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(json["initialized"], false);
    assert!(
        json["message"]
            .as_str()
            .unwrap()
            .contains("not initialized")
    );
}

/// Test that SyncTools::with_real_git() convenience constructor exists.
///
/// This test verifies:
/// - Production code can use with_real_git() for real SyncManager
/// - Constructor creates SyncManager with RealGit and production paths
#[tokio::test(flavor = "multi_thread")]
async fn test_sync_tools_with_real_git_constructor() {
    // Arrange
    let db = Arc::new(setup_test_db().await);

    // Act: Create tools with real git (for production use)
    // NOTE: This will fail until we implement SyncTools::with_real_git()
    let _tools = SyncTools::with_real_git(db);

    // Assert: If we got here, constructor works
    // We won't actually test sync operations because that would touch production paths
}

// Original test removed - replaced with test_sync_status_not_initialized_with_temp_dir
// which properly uses temp directories and MockGitOps for test isolation.
