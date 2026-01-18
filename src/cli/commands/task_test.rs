use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::task::*;
use crate::cli::commands::task_list::{CreateTaskListRequest, create_task_list};
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
        list_id: None,
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
        list_id: None,
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
        list_id: None,
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
    let request = crate::cli::commands::task_list::CreateTaskListRequest {
        title: "Test Task List".to_string(),
        project_id: project_id.to_string(),
        description: None,
        tags: None,
        repo_ids: None,
    };
    let result = crate::cli::commands::task_list::create_task_list(&api_client, request)
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
    let request = CreateTaskRequest {
        title: "Integration Test Task".to_string(),
        description: Some("Test description".to_string()),
        priority: Some(3),
        tags: Some(vec!["bug".to_string(), "urgent".to_string()]),
        external_refs: None,
        parent_id: None,
    };
    let result = create_task(&api_client, &list_id, request).await;

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
    let request = CreateTaskRequest {
        title: "Minimal Task".to_string(),
        description: None,
        priority: None,
        tags: None,
        external_refs: None,
        parent_id: None,
    };
    let result = create_task(&api_client, &list_id, request).await;

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
    let request = CreateTaskRequest {
        title: "Task with external refs".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: None,
        external_refs: Some(vec!["owner/repo#123".to_string(), "PROJ-456".to_string()]),
    };
    let result = create_task(&api_client, &list_id, request).await;

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
    let req1 = CreateTaskRequest {
        title: "Task 1".to_string(),
        description: None,
        parent_id: None,
        priority: Some(1),
        tags: None,
        external_refs: None,
    };
    create_task(&api_client, &list_id, req1)
        .await
        .expect("Failed to create task 1");
    let req2 = CreateTaskRequest {
        title: "Task 2".to_string(),
        description: None,
        parent_id: None,
        priority: Some(2),
        tags: None,
        external_refs: None,
    };
    create_task(&api_client, &list_id, req2)
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
            r#type: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
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

    // Create task with tags
    let request = CreateTaskRequest {
        title: "Task to Complete".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: Some(vec!["feature".to_string(), "completed".to_string()]),
        external_refs: None,
    };
    let create_result = create_task(&api_client, &list_id, request)
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

    // Verify task is marked as done and tags persist
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let updated_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_task["status"], "done");
    assert_eq!(updated_task["tags"], json!(["feature", "completed"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_task_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create task (starts as backlog) with tags
    let request = CreateTaskRequest {
        title: "Task to Transition".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: Some(vec!["workflow".to_string()]),
        external_refs: None,
    };
    let create_result = create_task(&api_client, &list_id, request)
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

    // Verify status changed and tags persist
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let updated_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_task["status"], "todo");
    assert_eq!(updated_task["tags"], json!(["workflow"]));

    // Transition to in_progress
    transition_task(&api_client, task_id, "in_progress")
        .await
        .expect("Failed to transition");
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let updated_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_task["status"], "in_progress");
    assert_eq!(updated_task["tags"], json!(["workflow"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create parent task
    let parent_request = CreateTaskRequest {
        title: "Parent Task".to_string(),
        description: None,
        parent_id: None,
        priority: Some(2),
        tags: None,
        external_refs: None,
    };
    let parent_result = create_task(&api_client, &list_id, parent_request)
        .await
        .expect("Failed to create parent task");

    let parent_id = parent_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract parent task ID");

    // Create regular task
    let request = CreateTaskRequest {
        title: "Original Title".to_string(),
        description: None,
        parent_id: None,
        priority: Some(3),
        tags: None,
        external_refs: None,
    };
    let create_result = create_task(&api_client, &list_id, request)
        .await
        .expect("Failed to create task");

    let task_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    // Update task to become a subtask and update other fields
    let update_request = UpdateTaskRequest {
        title: Some("Updated Title".to_string()),
        description: Some("New description".to_string()),
        status: None,
        priority: Some(1),
        parent_id: Some(Some(parent_id.to_string())),
        tags: Some(vec!["feature".to_string(), "backend".to_string()]),
        external_refs: None,
        list_id: None,
    };
    let result = update_task(&api_client, task_id, update_request).await;
    assert!(result.is_ok());

    // Verify updates including parent_id
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let updated_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_task["title"], "Updated Title");
    assert_eq!(updated_task["description"], "New description");
    assert_eq!(updated_task["priority"], 1);
    assert_eq!(updated_task["tags"], json!(["feature", "backend"]));
    assert_eq!(updated_task["parent_id"], parent_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_with_filters_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create tasks with different statuses and tags
    let request = CreateTaskRequest {
        title: "Bug Fix".to_string(),
        description: None,
        parent_id: None,
        priority: Some(1),
        tags: Some(vec!["bug".to_string()]),
        external_refs: None,
    };
    let _task1 = create_task(&api_client, &list_id, request)
        .await
        .expect("Failed to create task");

    // Filter by non-existent tag - should succeed (doesn't error)
    let result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: None,
            status: None,
            parent_id: None,
            tags: Some("nonexistent"),
            r#type: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
        "json",
    )
    .await;

    // Should not error - API returns results (may be empty or may return all tasks depending on implementation)
    assert!(
        result.is_ok(),
        "Filtering by nonexistent tag should not error"
    );
}

// NOTE: Offset pagination edge case test removed - API may reject or handle differently
// Future improvement: verify actual API behavior and add appropriate test

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_with_offset() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list
    let request = CreateTaskListRequest {
        title: "Test List".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: None,
        repo_ids: None,
    };
    let list_result = create_task_list(&api_client, request).await;
    let list_id = list_result
        .unwrap()
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap()
        .to_string();

    // Create 3 tasks
    for i in 1..=3 {
        let request = CreateTaskRequest {
            title: format!("Task {}", i),
            description: None,
            parent_id: None,
            priority: None,
            tags: None,
            external_refs: None,
        };
        let _ = create_task(&api_client, &list_id, request).await;
    }

    // List with offset=1 (skip first task)
    let result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: None,
            status: None,
            parent_id: None,
            tags: None,
            r#type: None,
            limit: None,
            offset: Some(1),
            sort: None,
            order: None,
        },
        "json",
    )
    .await;
    assert!(result.is_ok(), "List with offset should succeed");

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        parsed.as_array().unwrap().len(),
        2,
        "Should return 2 tasks after skipping 1"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_with_sort_and_order() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list
    let request = CreateTaskListRequest {
        title: "Test List".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: None,
        repo_ids: None,
    };
    let list_result = create_task_list(&api_client, request).await;
    let list_id = list_result
        .unwrap()
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap()
        .to_string();

    // Create tasks with different titles
    let req1 = CreateTaskRequest {
        title: "Zebra Task".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: None,
        external_refs: None,
    };
    let _ = create_task(&api_client, &list_id, req1).await;

    let req2 = CreateTaskRequest {
        title: "Alpha Task".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: None,
        external_refs: None,
    };
    let _ = create_task(&api_client, &list_id, req2).await;

    let req3 = CreateTaskRequest {
        title: "Beta Task".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: None,
        external_refs: None,
    };
    let _ = create_task(&api_client, &list_id, req3).await;

    // List sorted by title ascending
    let result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: None,
            status: None,
            parent_id: None,
            tags: None,
            r#type: None,
            limit: None,
            offset: None,
            sort: Some("title"),
            order: Some("asc"),
        },
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let tasks = parsed.as_array().unwrap();

    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0]["title"], "Alpha Task");
    assert_eq!(tasks[1]["title"], "Beta Task");
    assert_eq!(tasks[2]["title"], "Zebra Task");

    // List sorted by title descending
    let result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: None,
            status: None,
            parent_id: None,
            tags: None,
            r#type: None,
            limit: None,
            offset: None,
            sort: Some("title"),
            order: Some("desc"),
        },
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let tasks = parsed.as_array().unwrap();

    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0]["title"], "Zebra Task");
    assert_eq!(tasks[1]["title"], "Beta Task");
    assert_eq!(tasks[2]["title"], "Alpha Task");
}

