use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::task_list::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use tokio::net::TcpListener;

// =============================================================================
// Integration Tests - Test CLI commands against real HTTP server
// =============================================================================

/// Spawn a test HTTP server with in-memory database
async fn spawn_test_server() -> (String, String, tokio::task::JoinHandle<()>) {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

    // Create test project
    let project_id = sqlx::query_scalar::<_, String>(
        "INSERT INTO project (id, title, description, tags, created_at, updated_at) 
         VALUES ('test0000', 'Test Project', 'Test project for CLI tests', '[]', datetime('now'), datetime('now')) 
         RETURNING id"
    )
    .fetch_one(db.pool())
    .await
    .expect("Failed to create test project");

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

    (url, project_id, handle)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_list_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list
    let result = create_task_list(
        &api_client,
        "Integration Test List",
        &project_id,
        Some("Test description"),
        Some("testing,integration"),
        None,
    )
    .await;

    assert!(result.is_ok());
    let output = result.unwrap();

    // Output is success message
    assert!(output.contains("Integration Test List"));
    assert!(output.contains("Created task list"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create two task lists
    create_task_list(&api_client, "List 1", &project_id, None, Some("tag1"), None)
        .await
        .expect("Failed to create list 1");
    create_task_list(&api_client, "List 2", &project_id, None, Some("tag2"), None)
        .await
        .expect("Failed to create list 2");

    // List task lists
    let result = list_task_lists(&api_client, None, None, None, None, None, None, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("Failed to parse JSON");

    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["title"], "List 1");
    assert_eq!(parsed[1]["title"], "List 2");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_list_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create task list
    let create_result = create_task_list(
        &api_client,
        "Test List",
        &project_id,
        Some("Description"),
        None,
        None,
    )
    .await
    .expect("Failed to create task list");

    // Extract list ID from success message
    let list_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract list ID");

    // Get task list
    let result = get_task_list(&api_client, list_id, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let task_list: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(task_list["title"], "Test List");
    assert_eq!(task_list["description"], "Description");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_list_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create task list
    let create_result = create_task_list(
        &api_client,
        "Original Title",
        &project_id,
        Some("Original desc"),
        Some("tag1"),
        None,
    )
    .await
    .expect("Failed to create task list");

    let list_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract list ID");

    // Update task list
    let result = update_task_list(
        &api_client,
        list_id,
        Some("Updated Title"),
        Some("Updated description"),
        None,
        Some("tag1,tag2"),
    )
    .await;
    assert!(result.is_ok());

    // Verify updates
    let get_result = get_task_list(&api_client, list_id, "json")
        .await
        .expect("Failed to get task list");
    let updated_list: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_list["title"], "Updated Title");
    assert_eq!(updated_list["description"], "Updated description");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_list_stats_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create task list
    let create_result = create_task_list(
        &api_client,
        "Stats Test List",
        &project_id,
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create task list");

    let list_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract list ID");

    // Create some tasks in the list
    crate::cli::commands::task::create_task(
        &api_client,
        list_id,
        "Task 1",
        None,
        Some(1),
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create task 1");

    crate::cli::commands::task::create_task(
        &api_client,
        list_id,
        "Task 2",
        None,
        Some(2),
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create task 2");

    // Get stats
    let result = get_task_list_stats(&api_client, list_id, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let stats: serde_json::Value = serde_json::from_str(&output).unwrap();

    // Both tasks should be in backlog status by default
    assert_eq!(stats["total"], 2);
    assert_eq!(stats["backlog"], 2);
}
