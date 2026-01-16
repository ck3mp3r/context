use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::task::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use serde_json::json;
use tokio::net::TcpListener;

// Tests for empty string parent_id handling (CLI pattern: --parent-id="" removes parent)

#[test]
fn test_update_request_empty_string_parent_id_converts_to_none() {
    // Simulate CLI logic: empty string should convert to Some(None) to remove parent
    let parent_id_input = Some("".to_string());

    let parent_id = parent_id_input.map(|s| {
        if s.is_empty() {
            None // Empty string means remove parent
        } else {
            Some(s.to_string())
        }
    });

    let req = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id,
        tags: None,
        external_refs: None,
    };

    // parent_id should be Some(None) - explicitly removing the parent
    assert_eq!(req.parent_id, Some(None));

    // Serialize and verify it includes "parent_id": null
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"parent_id\":null"));
}

#[test]
fn test_update_request_non_empty_parent_id_sets_value() {
    // CLI: --parent-id="parent123" should set parent
    let parent_id_input = Some("parent123".to_string());

    let parent_id = parent_id_input.map(|s| {
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    });

    let req = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id,
        tags: None,
        external_refs: None,
    };

    // parent_id should be Some(Some("parent123"))
    assert_eq!(req.parent_id, Some(Some("parent123".to_string())));

    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"parent_id\":\"parent123\""));
}

#[test]
fn test_update_request_missing_parent_id_field_is_none() {
    // CLI: not providing --parent-id at all should be None (no change)
    let parent_id_input: Option<String> = None;

    let parent_id = parent_id_input.map(|s| {
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    });

    let req = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id,
        tags: None,
        external_refs: None,
    };

    // parent_id should be None - field not included in update
    assert_eq!(req.parent_id, None);

    // Serialize and verify parent_id is omitted (skip_serializing_if)
    let json = serde_json::to_string(&req).unwrap();
    assert!(!json.contains("parent_id"));
}

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

