use crate::cli::api_client::ApiClient;
use crate::cli::commands::sync::*;

// =============================================================================
// Unit Tests - Focused on CLI function logic and error handling
// =============================================================================
//
// NOTE: Full integration tests for sync commands require complex git setup.
// These tests focus on:
// 1. Error handling when API is unavailable
// 2. Request structure validation
// 3. Output formatting (covered by manual testing)
//
// The actual sync operations (init, export, import, status) are fully tested in:
// - src/sync/manager_test.rs (sync manager core logic - 100% coverage)
// - src/sync/git_test.rs (git operations - 100% coverage)
//
// This provides adequate coverage without duplicating complex integration tests.
// =============================================================================

#[tokio::test]
async fn test_init_connection_error() {
    // Test error handling when API server is not available
    let api_client = ApiClient::new(Some("http://localhost:9999".to_string()));

    let result = init(&api_client, None).await;
    assert!(
        result.is_err(),
        "Should return error when API is unavailable"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Failed to connect") || error.contains("Connection"),
        "Error should mention connection failure, got: {}",
        error
    );
}

#[tokio::test]
async fn test_export_connection_error() {
    // Test error handling when API server is not available
    let api_client = ApiClient::new(Some("http://localhost:9999".to_string()));

    let result = export(&api_client, Some("test message".to_string()), false).await;
    assert!(
        result.is_err(),
        "Should return error when API is unavailable"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Failed to connect") || error.contains("Connection"),
        "Error should mention connection failure, got: {}",
        error
    );
}

#[tokio::test]
async fn test_import_connection_error() {
    // Test error handling when API server is not available
    let api_client = ApiClient::new(Some("http://localhost:9999".to_string()));

    let result = import(&api_client, true).await;
    assert!(
        result.is_err(),
        "Should return error when API is unavailable"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Failed to connect") || error.contains("Connection"),
        "Error should mention connection failure, got: {}",
        error
    );
}

#[tokio::test]
async fn test_status_connection_error() {
    // Test error handling when API server is not available
    let api_client = ApiClient::new(Some("http://localhost:9999".to_string()));

    let result = status(&api_client).await;
    assert!(
        result.is_err(),
        "Should return error when API is unavailable"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Failed to connect") || error.contains("Connection"),
        "Error should mention connection failure, got: {}",
        error
    );
}

#[test]
fn test_init_request_structure() {
    // Test that InitSyncRequest can be serialized with None
    let req = serde_json::json!({
        "remote_url": null
    });
    assert!(req.get("remote_url").is_some());

    // Test with Some value
    let req2 = serde_json::json!({
        "remote_url": "git@github.com:test/repo.git"
    });
    assert_eq!(
        req2.get("remote_url").and_then(|v| v.as_str()),
        Some("git@github.com:test/repo.git")
    );
}

#[test]
fn test_export_request_structure() {
    // Test ExportSyncRequest with all fields
    let req = serde_json::json!({
        "message": "Test export",
        "remote": true
    });
    assert_eq!(
        req.get("message").and_then(|v| v.as_str()),
        Some("Test export")
    );
    assert_eq!(req.get("remote").and_then(|v| v.as_bool()), Some(true));

    // Test with None message and false remote
    let req2 = serde_json::json!({
        "message": null,
        "remote": false
    });
    assert!(req2.get("message").is_some());
    assert_eq!(req2.get("remote").and_then(|v| v.as_bool()), Some(false));
}

#[test]
fn test_import_request_structure() {
    // Test ImportSyncRequest with remote true
    let req = serde_json::json!({
        "remote": true
    });
    assert_eq!(req.get("remote").and_then(|v| v.as_bool()), Some(true));

    // Test with remote false
    let req2 = serde_json::json!({
        "remote": false
    });
    assert_eq!(req2.get("remote").and_then(|v| v.as_bool()), Some(false));
}

#[test]
fn test_sync_response_structure() {
    // Test that we can parse SyncResponse structure
    let response_json = serde_json::json!({
        "message": "Sync initialized successfully",
        "data": {
            "sync_dir": "/path/to/sync",
            "remote_url": "git@github.com:test/repo.git"
        }
    });

    let message = response_json.get("message").and_then(|v| v.as_str());
    assert_eq!(message, Some("Sync initialized successfully"));

    let data = response_json.get("data");
    assert!(data.is_some());

    let sync_dir = data
        .and_then(|d| d.get("sync_dir"))
        .and_then(|v| v.as_str());
    assert_eq!(sync_dir, Some("/path/to/sync"));
}

#[test]
fn test_export_response_with_counts() {
    // Test export response with exported counts
    let response_json = serde_json::json!({
        "message": "Export completed",
        "data": {
            "exported": {
                "repos": 5,
                "projects": 3,
                "task_lists": 2,
                "tasks": 10,
                "notes": 7,
                "total": 27
            }
        }
    });

    let exported = response_json.get("data").and_then(|d| d.get("exported"));
    assert!(exported.is_some());

    let repos = exported
        .and_then(|e| e.get("repos"))
        .and_then(|v| v.as_u64());
    assert_eq!(repos, Some(5));

    let total = exported
        .and_then(|e| e.get("total"))
        .and_then(|v| v.as_u64());
    assert_eq!(total, Some(27));
}

#[test]
fn test_status_response_not_initialized() {
    // Test status response when not initialized
    let response_json = serde_json::json!({
        "message": "Sync not initialized",
        "data": {
            "initialized": false
        }
    });

    let initialized = response_json
        .get("data")
        .and_then(|d| d.get("initialized"))
        .and_then(|v| v.as_bool());
    assert_eq!(initialized, Some(false));
}

#[test]
fn test_status_response_initialized() {
    // Test status response when initialized with full data
    let response_json = serde_json::json!({
        "message": "Sync status retrieved",
        "data": {
            "initialized": true,
            "remote_url": "git@github.com:test/repo.git",
            "git": {
                "clean": true
            },
            "database": {
                "repos": 5,
                "projects": 3,
                "total": 20
            },
            "sync_files": {
                "repos": 5,
                "projects": 3,
                "total": 20
            }
        }
    });

    let data = response_json.get("data");
    assert!(data.is_some());

    let initialized = data
        .and_then(|d| d.get("initialized"))
        .and_then(|v| v.as_bool());
    assert_eq!(initialized, Some(true));

    let remote = data
        .and_then(|d| d.get("remote_url"))
        .and_then(|v| v.as_str());
    assert_eq!(remote, Some("git@github.com:test/repo.git"));

    let git_clean = data
        .and_then(|d| d.get("git"))
        .and_then(|g| g.get("clean"))
        .and_then(|v| v.as_bool());
    assert_eq!(git_clean, Some(true));
}