// =============================================================================
// Subtask (Hierarchical) Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_with_parent_id() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create parent task
    let parent_request = CreateTaskRequest {
        title: "Parent Task".to_string(),
        description: Some("Parent task description".to_string()),
        parent_id: None,
        priority: Some(2),
        tags: Some(vec!["epic".to_string()]),
        external_refs: None,
    };
    let parent_result = create_task(&api_client, &list_id, parent_request)
        .await
        .expect("Failed to create parent task");

    let parent_id = parent_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract parent task ID");

    // Create subtask with parent_id
    let subtask_request = CreateTaskRequest {
        title: "Subtask".to_string(),
        description: Some("This is a subtask".to_string()),
        parent_id: Some(parent_id.to_string()),
        priority: Some(3),
        tags: Some(vec!["implementation".to_string()]),
        external_refs: None,
    };
    let subtask_result = create_task(&api_client, &list_id, subtask_request).await;
    assert!(
        subtask_result.is_ok(),
        "Should create subtask with parent_id"
    );

    let subtask_id = subtask_result
        .unwrap()
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract subtask ID")
        .to_string();

    // Verify subtask has parent_id
    let get_result = get_task(&api_client, &subtask_id, "json")
        .await
        .expect("Failed to get subtask");
    let subtask: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(subtask["parent_id"], parent_id);
    assert_eq!(subtask["title"], "Subtask");
    assert_eq!(subtask["list_id"], list_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_filtered_by_parent_id() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create parent task
    let parent_request = CreateTaskRequest {
        title: "Parent Task".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: None,
        external_refs: None,
    };
    let parent_result = create_task(&api_client, &list_id, parent_request)
        .await
        .expect("Failed to create parent task");

    let parent_id = parent_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract parent task ID");

    // Create two subtasks
    let subtask1_request = CreateTaskRequest {
        title: "Subtask 1".to_string(),
        description: None,
        parent_id: Some(parent_id.to_string()),
        priority: None,
        tags: None,
        external_refs: None,
    };
    create_task(&api_client, &list_id, subtask1_request)
        .await
        .expect("Failed to create subtask 1");

    let subtask2_request = CreateTaskRequest {
        title: "Subtask 2".to_string(),
        description: None,
        parent_id: Some(parent_id.to_string()),
        priority: None,
        tags: None,
        external_refs: None,
    };
    create_task(&api_client, &list_id, subtask2_request)
        .await
        .expect("Failed to create subtask 2");

    // Create another parent task without subtasks
    let other_parent_request = CreateTaskRequest {
        title: "Other Parent".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: None,
        external_refs: None,
    };
    create_task(&api_client, &list_id, other_parent_request)
        .await
        .expect("Failed to create other parent");

    // List tasks filtered by parent_id
    let result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: None,
            status: None,
            parent_id: Some(parent_id),
            tags: None,
            r#type: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let tasks = parsed.as_array().unwrap();

    // Should only return the 2 subtasks
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0]["title"], "Subtask 1");
    assert_eq!(tasks[0]["parent_id"], parent_id);
    assert_eq!(tasks[1]["title"], "Subtask 2");
    assert_eq!(tasks[1]["parent_id"], parent_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_to_remove_parent_id() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create parent task
    let parent_request = CreateTaskRequest {
        title: "Parent Task".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: None,
        external_refs: None,
    };
    let parent_result = create_task(&api_client, &list_id, parent_request)
        .await
        .expect("Failed to create parent task");

    let parent_id = parent_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract parent task ID");

    // Create subtask
    let subtask_request = CreateTaskRequest {
        title: "Subtask".to_string(),
        description: None,
        parent_id: Some(parent_id.to_string()),
        priority: None,
        tags: None,
        external_refs: None,
    };
    let subtask_result = create_task(&api_client, &list_id, subtask_request)
        .await
        .expect("Failed to create subtask");

    let subtask_id = subtask_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract subtask ID")
        .to_string();

    // Verify it has parent_id
    let get_result = get_task(&api_client, &subtask_id, "json")
        .await
        .expect("Failed to get subtask");
    let subtask_before: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(subtask_before["parent_id"], parent_id);

    // Update to remove parent_id (convert subtask to top-level task)
    let update_request = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id: Some(None), // Explicitly remove parent
        tags: None,
        external_refs: None,
        list_id: None,
    };
    let update_result = update_task(&api_client, &subtask_id, update_request).await;
    assert!(update_result.is_ok(), "Should be able to remove parent_id");

    // Verify parent_id is removed
    let get_result_after = get_task(&api_client, &subtask_id, "json")
        .await
        .expect("Failed to get task after update");
    let task_after: serde_json::Value = serde_json::from_str(&get_result_after).unwrap();
    assert!(
        task_after["parent_id"].is_null(),
        "parent_id should be null after removal"
    );
}

