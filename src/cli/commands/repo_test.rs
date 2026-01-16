use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::repo::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use tokio::net::TcpListener;

/// Spawn a test HTTP server with in-memory database
async fn spawn_test_server() -> (String, tokio::task::JoinHandle<()>) {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
    );
    let app = routes::create_router(state, false);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (url, handle)
}

#[tokio::test]
async fn test_delete_repo_without_force() {
    // Test the --force flag validation (pure logic, no HTTP needed)
    let api_client = ApiClient::new(None);
    let result = delete_repo(&api_client, "test-id", false).await;

    assert!(result.is_err());
    if let Err(e) = result {
        let error_msg = e.to_string();
        assert!(
            error_msg.contains("--force"),
            "Error should mention --force flag"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_repos() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    let result = list_repos(&api_client, None, None, None, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 0); // Initially empty
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_and_get_repo() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create
    let create_result = create_repo(&api_client, "https://github.com/test/repo", None, None).await;
    assert!(create_result.is_ok());

    let output = create_result.unwrap();
    assert!(output.contains("Created repository"));

    // Extract ID from output (contains ID in message)
    // For now just verify list shows the repo
    let list_result = list_repos(&api_client, None, None, None, "json").await;
    assert!(list_result.is_ok());

    let output = list_result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 1);
}

// =============================================================================
// Unhappy Path Tests - NOT FOUND Errors
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_get_repo_not_found() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to get non-existent repo
    let result = get_repo(&api_client, "nonexist", "json").await;

    // Should return error (might be decode error or 404)
    assert!(result.is_err(), "Should return error for non-existent repo");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_repo_not_found() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to update non-existent repo
    let result = update_repo(
        &api_client,
        "nonexist",
        Some("https://github.com/test/new"),
        None,
        None,
    )
    .await;

    // Should return error
    assert!(result.is_err(), "Should return error for non-existent repo");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_repo_not_found_with_force() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to delete non-existent repo with --force
    let result = delete_repo(&api_client, "nonexist", true).await;

    // Should return error
    assert!(result.is_err(), "Should return error for non-existent repo");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

// =============================================================================
// Unhappy Path Tests - Validation Errors
// =============================================================================

// NOTE: The following validation tests are NOT included because the API does not validate these cases:
// - test_create_repo_empty_remote: API might allow empty remote URLs (no validation at HTTP API layer)
// - test_create_repo_invalid_remote_format: API likely doesn't validate URL format
