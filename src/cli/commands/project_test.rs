use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::project::*;
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

    let result = list_projects(&api_client, None, None, None, "json").await;
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
    )
    .await;
    assert!(create_result.is_ok());

    let output = create_result.unwrap();
    assert!(output.contains("Created project"));

    // List shows our new project
    let list_result = list_projects(&api_client, None, None, None, "json").await;
    assert!(list_result.is_ok());

    let output = list_result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 1); // Just Test Project
}
