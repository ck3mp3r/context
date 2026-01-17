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

    let result = list_repos(&api_client, None, None, None, None, None, "json").await;
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
    let create_result = create_repo(
        &api_client,
        "https://github.com/test/repo",
        None,
        None,
        None,
    )
    .await;
    assert!(create_result.is_ok());

    let output = create_result.unwrap();
    assert!(output.contains("Created repository"));

    // Extract ID from output (contains ID in message)
    // For now just verify list shows the repo
    let list_result = list_repos(&api_client, None, None, None, None, None, "json").await;
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

// =============================================================================
// project_ids Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_repo_with_project_ids() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // First, create a project to link to
    let project_payload = serde_json::json!({
        "title": "Test Project for Repo Linking"
    });
    let project_response = api_client
        .post("/api/v1/projects")
        .json(&project_payload)
        .send()
        .await
        .expect("Failed to create project");
    let project: serde_json::Value = project_response.json().await.unwrap();
    let project_id = project["id"].as_str().unwrap();

    // Create repo with project_ids
    let create_result = create_repo(
        &api_client,
        "https://github.com/test/repo-with-projects",
        None,
        None,
        Some(project_id), // project_ids parameter
    )
    .await;
    assert!(create_result.is_ok(), "Should create repo with project_ids");

    // Get the repo ID from the create response
    let output = create_result.unwrap();
    // Extract ID from message like "✓ Created repository: https://github.com/test/repo-with-projects (abc123)"
    let repo_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract repo ID from create output");

    // Verify the repo has project_ids by fetching it with get (not list, since list doesn't load M:N relationships)
    let get_result = get_repo(&api_client, repo_id, "json").await;
    assert!(get_result.is_ok());

    let repo: Repo = serde_json::from_str(&get_result.unwrap()).unwrap();
    assert_eq!(repo.project_ids.len(), 1);
    assert_eq!(repo.project_ids[0], project_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_repo_with_project_ids() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create a repo without project_ids
    let create_result = create_repo(
        &api_client,
        "https://github.com/test/repo-to-update",
        None,
        None,
        None,
    )
    .await;
    assert!(create_result.is_ok());

    // Get the repo ID from the create response
    let output = create_result.unwrap();
    let repo_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract repo ID from create output");

    // Create a project to link
    let project_payload = serde_json::json!({
        "title": "Project for Update Test"
    });
    let project_response = api_client
        .post("/api/v1/projects")
        .json(&project_payload)
        .send()
        .await
        .expect("Failed to create project");
    let project: serde_json::Value = project_response.json().await.unwrap();
    let project_id = project["id"].as_str().unwrap();

    // Update repo to add project_ids
    let update_result = update_repo(
        &api_client,
        repo_id,
        None,
        None,
        None,
        Some(project_id), // project_ids parameter
    )
    .await;
    assert!(update_result.is_ok(), "Should update repo with project_ids");

    // Verify the update
    let get_result = get_repo(&api_client, repo_id, "json").await;
    assert!(get_result.is_ok());
    let repo: Repo = serde_json::from_str(&get_result.unwrap()).unwrap();
    assert_eq!(repo.project_ids.len(), 1);
    assert_eq!(repo.project_ids[0], project_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_repo_with_multiple_project_ids() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create two projects
    let project1_payload = serde_json::json!({"title": "Project 1"});
    let project1_response = api_client
        .post("/api/v1/projects")
        .json(&project1_payload)
        .send()
        .await
        .expect("Failed to create project 1");
    let project1: serde_json::Value = project1_response.json().await.unwrap();
    let project1_id = project1["id"].as_str().unwrap();

    let project2_payload = serde_json::json!({"title": "Project 2"});
    let project2_response = api_client
        .post("/api/v1/projects")
        .json(&project2_payload)
        .send()
        .await
        .expect("Failed to create project 2");
    let project2: serde_json::Value = project2_response.json().await.unwrap();
    let project2_id = project2["id"].as_str().unwrap();

    // Create repo with multiple project_ids (comma-separated)
    let project_ids_str = format!("{},{}", project1_id, project2_id);
    let create_result = create_repo(
        &api_client,
        "https://github.com/test/multi-project-repo",
        None,
        None,
        Some(&project_ids_str),
    )
    .await;
    assert!(
        create_result.is_ok(),
        "Should create repo with multiple project_ids"
    );

    // Get the repo ID from the create response
    let output = create_result.unwrap();
    let repo_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract repo ID from create output");

    // Verify both projects are linked
    let get_result = get_repo(&api_client, repo_id, "json").await;
    assert!(get_result.is_ok());
    let repo: Repo = serde_json::from_str(&get_result.unwrap()).unwrap();
    assert_eq!(repo.project_ids.len(), 2);
    assert!(repo.project_ids.contains(&project1_id.to_string()));
    assert!(repo.project_ids.contains(&project2_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_repos_with_offset() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create 3 repos
    for i in 1..=3 {
        let _ = create_repo(
            &api_client,
            &format!("https://github.com/test/repo{}", i),
            None,
            None,
            None,
        )
        .await;
    }

    // List with offset=1 (skip first repo)
    let result = list_repos(&api_client, None, None, Some(1), None, None, "json").await;
    assert!(result.is_ok(), "List with offset should succeed");

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        parsed.as_array().unwrap().len(),
        2,
        "Should return 2 repos after skipping 1"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_repos_with_sort_and_order() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create repos with different remote URLs
    let _ = create_repo(&api_client, "https://github.com/z/zebra", None, None, None).await;
    let _ = create_repo(&api_client, "https://github.com/a/alpha", None, None, None).await;
    let _ = create_repo(&api_client, "https://github.com/b/beta", None, None, None).await;

    // List sorted by remote ascending
    let result = list_repos(
        &api_client,
        None,
        None,
        None,
        Some("remote"),
        Some("asc"),
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let repos = parsed.as_array().unwrap();

    assert_eq!(repos.len(), 3);
    assert!(repos[0]["remote"].as_str().unwrap().contains("alpha"));
    assert!(repos[1]["remote"].as_str().unwrap().contains("beta"));
    assert!(repos[2]["remote"].as_str().unwrap().contains("zebra"));

    // List sorted by remote descending
    let result = list_repos(
        &api_client,
        None,
        None,
        None,
        Some("remote"),
        Some("desc"),
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let repos = parsed.as_array().unwrap();

    assert_eq!(repos.len(), 3);
    assert!(repos[0]["remote"].as_str().unwrap().contains("zebra"));
    assert!(repos[1]["remote"].as_str().unwrap().contains("beta"));
    assert!(repos[2]["remote"].as_str().unwrap().contains("alpha"));
}
