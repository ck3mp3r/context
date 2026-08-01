//! Tests for sync MCP tools.
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use std::sync::Arc;

use tempfile::TempDir;

use context_db::SqliteDatabase;
use context_server::mcp::tools::sync::{SyncParams, SyncTools};
use context_sync::{MockGitOps, SyncManager};

use rmcp::{handler::server::wrapper::Parameters, model::ContentBlock};

async fn setup_test_db() -> SqliteDatabase {
    common::setup_db().await
}

/// Test sync status when NOT initialized - using ISOLATED temp directory.
#[tokio::test(flavor = "multi_thread")]
async fn test_sync_status_not_initialized_with_temp_dir() {
    let db = Arc::new(setup_test_db().await);
    let temp_dir = TempDir::new().unwrap();
    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    let tools = SyncTools::with_manager(db, manager);

    let params = SyncParams {
        operation: "status".to_string(),
        remote_url: None,
        message: None,
        remote: None,
    };

    let result = tools.sync(Parameters(params)).await.unwrap();

    let text = match &result.content[0] {
        ContentBlock::Text(text_content) => text_content.text.as_str(),
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
#[tokio::test(flavor = "multi_thread")]
async fn test_sync_tools_with_real_git_constructor() {
    let db = Arc::new(setup_test_db().await);

    let _tools = SyncTools::with_real_git(db);
}

/// Test that invalid operation strings return proper error.
#[tokio::test(flavor = "multi_thread")]
async fn test_sync_invalid_operation_error() {
    let db = Arc::new(setup_test_db().await);
    let temp_dir = TempDir::new().unwrap();
    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let tools = SyncTools::with_manager(db, manager);

    let params = SyncParams {
        operation: "invalid_operation".to_string(),
        remote_url: None,
        message: None,
        remote: None,
    };

    let result = tools.sync(Parameters(params)).await;

    assert!(result.is_err());
    let err = result.unwrap_err();

    let err_msg = format!("{:?}", err);
    assert!(
        err_msg.contains("invalid_operation") || err_msg.contains("Invalid operation"),
        "Error should mention invalid operation. Got: {}",
        err_msg
    );
}
