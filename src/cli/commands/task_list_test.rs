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
    let api_client = ApiClient::new(Some(url.clone()));

    // Create a repository to link
    let repo_payload = serde_json::json!({
        "remote": "https://github.com/test/task-list-repo",
        "tags": []
    });
    let repo_response = api_client
        .post("/api/v1/repos")
        .json(&repo_payload)
        .send()
        .await
        .expect("Failed to create repo");
    let repo: serde_json::Value = repo_response.json().await.unwrap();
    let repo_id = repo["id"].as_str().unwrap();

    // Create task list with description, tags, and repo link
    let request = CreateTaskListRequest {
        title: "Integration Test List".to_string(),
        project_id: project_id.clone(),
        description: Some("Test description".to_string()),
        tags: Some(vec!["testing".to_string(), "integration".to_string()]),
        repo_ids: Some(vec![repo_id.to_string()]),
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
    assert_eq!(created_list["repo_ids"], json!([repo_id]));
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

// =============================================================================
// Repository Linking Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_list_with_multiple_repo_ids() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create two repositories
    let repo1_payload = serde_json::json!({
        "remote": "https://github.com/test/repo1",
        "tags": []
    });
    let repo1_response = api_client
        .post("/api/v1/repos")
        .json(&repo1_payload)
        .send()
        .await
        .expect("Failed to create repo 1");
    let repo1: serde_json::Value = repo1_response.json().await.unwrap();
    let repo1_id = repo1["id"].as_str().unwrap();

    let repo2_payload = serde_json::json!({
        "remote": "https://github.com/test/repo2",
        "tags": []
    });
    let repo2_response = api_client
        .post("/api/v1/repos")
        .json(&repo2_payload)
        .send()
        .await
        .expect("Failed to create repo 2");
    let repo2: serde_json::Value = repo2_response.json().await.unwrap();
    let repo2_id = repo2["id"].as_str().unwrap();

    // Create task list linked to multiple repos
    let request = CreateTaskListRequest {
        title: "Task List with Multiple Repos".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: None,
        repo_ids: Some(vec![repo1_id.to_string(), repo2_id.to_string()]),
    };
    let create_result = create_task_list(&api_client, request).await;
    assert!(
        create_result.is_ok(),
        "Should create task list with multiple repo_ids"
    );

    let output = create_result.unwrap();
    let list_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract list ID");

    // Verify both repos are linked
    let get_result = get_task_list(&api_client, list_id, "json").await;
    assert!(get_result.is_ok());
    let task_list: serde_json::Value = serde_json::from_str(&get_result.unwrap()).unwrap();
    let repo_ids_val = task_list["repo_ids"].as_array().unwrap();
    assert_eq!(repo_ids_val.len(), 2);
    assert!(repo_ids_val.contains(&json!(repo1_id)));
    assert!(repo_ids_val.contains(&json!(repo2_id)));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_list_to_add_repo_ids() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create a repository
    let repo_payload = serde_json::json!({
        "remote": "https://github.com/test/update-repo",
        "tags": []
    });
    let repo_response = api_client
        .post("/api/v1/repos")
        .json(&repo_payload)
        .send()
        .await
        .expect("Failed to create repo");
    let repo: serde_json::Value = repo_response.json().await.unwrap();
    let repo_id = repo["id"].as_str().unwrap();

    // Create task list without repo_ids
    let request = CreateTaskListRequest {
        title: "Task List Without Repos".to_string(),
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

    // Update task list to add repo_ids
    let update_request = UpdateTaskListRequest {
        title: "Task List With Repos".to_string(),
        description: None,
        status: None,
        tags: None,
        project_id: None,
        repo_ids: Some(vec![repo_id.to_string()]),
    };
    let update_result = update_task_list(&api_client, list_id, update_request).await;
    assert!(
        update_result.is_ok(),
        "Should update task list with repo_ids"
    );

    // Verify the update
    let get_result = get_task_list(&api_client, list_id, "json").await;
    assert!(get_result.is_ok());
    let task_list: serde_json::Value = serde_json::from_str(&get_result.unwrap()).unwrap();
    let repo_ids_val = task_list["repo_ids"].as_array().unwrap();
    assert_eq!(repo_ids_val.len(), 1);
    assert_eq!(repo_ids_val[0], json!(repo_id));
}

// =============================================================================
// Empty Results and Display Format Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_empty_result() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // List task lists with no data (table format)
    let result = list_task_lists(
        &api_client,
        None,
        None,
        None,
        None,
        PageParams::default(),
        "table",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output, "No task lists found.", "Should show empty message");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_table_format() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create a task list
    let request = CreateTaskListRequest {
        title: "Test List for Table".to_string(),
        project_id: project_id.clone(),
        description: Some("Description".to_string()),
        tags: Some(vec!["tag1".to_string()]),
        repo_ids: None,
    };
    create_task_list(&api_client, request)
        .await
        .expect("Failed to create task list");

    // List in table format
    let result = list_task_lists(
        &api_client,
        None,
        None,
        None,
        None,
        PageParams::default(),
        "table",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should contain table headers and data
    assert!(output.contains("ID"), "Table should have ID column");
    assert!(output.contains("Title"), "Table should have Title column");
    assert!(
        output.contains("Project"),
        "Table should have Project column"
    );
    assert!(output.contains("Status"), "Table should have Status column");
    assert!(output.contains("Tags"), "Table should have Tags column");
    assert!(
        output.contains("Test List for Table"),
        "Table should contain task list title"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_list_table_format() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list with all fields
    let request = CreateTaskListRequest {
        title: "Detailed Test List".to_string(),
        project_id: project_id.clone(),
        description: Some("Test description".to_string()),
        tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
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

    // Get in table format
    let result = get_task_list(&api_client, list_id, "table").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should contain all fields in table format
    assert!(output.contains("Field"), "Table should have Field column");
    assert!(output.contains("Value"), "Table should have Value column");
    assert!(output.contains("ID"), "Table should show ID field");
    assert!(output.contains("Title"), "Table should show Title field");
    assert!(
        output.contains("Description"),
        "Table should show Description field"
    );
    assert!(
        output.contains("Project ID"),
        "Table should show Project ID field"
    );
    assert!(output.contains("Status"), "Table should show Status field");
    assert!(output.contains("Tags"), "Table should show Tags field");
    assert!(
        output.contains("External Refs"),
        "Table should show External Refs field"
    );
    assert!(
        output.contains("Created"),
        "Table should show Created field"
    );
    assert!(
        output.contains("Updated"),
        "Table should show Updated field"
    );
    assert!(
        output.contains("Detailed Test List"),
        "Table should contain title"
    );
    assert!(
        output.contains("Test description"),
        "Table should contain description"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_list_stats_table_format() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list
    let request = CreateTaskListRequest {
        title: "Stats Table Test".to_string(),
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

    // Get stats in table format
    let result = get_task_list_stats(&api_client, list_id, "table").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should contain table with all status counts
    assert!(output.contains("Metric"), "Table should have Metric column");
    assert!(output.contains("Count"), "Table should have Count column");
    assert!(output.contains("Total"), "Table should show Total");
    assert!(output.contains("Backlog"), "Table should show Backlog");
    assert!(output.contains("Todo"), "Table should show Todo");
    assert!(
        output.contains("In Progress"),
        "Table should show In Progress"
    );
    assert!(output.contains("Review"), "Table should show Review");
    assert!(output.contains("Done"), "Table should show Done");
    assert!(output.contains("Cancelled"), "Table should show Cancelled");
}

// =============================================================================
// Query Parameter and Filtering Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_with_project_filter() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list in test project
    let request = CreateTaskListRequest {
        title: "Project Filtered List".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: None,
        repo_ids: None,
    };
    create_task_list(&api_client, request)
        .await
        .expect("Failed to create task list");

    // List with project_id filter
    let result = list_task_lists(
        &api_client,
        None,
        Some(&project_id),
        None,
        None,
        PageParams::default(),
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed.as_array().unwrap().len() >= 1);
    assert_eq!(parsed[0]["project_id"], project_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_with_status_filter() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list
    let request = CreateTaskListRequest {
        title: "Active List".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: None,
        repo_ids: None,
    };
    create_task_list(&api_client, request)
        .await
        .expect("Failed to create task list");

    // List with status filter
    let result = list_task_lists(
        &api_client,
        None,
        None,
        Some("active"),
        None,
        PageParams::default(),
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let lists = parsed.as_array().unwrap();
    // All returned lists should have active status
    for list in lists {
        assert_eq!(list["status"], "active");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_with_tags_filter() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list with specific tags
    let request = CreateTaskListRequest {
        title: "Tagged List".to_string(),
        project_id: project_id.clone(),
        description: None,
        tags: Some(vec!["urgent".to_string(), "backend".to_string()]),
        repo_ids: None,
    };
    create_task_list(&api_client, request)
        .await
        .expect("Failed to create task list");

    // List with tags filter
    let result = list_task_lists(
        &api_client,
        None,
        None,
        None,
        Some("urgent"),
        PageParams::default(),
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(
        parsed.as_array().unwrap().len() >= 1,
        "Should find at least one list with 'urgent' tag"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_with_limit() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create 5 task lists
    for i in 1..=5 {
        let request = CreateTaskListRequest {
            title: format!("List {}", i),
            project_id: project_id.clone(),
            description: None,
            tags: None,
            repo_ids: None,
        };
        create_task_list(&api_client, request)
            .await
            .expect("Failed to create task list");
    }

    // List with limit=3
    let page = PageParams {
        limit: Some(3),
        offset: None,
        sort: None,
        order: None,
    };
    let result = list_task_lists(&api_client, None, None, None, None, page, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        parsed.as_array().unwrap().len(),
        3,
        "Should return exactly 3 lists"
    );
}

// =============================================================================
// Delete Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task_list_without_force_flag() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list
    let request = CreateTaskListRequest {
        title: "List to Delete".to_string(),
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

    // Try to delete without --force flag
    let result = delete_task_list(&api_client, list_id, false).await;

    // Should return error requiring --force flag
    assert!(result.is_err(), "Should require --force flag for delete");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("--force") || error.contains("force"),
        "Error should mention --force flag requirement, got: {}",
        error
    );
}

// =============================================================================
// Update Edge Cases
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_list_with_empty_title() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list
    let request = CreateTaskListRequest {
        title: "Original Title".to_string(),
        project_id: project_id.clone(),
        description: Some("Original desc".to_string()),
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

    // Update with empty title (should fetch and preserve current title)
    let update_request = UpdateTaskListRequest {
        title: "".to_string(), // Empty title
        description: Some("Updated description".to_string()),
        status: None,
        tags: None,
        project_id: None,
        repo_ids: None,
    };
    let result = update_task_list(&api_client, list_id, update_request).await;
    assert!(result.is_ok(), "Should update even with empty title");

    // Verify title was preserved
    let get_result = get_task_list(&api_client, list_id, "json")
        .await
        .expect("Failed to get task list");
    let updated_list: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(
        updated_list["title"], "Original Title",
        "Title should be preserved when empty string provided"
    );
    assert_eq!(
        updated_list["description"], "Updated description",
        "Description should be updated"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task_list_with_force_flag() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list
    let request = CreateTaskListRequest {
        title: "List to Delete".to_string(),
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

    // Delete with --force flag
    let result = delete_task_list(&api_client, list_id, true).await;
    assert!(result.is_ok(), "Should delete with --force flag");

    let output = result.unwrap();
    assert!(
        output.contains("Deleted"),
        "Should confirm deletion, got: {}",
        output
    );
    assert!(output.contains(list_id), "Should mention the deleted ID");

    // Verify task list is actually deleted
    let get_result = get_task_list(&api_client, list_id, "json").await;
    assert!(
        get_result.is_err(),
        "Getting deleted task list should return error"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task_list_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to delete non-existent task list
    let result = delete_task_list(&api_client, "nonexist", true).await;

    // Should return error
    assert!(
        result.is_err(),
        "Should return error for non-existent task list"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}