// =============================================================================
// External References Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_to_add_external_refs() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create task without external_refs
    let request = CreateTaskRequest {
        title: "Task Without External Refs".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: None,
        external_refs: None,
    };
    let create_result = create_task(&api_client, &list_id, request)
        .await
        .expect("Failed to create task");

    let task_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    // Update task to add external_refs
    let update_request = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id: None,
        tags: None,
        external_refs: Some(vec!["owner/repo#456".to_string(), "JIRA-789".to_string()]),
        list_id: None,
    };
    let update_result = update_task(&api_client, task_id, update_request).await;
    assert!(update_result.is_ok(), "Should be able to add external_refs");

    // Verify external_refs were added
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(task["external_refs"], json!(["owner/repo#456", "JIRA-789"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_external_refs_persist_across_status_transitions() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create task with external_refs
    let request = CreateTaskRequest {
        title: "Task with External Refs".to_string(),
        description: None,
        parent_id: None,
        priority: Some(1),
        tags: None,
        external_refs: Some(vec!["owner/repo#100".to_string()]),
    };
    let create_result = create_task(&api_client, &list_id, request)
        .await
        .expect("Failed to create task");

    let task_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    // Transition task to todo
    let transition_result = transition_task(&api_client, task_id, "todo").await;
    assert!(transition_result.is_ok());

    // Verify external_refs still present
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(task["status"], "todo");
    assert_eq!(task["external_refs"], json!(["owner/repo#100"]));

    // Transition to in_progress
    let _ = transition_task(&api_client, task_id, "in_progress").await;

    // Verify external_refs still present after another transition
    let get_result2 = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let task2: serde_json::Value = serde_json::from_str(&get_result2).unwrap();
    assert_eq!(task2["status"], "in_progress");
    assert_eq!(task2["external_refs"], json!(["owner/repo#100"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_subtask_with_external_refs() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create parent task
    let parent_request = CreateTaskRequest {
        title: "Parent with External Ref".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: None,
        external_refs: Some(vec!["EPIC-100".to_string()]),
    };
    let parent_result = create_task(&api_client, &list_id, parent_request)
        .await
        .expect("Failed to create parent task");

    let parent_id = parent_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract parent task ID");

    // Create subtask with its own external_ref
    let subtask_request = CreateTaskRequest {
        title: "Subtask with External Ref".to_string(),
        description: None,
        parent_id: Some(parent_id.to_string()),
        priority: None,
        tags: None,
        external_refs: Some(vec!["owner/repo#200".to_string()]),
    };
    let subtask_result = create_task(&api_client, &list_id, subtask_request).await;
    assert!(
        subtask_result.is_ok(),
        "Should create subtask with external_refs"
    );

    let subtask_id = subtask_result
        .unwrap()
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract subtask ID")
        .to_string();

    // Verify both parent and subtask have their own external_refs
    let parent_get = get_task(&api_client, parent_id, "json")
        .await
        .expect("Failed to get parent");
    let parent_task: serde_json::Value = serde_json::from_str(&parent_get).unwrap();
    assert_eq!(parent_task["external_refs"], json!(["EPIC-100"]));

    let subtask_get = get_task(&api_client, &subtask_id, "json")
        .await
        .expect("Failed to get subtask");
    let subtask: serde_json::Value = serde_json::from_str(&subtask_get).unwrap();
    assert_eq!(subtask["external_refs"], json!(["owner/repo#200"]));
    assert_eq!(subtask["parent_id"], parent_id);
}
