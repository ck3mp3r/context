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
    let request = CreateTaskListRequest {
        title: "Integration Test List".to_string(),
        project_id: project_id.clone(),
        description: Some("Test description".to_string()),
        tags: Some(vec!["testing".to_string(), "integration".to_string()]),
        repo_ids: None,
    };
    let result = create_task_list(&api_client, request).await;

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
    let req1 = CreateTaskListRequest {
        title: "List 1".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: Some(vec!["tag1".to_string()]),
        repo_ids: None,
    };
    create_task_list(&api_client, req1)
        .await
        .expect("Failed to create list 1");

    let req2 = CreateTaskListRequest {
        title: "List 2".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: Some(vec!["tag2".to_string()]),
        repo_ids: None,
    };
    create_task_list(&api_client, req2)
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
    let request = CreateTaskListRequest {
        title: "Test List".to_string(),
        project_id: project_id.clone(),
        description: Some("Description".to_string()),
        tags: None,
        repo_ids: None,
    };
    let create_result = create_task_list(&api_client, request)
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
    let request = CreateTaskListRequest {
        title: "Original Title".to_string(),
        project_id: project_id.clone(),
        description: Some("Original desc".to_string()),
        tags: Some(vec!["tag1".to_string()]),
        repo_ids: None,
    };
    let create_result = create_task_list(&api_client, request)
        .await
        .expect("Failed to create task list");

    let list_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract list ID");

    // Update task list
    let update_request = UpdateTaskListRequest {
        title: "Updated Title".to_string(),
        description: Some("Updated description".to_string()),
        status: None,
        tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
        project_id: None,
        repo_ids: None,
    };
    let result = update_task_list(&api_client, list_id, update_request).await;
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
    let request = CreateTaskListRequest {
        title: "Stats Test List".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: None,
        repo_ids: None,
    };
    let create_result = create_task_list(&api_client, request)
        .await
        .expect("Failed to create task list");

    let list_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract list ID");

    // Create some tasks in the list
    let req1 = crate::cli::commands::task::CreateTaskRequest {
        title: "Task 1".to_string(),
        description: None,
        parent_id: None,
        priority: Some(1),
        tags: None,
        external_refs: None,
    };
    crate::cli::commands::task::create_task(&api_client, list_id, req1)
        .await
        .expect("Failed to create task 1");

    let req2 = crate::cli::commands::task::CreateTaskRequest {
        title: "Task 2".to_string(),
        description: None,
        parent_id: None,
        priority: Some(2),
        tags: None,
        external_refs: None,
    };
    crate::cli::commands::task::create_task(&api_client, list_id, req2)
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
    let update_request = UpdateTaskListRequest {
        title: "New Title".to_string(),
        description: Some("New desc".to_string()),
        status: None,
        tags: None,
        project_id: None,
        repo_ids: None,
    };
    let result = update_task_list(&api_client, "nonexist", update_request).await;

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
    let request = CreateTaskListRequest {
        title: "Task List Title".to_string(),
        project_id: "nonexist".to_string(),
        description: None,
        tags: None,
        repo_ids: None,
    };
    let result = create_task_list(&api_client, request).await;

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
        let request = CreateTaskListRequest {
            title: format!("List {}", i),
            project_id: project_id.clone(),
            description: None,
            tags: None,
            repo_ids: None,
        };
        let _ = create_task_list(&api_client, request).await;
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
    let req1 = CreateTaskListRequest {
        title: "Zebra List".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: None,
        repo_ids: None,
    };
    let _ = create_task_list(&api_client, req1).await;

    let req2 = CreateTaskListRequest {
        title: "Alpha List".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: None,
        repo_ids: None,
    };
    let _ = create_task_list(&api_client, req2).await;

    let req3 = CreateTaskListRequest {
        title: "Beta List".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: None,
        repo_ids: None,
    };
    let _ = create_task_list(&api_client, req3).await;

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