/// Helper to create a task list via HTTP and return its ID
async fn create_test_task_list(api_url: &str, project_id: &str) -> String {
    let api_client = ApiClient::new(Some(api_url.to_string()));
    let result = crate::cli::commands::task_list::create_task_list(
        &api_client,
        "Test Task List",
        project_id,
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create test task list");

    // Extract ID from success message: "✓ Created task list: Title (list_id)"
    result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task list ID")
        .to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create task with all fields
    let result = create_task(
        &api_client,
        &list_id,
        "Integration Test Task",
        Some("Test description"),
        Some(3),
        Some("bug,urgent"),
        None,
        None,
    )
    .await;

    assert!(result.is_ok());
    let output = result.unwrap();

    // Extract task ID from success message: "✓ Created task: Title (task_id)"
    let task_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    // Verify all fields were persisted correctly by fetching the task
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let created_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();

    assert_eq!(created_task["title"], "Integration Test Task");
    assert_eq!(created_task["description"], "Test description");
    assert_eq!(created_task["priority"], 3);
    assert_eq!(created_task["tags"], json!(["bug", "urgent"]));
    assert_eq!(created_task["status"], "backlog");
    assert_eq!(created_task["list_id"], list_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_minimal_defaults_to_p5() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create task with only required fields (title)
    let result = create_task(
        &api_client,
        &list_id,
        "Minimal Task",
        None, // no description
        None, // no priority
        None, // no tags
        None,
        None,
    )
    .await;

    assert!(result.is_ok());
    let output = result.unwrap();

    let task_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    // Verify defaults: priority should be 5 (lowest)
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let created_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();

    assert_eq!(created_task["title"], "Minimal Task");
    assert_eq!(created_task["priority"], 5, "Default priority should be P5");
    assert_eq!(
        created_task["status"], "backlog",
        "Default status should be backlog"
    );
    assert_eq!(
        created_task["tags"],
        json!([]),
        "Tags should be empty array"
    );
    assert!(
        created_task["description"].is_null() || created_task["description"] == "",
        "Description should be null or empty"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_with_external_refs() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create task with external refs (GitHub issue and Jira ticket)
    let result = create_task(
        &api_client,
        &list_id,
        "Task with external refs",
        None,
        None,
        None,
        Some("owner/repo#123,PROJ-456"),
        None,
    )
    .await;

    assert!(result.is_ok());
    let output = result.unwrap();

    let task_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    // Verify external refs were persisted
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let created_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();

    assert_eq!(created_task["title"], "Task with external refs");
    assert_eq!(
        created_task["external_refs"],
        json!(["owner/repo#123", "PROJ-456"])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create two tasks
    create_task(
        &api_client,
        &list_id,
        "Task 1",
        None,
        Some(1),
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create task 1");
    create_task(
        &api_client,
        &list_id,
        "Task 2",
        None,
        Some(2),
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create task 2");

    // List tasks
    let result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: None,
            status: None,
            parent_id: None,
            tags: None,
            limit: None,
            offset: None,
        },
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("Failed to parse JSON");

    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["title"], "Task 1");
    assert_eq!(parsed[1]["title"], "Task 2");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_complete_task_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create task
    let create_result = create_task(
        &api_client,
        &list_id,
        "Task to Complete",
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create task");

    // Extract task ID from success message: "✓ Created task: Title (task_id)"
    let task_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    // Complete task
    let result = complete_task(&api_client, task_id).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.contains(task_id));

    // Verify task is marked as done
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let updated_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_task["status"], "done");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_task_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create task (starts as backlog)
    let create_result = create_task(
        &api_client,
        &list_id,
        "Task to Transition",
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create task");

    let task_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    // Transition to todo
    let result = transition_task(&api_client, task_id, "todo").await;
    assert!(result.is_ok());

    // Verify status changed
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let updated_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_task["status"], "todo");

    // Transition to in_progress
    transition_task(&api_client, task_id, "in_progress")
        .await
        .expect("Failed to transition");
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let updated_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_task["status"], "in_progress");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create task
    let create_result = create_task(
        &api_client,
        &list_id,
        "Original Title",
        None,
        Some(3),
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create task");

    let task_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    // Update task
    let result = update_task(
        &api_client,
        task_id,
        UpdateTaskParams {
            title: Some("Updated Title"),
            description: Some("New description"),
            status: None,
            priority: Some(1),
            parent_id: None,
            tags: Some("feature,backend"),
            external_refs: None,
        },
    )
    .await;
    assert!(result.is_ok());

    // Verify updates
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let updated_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_task["title"], "Updated Title");
    assert_eq!(updated_task["description"], "New description");
    assert_eq!(updated_task["priority"], 1);
    assert_eq!(updated_task["tags"], json!(["feature", "backend"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_with_filters_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create tasks with different statuses and tags
    let task1 = create_task(
        &api_client,
        &list_id,
        "Bug Fix",
        None,
        Some(1),
        Some("bug"),
        None,
        None,
    )
    .await
    .expect("Failed to create task 1");

    let task1_id = task1
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    create_task(
        &api_client,
        &list_id,
        "Feature",
        None,
        Some(3),
        Some("feature"),
        None,
        None,
    )
    .await
    .expect("Failed to create task 2");

    // Transition first task to todo
    transition_task(&api_client, task1_id, "todo")
        .await
        .expect("Failed to transition");

    // Filter by status=todo
    let result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: None,
            status: Some("todo"),
            parent_id: None,
            tags: None,
            limit: None,
            offset: None,
        },
        "json",
    )
    .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["title"], "Bug Fix");

    // Filter by tags=feature
    let result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: None,
            status: None,
            parent_id: None,
            tags: Some("feature"),
            limit: None,
            offset: None,
        },
        "json",
    )
    .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    // Find the task with "feature" tag
    let feature_task = parsed
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["title"] == "Feature");
    assert!(
        feature_task.is_some(),
        "Should find Feature task in results"
    );
    assert_eq!(feature_task.unwrap()["tags"], json!(["feature"]));
}

