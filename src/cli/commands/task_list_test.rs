use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::PageParams;
use crate::cli::commands::task_list::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use serde_json::json;
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

    // Create task list with description and tags
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

    // Extract list ID from success message: "✓ Created task list: Title (list_id)"
    let list_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract list ID");

    // Verify all fields were persisted correctly by fetching the task list
    let get_result = get_task_list(&api_client, list_id, "json")
        .await
        .expect("Failed to get task list");
    let created_list: serde_json::Value = serde_json::from_str(&get_result).unwrap();

    assert_eq!(created_list["title"], "Integration Test List");
    assert_eq!(created_list["description"], "Test description");
    assert_eq!(created_list["tags"], json!(["testing", "integration"]));
    assert_eq!(created_list["project_id"], project_id);
    assert_eq!(created_list["status"], "active");
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
    let result = list_task_lists(
        &api_client,
        None,
        None,
        None,
        None,
        PageParams::default(),
        "json",
    )
    .await;
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

// =============================================================================
// Unhappy Path Tests - NOT FOUND Errors
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_list_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to get non-existent task_list
    let result = get_task_list(&api_client, "nonexist", "json").await;

    // Should return error with not found message
    assert!(
        result.is_err(),
        "Should return error for non-existent task_list"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_list_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to update non-existent task_list
    let result = update_task_list(
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
        "Should return error for non-existent task_list"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

// NOTE: Stats endpoint does not return 404 for nonexistent lists (returns empty stats instead)
// - test_get_task_list_stats_not_found: Not included - API returns empty/zero stats instead of error

// =============================================================================
// Unhappy Path Tests - Validation Errors
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_list_with_nonexistent_project_id() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to create task_list with non-existent project_id
    let result =
        create_task_list(&api_client, "Task List Title", "nonexist", None, None, None).await;

    // Should return error (foreign key constraint)
    assert!(
        result.is_err(),
        "Should return error for non-existent project_id"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found")
            || error.contains("404")
            || error.contains("project")
            || error.contains("foreign key")
            || error.contains("constraint"),
        "Error should mention project or foreign key, got: {}",
        error
    );
}

// NOTE: The following validation tests are NOT included because the API does not validate these cases:
// - test_create_task_list_empty_title: API allows empty titles (no validation at HTTP API layer)
// - test_create_task_list_without_project_id: This is a CLI-level check - API requires project_id field,
//   but CLI always provides it (might be empty string which API would accept)

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_with_offset() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create 3 task lists
    for i in 1..=3 {
        let _ = create_task_list(
            &api_client,
            &format!("List {}", i),
            &project_id,
            None,
            None,
            None,
        )
        .await;
    }

    // List with offset=1 (skip first list)
    let page = PageParams {
        limit: None,
        offset: Some(1),
        sort: None,
        order: None,
    };
    let result = list_task_lists(&api_client, None, None, None, None, page, "json").await;
    assert!(result.is_ok(), "List with offset should succeed");

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        parsed.as_array().unwrap().len(),
        2,
        "Should return 2 lists after skipping 1"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_with_sort_and_order() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create lists with different titles
    let _ = create_task_list(&api_client, "Zebra List", &project_id, None, None, None).await;
    let _ = create_task_list(&api_client, "Alpha List", &project_id, None, None, None).await;
    let _ = create_task_list(&api_client, "Beta List", &project_id, None, None, None).await;

    // List sorted by title ascending
    let page = PageParams {
        limit: None,
        offset: None,
        sort: Some("title"),
        order: Some("asc"),
    };
    let result = list_task_lists(&api_client, None, None, None, None, page, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let lists = parsed.as_array().unwrap();

    assert_eq!(lists.len(), 3);
    assert_eq!(lists[0]["title"], "Alpha List");
    assert_eq!(lists[1]["title"], "Beta List");
    assert_eq!(lists[2]["title"], "Zebra List");

    // List sorted by title descending
    let page = PageParams {
        limit: None,
        offset: None,
        sort: Some("title"),
        order: Some("desc"),
    };
    let result = list_task_lists(&api_client, None, None, None, None, page, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let lists = parsed.as_array().unwrap();

    assert_eq!(lists.len(), 3);
    assert_eq!(lists[0]["title"], "Zebra List");
    assert_eq!(lists[1]["title"], "Beta List");
    assert_eq!(lists[2]["title"], "Alpha List");
}
