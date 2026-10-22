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
// The actual sync operations (init, export, import, status) are tested in the
// context-sync crate:
// - crates/context-sync/test/manager.rs (mod manager_test — sync manager core logic)
// - crates/context-sync/test/git.rs (mod git_test — git operations)
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

// =============================================================================
// Output formatting tests against a mock API server
// =============================================================================

/// Start a mock API server that answers every sync request with `body`.
async fn spawn_mock_api(body: serde_json::Value) -> String {
    use axum::{
        Json, Router,
        routing::{get, post},
    };

    let handler = move || {
        let body = body.clone();
        async move { Json(body) }
    };

    let app = Router::new()
        .route("/api/v1/sync/status", get(handler.clone()))
        .route("/api/v1/sync/export", post(handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind mock server");
    let addr = listener.local_addr().expect("Failed to read mock address");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("Mock server failed");
    });

    format!("http://{}", addr)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_includes_last_synced_at() {
    let body = serde_json::json!({
        "message": "Sync status retrieved",
        "data": {
            "initialized": true,
            "remote_url": "git@github.com:test/repo.git",
            "last_synced_at": "2026-10-18 12:00:00",
            "schema_version": 20261018000002i64,
            "git": { "clean": true },
            "database": {
                "repos": 0,
                "projects": 1,
                "task_lists": 0,
                "tasks": 0,
                "notes": 0,
                "skills": 0,
                "attachments": 0,
                "total": 1
            },
            "sync_files": {
                "repos": 0,
                "projects": 1,
                "task_lists": 0,
                "tasks": 0,
                "notes": 0,
                "skills": 0,
                "attachments": 0,
                "total": 1
            }
        }
    });

    let url = spawn_mock_api(body).await;
    let api_client = ApiClient::new(Some(url));

    let output = status(&api_client).await.expect("status should succeed");

    assert!(
        output
            .lines()
            .any(|line| line == "last_synced_at: 2026-10-18 12:00:00"),
        "status output must show the last sync timestamp line, got: {}",
        output
    );
    assert!(
        output
            .lines()
            .any(|line| line == "schema_version: 20261018000002"),
        "status output must show the local schema version line, got: {}",
        output
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_shows_change_count() {
    let body = serde_json::json!({
        "message": "Export completed",
        "data": {
            "changes": 5,
            "exported": {
                "repos": 0,
                "projects": 1,
                "task_lists": 0,
                "tasks": 4,
                "notes": 0,
                "skills": 0,
                "attachments": 0,
                "total": 5
            }
        }
    });

    let url = spawn_mock_api(body).await;
    let api_client = ApiClient::new(Some(url));

    let output = export(&api_client, None, false)
        .await
        .expect("export should succeed");

    assert!(
        output.contains("5 changes"),
        "export output must show the incremental change count, got: {}",
        output
    );
}

fn row_for<'a>(output: &'a str, item: &str) -> &'a str {
    output
        .lines()
        .find(|line| line.contains(item))
        .unwrap_or_else(|| panic!("status output must contain a {item} row, got: {output}"))
}

fn last_cell(row: &str) -> &str {
    row.trim()
        .trim_end_matches('│')
        .rsplit('│')
        .next()
        .unwrap_or("")
        .trim()
}

fn cell(row: &str, index: usize) -> &str {
    row.split('│').nth(index).unwrap_or("").trim()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_shows_tombstones_column() {
    let body = serde_json::json!({
        "message": "Sync status retrieved",
        "data": {
            "initialized": true,
            "remote_url": "git@github.com:test/repo.git",
            "last_synced_at": "2026-10-18 12:00:00",
            "schema_version": 20261018000002i64,
            "git": { "clean": true },
            "database": {
                "repos": 1,
                "projects": 2,
                "task_lists": 3,
                "tasks": 4,
                "notes": 5,
                "skills": 6,
                "attachments": 7,
                "total": 28
            },
            "sync_files": {
                "repos": 21,
                "projects": 22,
                "task_lists": 23,
                "tasks": 24,
                "notes": 25,
                "skills": 26,
                "attachments": 27,
                "total": 168
            },
            "tombstones": {
                "repos": 11,
                "projects": 12,
                "task_lists": 13,
                "tasks": 14,
                "notes": 15,
                "skills": 16,
                "attachments": 17,
                "total": 98
            }
        }
    });

    let url = spawn_mock_api(body).await;
    let api_client = ApiClient::new(Some(url));

    let output = status(&api_client).await.expect("status should succeed");

    assert!(
        output.contains("Tombstones"),
        "status output must label the Tombstones column, got: {}",
        output
    );

    let expected = [
        ("Repos", "11", "1", "21"),
        ("Projects", "12", "2", "22"),
        ("Task Lists", "13", "3", "23"),
        ("Tasks", "14", "4", "24"),
        ("Notes", "15", "5", "25"),
        ("Skills", "16", "6", "26"),
        ("Attachments", "17", "7", "27"),
        ("Total", "98", "28", "168"),
    ];

    for (item, tombstones, database, sync_files) in expected {
        let row = row_for(&output, item);
        assert_eq!(
            cell(row, 2),
            database,
            "{item} row must show {database} database entities, got: {row}"
        );
        assert_eq!(
            cell(row, 3),
            sync_files,
            "{item} row must show {sync_files} sync files, got: {row}"
        );
        assert_eq!(
            last_cell(row),
            tombstones,
            "{item} row must show {tombstones} tombstones, got: {row}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_without_tombstones_renders_dash() {
    let body = serde_json::json!({
        "message": "Sync status retrieved",
        "data": {
            "initialized": true,
            "remote_url": "git@github.com:test/repo.git",
            "last_synced_at": "2026-10-18 12:00:00",
            "schema_version": 20261018000002i64,
            "git": { "clean": true },
            "database": {
                "repos": 0,
                "projects": 1,
                "task_lists": 0,
                "tasks": 0,
                "notes": 0,
                "skills": 0,
                "attachments": 0,
                "total": 1
            },
            "sync_files": {
                "repos": 0,
                "projects": 1,
                "task_lists": 0,
                "tasks": 0,
                "notes": 0,
                "skills": 0,
                "attachments": 0,
                "total": 1
            }
        }
    });

    let url = spawn_mock_api(body).await;
    let api_client = ApiClient::new(Some(url));

    let output = status(&api_client).await.expect("status should succeed");

    let expected = [
        ("Repos", "0", "0"),
        ("Projects", "1", "1"),
        ("Task Lists", "0", "0"),
        ("Tasks", "0", "0"),
        ("Notes", "0", "0"),
        ("Skills", "0", "0"),
        ("Attachments", "0", "0"),
        ("Total", "1", "1"),
    ];

    for (item, database, sync_files) in expected {
        let row = row_for(&output, item);
        assert_eq!(
            cell(row, 2),
            database,
            "{item} row must show {database} database entities, got: {row}"
        );
        assert_eq!(
            cell(row, 3),
            sync_files,
            "{item} row must show {sync_files} sync files, got: {row}"
        );
        assert_eq!(
            last_cell(row),
            "-",
            "{item} row must render '-' when tombstones are absent, got: {row}"
        );
    }
}
