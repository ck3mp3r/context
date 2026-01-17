use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::project::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use serde_json::json;
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
async fn test_delete_project_without_force() {
    // Test that delete without --force flag is rejected (pure logic, no HTTP needed)
    let api_client = ApiClient::new(None);
    let result = delete_project(&api_client, "test-id", false).await;

    // Should return an error about requiring --force
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
async fn test_list_projects() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    let result = list_projects(&api_client, None, None, None, None, None, None, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed.is_array(), "Output should be an array");
    // No default project in migrations, expect empty list
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_and_get_project() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create
    let create_result = create_project(
        &api_client,
        "Test Project",
        Some("Test desc"),
        Some("tag1,tag2"),
        None,
    )
    .await;
    assert!(create_result.is_ok());

    let output = create_result.unwrap();
    assert!(output.contains("Created project"));

    // List shows our new project
    let list_result = list_projects(&api_client, None, None, None, None, None, None, "json").await;
    assert!(list_result.is_ok());

    let output = list_result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 1); // Just Test Project
}

// =============================================================================
// External Reference Support
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_external_refs() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create project with external_refs
    let create_result = create_project(
        &api_client,
        "GitHub Project",
        Some("Linked to GitHub issue"),
        None,
        Some("owner/repo#123"),
    )
    .await;
    assert!(create_result.is_ok());

    let output = create_result.unwrap();
    assert!(output.contains("Created project"));

    // List and verify external_refs is present
    let list_result = list_projects(&api_client, None, None, None, None, None, None, "json").await;
    assert!(list_result.is_ok());

    let output = list_result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let projects = parsed.as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["external_refs"], json!(["owner/repo#123"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_project_external_refs() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create project without external_refs
    let create_result = create_project(
        &api_client,
        "Project Without Ref",
        Some("No external ref yet"),
        None,
        None,
    )
    .await;
    assert!(create_result.is_ok());

    // Get project ID from list
    let list_result = list_projects(&api_client, None, None, None, None, None, None, "json").await;
    let output = list_result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let project_id = parsed[0]["id"].as_str().unwrap();

    // Update to add external_refs
    let update_result = update_project(
        &api_client,
        project_id,
        Some("Project With Ref"),
        None,
        None,
        Some("JIRA-456"),
    )
    .await;
    assert!(update_result.is_ok());

    // Verify external_refs was added
    let list_result = list_projects(&api_client, None, None, None, None, None, None, "json").await;
    let output = list_result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed[0]["external_refs"], json!(["JIRA-456"]));
}

// =============================================================================
// Unhappy Path Tests - NOT FOUND Errors
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_get_project_not_found() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to get non-existent project
    let result = get_project(&api_client, "nonexist", "json").await;

    // Should return error (might be decode error or 404)
    assert!(
        result.is_err(),
        "Should return error for non-existent project"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_project_not_found() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to update non-existent project
    let result = update_project(
        &api_client,
        "nonexist",
        Some("New Title"),
        Some("New desc"),
        None,
        None,
    )
    .await;

    // Should return error
    assert!(
        result.is_err(),
        "Should return error for non-existent project"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_project_not_found_with_force() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to delete non-existent project with --force
    let result = delete_project(&api_client, "nonexist", true).await;

    // Should return error
    assert!(
        result.is_err(),
        "Should return error for non-existent project"
    );
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
// - test_create_project_empty_title: API allows empty titles (no validation at HTTP API layer)

// =============================================================================
// List Parameters Tests (offset, sort, order)
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_offset() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create 3 projects
    for i in 1..=3 {
        let result = create_project(
            &api_client,
            &format!("Project {}", i),
            Some(&format!("Description {}", i)),
            None,
            None,
        )
        .await;
        assert!(
            result.is_ok(),
            "Failed to create project {}: {:?}",
            i,
            result
        );
    }

    // List with offset=1 (skip first project)
    let result = list_projects(&api_client, None, None, None, Some(1), None, None, "json").await;
    assert!(result.is_ok(), "List with offset should succeed");

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        parsed.as_array().unwrap().len(),
        2,
        "Should return 2 projects after skipping 1"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_sort_and_order() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create projects with different titles
    let _ = create_project(&api_client, "Zebra Project", None, None, None).await;
    let _ = create_project(&api_client, "Alpha Project", None, None, None).await;
    let _ = create_project(&api_client, "Beta Project", None, None, None).await;

    // List sorted by title ascending
    let result = list_projects(
        &api_client,
        None,
        None,
        None,
        None,
        Some("title"),
        Some("asc"),
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let projects = parsed.as_array().unwrap();

    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0]["title"], "Alpha Project");
    assert_eq!(projects[1]["title"], "Beta Project");
    assert_eq!(projects[2]["title"], "Zebra Project");

    // List sorted by title descending
    let result = list_projects(
        &api_client,
        None,
        None,
        None,
        None,
        Some("title"),
        Some("desc"),
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let projects = parsed.as_array().unwrap();

    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0]["title"], "Zebra Project");
    assert_eq!(projects[1]["title"], "Beta Project");
    assert_eq!(projects[2]["title"], "Alpha Project");
}