// =============================================================================
// Unhappy Path Tests - NOT FOUND Errors
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to get non-existent task
    let result = get_task(&api_client, "nonexist", "json").await;

    // Should return error with not found message
    assert!(result.is_err(), "Should return error for non-existent task");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to update non-existent task
    let result = update_task(
        &api_client,
        "nonexist",
        UpdateTaskParams {
            title: Some("New Title"),
            description: None,
            status: None,
            priority: None,
            parent_id: None,
            tags: None,
            external_refs: None,
        },
    )
    .await;

    // Should return error
    assert!(result.is_err(), "Should return error for non-existent task");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to delete non-existent task (with force=true as required by signature)
    let result = delete_task(&api_client, "nonexist", true).await;

    // Should return error
    assert!(result.is_err(), "Should return error for non-existent task");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_complete_task_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to complete non-existent task
    let result = complete_task(&api_client, "nonexist").await;

    // Should return error
    assert!(result.is_err(), "Should return error for non-existent task");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_task_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to transition non-existent task
    let result = transition_task(&api_client, "nonexist", "todo").await;

    // Should return error
    assert!(result.is_err(), "Should return error for non-existent task");
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

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_with_invalid_priority_zero() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Try to create task with priority 0 (invalid: must be 1-5)
    let result = create_task(
        &api_client,
        &list_id,
        "Invalid Priority Task",
        None,
        Some(0),
        None,
        None,
        None,
    )
    .await;

    // Should return error with validation message
    assert!(result.is_err(), "Should return error for priority 0");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("priority") || error.contains("1") || error.contains("5"),
        "Error should mention priority validation, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_with_invalid_priority_six() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Try to create task with priority 6 (invalid: must be 1-5)
    let result = create_task(
        &api_client,
        &list_id,
        "Invalid Priority Task",
        None,
        Some(6),
        None,
        None,
        None,
    )
    .await;

    // Should return error
    assert!(result.is_err(), "Should return error for priority 6");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("priority") || error.contains("1") || error.contains("5"),
        "Error should mention priority validation, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_with_invalid_priority_negative() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Try to create task with priority -1 (invalid)
    let result = create_task(
        &api_client,
        &list_id,
        "Invalid Priority Task",
        None,
        Some(-1),
        None,
        None,
        None,
    )
    .await;

    // Should return error
    assert!(result.is_err(), "Should return error for priority -1");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("priority") || error.contains("1") || error.contains("5"),
        "Error should mention priority validation, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_with_nonexistent_list_id() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to create task with non-existent list_id
    let result = create_task(
        &api_client,
        "nonexist",
        "Task Title",
        None,
        None,
        None,
        None,
        None,
    )
    .await;

    // Should return error (foreign key constraint)
    assert!(
        result.is_err(),
        "Should return error for non-existent list_id"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found")
            || error.contains("404")
            || error.contains("foreign key")
            || error.contains("constraint"),
        "Error should mention foreign key or not found, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_with_nonexistent_parent_id() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Try to create subtask with non-existent parent_id
    let result = create_task(
        &api_client,
        &list_id,
        "Subtask",
        None,
        None,
        None,
        None,
        Some("nonexist"),
    )
    .await;

    // Should return error (foreign key constraint)
    assert!(
        result.is_err(),
        "Should return error for non-existent parent_id"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found")
            || error.contains("404")
            || error.contains("parent")
            || error.contains("foreign key")
            || error.contains("constraint"),
        "Error should mention parent or foreign key, got: {}",
        error
    );
}

// NOTE: The following tests are NOT included because the API does not validate these cases:
// - test_transition_task_invalid_backlog_to_done: API PATCH /tasks/{id} allows any status transition
//   (validation only exists in MCP tool layer, not HTTP API)
// - test_create_task_empty_title: API allows empty titles (no validation at HTTP API layer)
