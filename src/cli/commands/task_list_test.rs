use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::PageParams;
use crate::cli::commands::task_list::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use serde_json::json;
use tokio::net::TcpListener;

// =============================================================================
// Integration Tests - Consolidated for 100% Coverage with Realistic Data
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
async fn test_task_list_crud_operations() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create repository for linking
    let repo_payload = serde_json::json!({
        "remote": "https://github.com/acme/backend-services",
        "tags": ["production", "backend"]
    });
    let repo_response = api_client
        .post("/api/v1/repos")
        .json(&repo_payload)
        .send()
        .await
        .expect("Failed to create repo");
    let repo: serde_json::Value = repo_response.json().await.unwrap();
    let repo_id = repo["id"].as_str().unwrap();

    // CREATE: Task list with all fields populated
    let create_request = CreateTaskListRequest {
        title: "Q1 2026 Backend Migration".to_string(),
        project_id: project_id.clone(),
        description: Some("Migrate legacy monolith to microservices architecture".to_string()),
        tags: Some(vec![
            "migration".to_string(),
            "backend".to_string(),
            "q1-2026".to_string(),
        ]),
        repo_ids: Some(vec![repo_id.to_string()]),
    };
    let create_result = create_task_list(&api_client, create_request).await;
    assert!(
        create_result.is_ok(),
        "Should create task list with full data"
    );

    // Extract list ID from success message
    let output = create_result.unwrap();
    let list_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract list ID");

    // GET: Verify all fields persisted correctly
    let get_result = get_task_list(&api_client, list_id, "json")
        .await
        .expect("Failed to get task list");
    let fetched_list: serde_json::Value = serde_json::from_str(&get_result).unwrap();

    assert_eq!(fetched_list["title"], "Q1 2026 Backend Migration");
    assert_eq!(
        fetched_list["description"],
        "Migrate legacy monolith to microservices architecture"
    );
    assert_eq!(
        fetched_list["tags"],
        json!(["migration", "backend", "q1-2026"])
    );
    assert_eq!(fetched_list["project_id"], project_id);
    assert_eq!(fetched_list["status"], "active");
    assert_eq!(fetched_list["repo_ids"], json!([repo_id]));

    // UPDATE: Change multiple fields including adding external_refs
    let update_request = UpdateTaskListRequest {
        title: "Q1 2026 Backend Migration (Updated)".to_string(),
        description: Some("Migrate to microservices with Kubernetes deployment".to_string()),
        status: None,
        tags: Some(vec![
            "migration".to_string(),
            "backend".to_string(),
            "q1-2026".to_string(),
            "kubernetes".to_string(),
        ]),
        project_id: None,
        repo_ids: None,
    };
    let update_result = update_task_list(&api_client, list_id, update_request).await;
    assert!(update_result.is_ok(), "Should update task list");

    // Add external_refs via API (not supported by UpdateTaskListRequest)
    let external_refs_payload = serde_json::json!({
        "title": "Q1 2026 Backend Migration (Updated)",
        "external_refs": ["ARCH-2026", "owner/repo#789"]
    });
    let _ = api_client
        .patch(&format!("/api/v1/task-lists/{}", list_id))
        .json(&external_refs_payload)
        .send()
        .await
        .expect("Failed to update external_refs");

    // Verify updates
    let get_updated = get_task_list(&api_client, list_id, "json")
        .await
        .expect("Failed to get updated task list");
    let updated_list: serde_json::Value = serde_json::from_str(&get_updated).unwrap();

    assert_eq!(updated_list["title"], "Q1 2026 Backend Migration (Updated)");
    assert_eq!(
        updated_list["description"],
        "Migrate to microservices with Kubernetes deployment"
    );
    assert_eq!(
        updated_list["tags"],
        json!(["migration", "backend", "q1-2026", "kubernetes"])
    );
    assert_eq!(
        updated_list["external_refs"],
        json!(["ARCH-2026", "owner/repo#789"])
    );

    // DELETE: Test requires force flag
    let delete_no_force = delete_task_list(&api_client, list_id, false).await;
    assert!(delete_no_force.is_err(), "Should require --force flag");
    assert!(delete_no_force.unwrap_err().to_string().contains("force"));

    // DELETE: Successful deletion with force flag
    let delete_result = delete_task_list(&api_client, list_id, true).await;
    assert!(delete_result.is_ok(), "Should delete with --force flag");
    assert!(delete_result.unwrap().contains("Deleted"));

    // Verify deletion
    let get_deleted = get_task_list(&api_client, list_id, "json").await;
    assert!(
        get_deleted.is_err(),
        "Should return error for deleted task list"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_list_list_with_comprehensive_filters() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create diverse task lists for filtering
    let lists = vec![
        (
            "Alpha Backend Tasks",
            "active",
            vec!["backend", "urgent"],
            "Backend API development",
        ),
        (
            "Beta Frontend Tasks",
            "active",
            vec!["frontend", "ui"],
            "UI component library",
        ),
        (
            "Zebra DevOps Tasks",
            "active",
            vec!["devops", "infrastructure"],
            "CI/CD pipeline setup",
        ),
    ];

    for (title, _status, tags, desc) in lists {
        let request = CreateTaskListRequest {
            title: title.to_string(),
            project_id: project_id.clone(),
            description: Some(desc.to_string()),
            tags: Some(tags.iter().map(|s| s.to_string()).collect()),
            repo_ids: None,
        };
        create_task_list(&api_client, request)
            .await
            .expect("Failed to create list");
    }

    // Test ALL filters combined: project_id, status, tags, query, limit, sort, order
    let page = PageParams {
        limit: Some(2),
        offset: None,
        sort: Some("title"),
        order: Some("asc"),
    };
    let result = list_task_lists(
        &api_client,
        Some("Backend"),   // query filter
        Some(&project_id), // project_id filter
        Some("active"),    // status filter
        Some("backend"),   // tags filter
        page,
        "json",
    )
    .await;
    assert!(result.is_ok(), "List with all filters should succeed");

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let items = parsed.as_array().unwrap();

    // Should find "Alpha Backend Tasks" matching all filters
    assert!(!items.is_empty(), "Should find backend tasks");
    assert_eq!(items[0]["title"], "Alpha Backend Tasks");
    assert!(
        items[0]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t == "backend")
    );

    // Test offset parameter separately
    let page_with_offset = PageParams {
        limit: Some(1),
        offset: Some(1),
        sort: Some("title"),
        order: Some("asc"),
    };
    let result_offset = list_task_lists(
        &api_client,
        None,
        None,
        None,
        None,
        page_with_offset,
        "json",
    )
    .await;
    assert!(result_offset.is_ok(), "List with offset should succeed");
    let parsed_offset: serde_json::Value = serde_json::from_str(&result_offset.unwrap()).unwrap();
    assert_eq!(
        parsed_offset.as_array().unwrap().len(),
        1,
        "Should return 1 item after skipping 1"
    );

    // Test sort ordering (asc vs desc)
    let page_desc = PageParams {
        limit: None,
        offset: None,
        sort: Some("title"),
        order: Some("desc"),
    };
    let result_desc = list_task_lists(&api_client, None, None, None, None, page_desc, "json").await;
    assert!(result_desc.is_ok());
    let parsed_desc: serde_json::Value = serde_json::from_str(&result_desc.unwrap()).unwrap();
    let items_desc = parsed_desc.as_array().unwrap();
    assert_eq!(items_desc[0]["title"], "Zebra DevOps Tasks");
    assert_eq!(
        items_desc[items_desc.len() - 1]["title"],
        "Alpha Backend Tasks"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_list_stats_with_various_task_states() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create task list
    let request = CreateTaskListRequest {
        title: "Sprint 12 Development Tasks".to_string(),
        project_id: project_id.clone(),
        description: Some("Feature development for Q1 release".to_string()),
        tags: Some(vec!["sprint-12".to_string(), "development".to_string()]),
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

    // Create tasks with full data in various states
    let tasks = vec![
        (
            "Implement user authentication",
            Some("Add OAuth2 and JWT support"),
            1,
            vec!["security", "backend"],
        ),
        (
            "Design login UI",
            Some("Figma mockups for login flow"),
            2,
            vec!["design", "frontend"],
        ),
        (
            "Write API documentation",
            Some("OpenAPI spec for v2 endpoints"),
            3,
            vec!["documentation"],
        ),
    ];

    for (title, desc, priority, tags) in tasks {
        let req = crate::cli::commands::task::CreateTaskRequest {
            title: title.to_string(),
            description: desc.map(|s| s.to_string()),
            parent_id: None,
            priority: Some(priority),
            tags: Some(tags.iter().map(|s| s.to_string()).collect()),
            external_refs: Some(vec![format!("SPRINT-{}", priority * 100)]),
        };
        crate::cli::commands::task::create_task(&api_client, list_id, req)
            .await
            .expect("Failed to create task");
    }

    // Get stats (JSON format)
    let stats_result = get_task_list_stats(&api_client, list_id, "json").await;
    assert!(stats_result.is_ok(), "Should get stats");

    let stats: serde_json::Value = serde_json::from_str(&stats_result.unwrap()).unwrap();
    assert_eq!(stats["total"], 3, "Should have 3 total tasks");
    assert_eq!(stats["backlog"], 3, "All tasks start in backlog");

    // Get stats (table format) - tests table display code path
    let stats_table = get_task_list_stats(&api_client, list_id, "table").await;
    assert!(stats_table.is_ok());
    let table_output = stats_table.unwrap();
    assert!(table_output.contains("Metric"));
    assert!(table_output.contains("Count"));
    assert!(table_output.contains("Total"));
    assert!(table_output.contains("Backlog"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_list_repo_linking() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create multiple repositories
    let repos = vec![
        ("https://github.com/acme/frontend", vec!["frontend"]),
        ("https://github.com/acme/backend", vec!["backend"]),
        ("https://github.com/acme/shared", vec!["library"]),
    ];

    let mut repo_ids = Vec::new();
    for (remote, tags) in repos {
        let payload = serde_json::json!({
            "remote": remote,
            "tags": tags
        });
        let response = api_client
            .post("/api/v1/repos")
            .json(&payload)
            .send()
            .await
            .expect("Failed to create repo");
        let repo: serde_json::Value = response.json().await.unwrap();
        repo_ids.push(repo["id"].as_str().unwrap().to_string());
    }

    // CREATE: Task list with multiple repos linked
    let request = CreateTaskListRequest {
        title: "Full Stack Feature Development".to_string(),
        project_id: project_id.clone(),
        description: Some("Implement user profile page across all tiers".to_string()),
        tags: Some(vec!["fullstack".to_string(), "feature".to_string()]),
        repo_ids: Some(vec![repo_ids[0].clone(), repo_ids[1].clone()]),
    };
    let create_result = create_task_list(&api_client, request)
        .await
        .expect("Failed to create task list");

    let list_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract list ID");

    // Verify repos are linked
    let get_result = get_task_list(&api_client, list_id, "json").await.unwrap();
    let task_list: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    let linked_repos = task_list["repo_ids"].as_array().unwrap();
    assert_eq!(linked_repos.len(), 2);
    assert!(linked_repos.contains(&json!(repo_ids[0])));
    assert!(linked_repos.contains(&json!(repo_ids[1])));

    // UPDATE: Add third repo
    let update_request = UpdateTaskListRequest {
        title: "Full Stack Feature Development".to_string(),
        description: Some("Implement user profile with shared components".to_string()),
        status: None,
        tags: None,
        project_id: None,
        repo_ids: Some(vec![
            repo_ids[0].clone(),
            repo_ids[1].clone(),
            repo_ids[2].clone(),
        ]),
    };
    let update_result = update_task_list(&api_client, list_id, update_request).await;
    assert!(update_result.is_ok(), "Should update repo_ids");

    // Verify all three repos are now linked
    let get_updated = get_task_list(&api_client, list_id, "json").await.unwrap();
    let updated_list: serde_json::Value = serde_json::from_str(&get_updated).unwrap();
    let updated_repos = updated_list["repo_ids"].as_array().unwrap();
    assert_eq!(updated_repos.len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_list_display_formats() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Test empty result with table format
    let empty_result = list_task_lists(
        &api_client,
        None,
        None,
        None,
        None,
        PageParams::default(),
        "table",
    )
    .await;
    assert!(empty_result.is_ok());
    assert_eq!(empty_result.unwrap(), "No task lists found.");

    // Create task list with external_refs via API (for table format testing)
    let payload = serde_json::json!({
        "title": "Infrastructure Improvements",
        "project_id": project_id,
        "description": "Upgrade Kubernetes cluster and monitoring",
        "tags": ["devops", "infrastructure", "p1"],
        "external_refs": ["INFRA-2026", "owner/repo#999"]
    });
    let response = api_client
        .post("/api/v1/task-lists")
        .json(&payload)
        .send()
        .await
        .expect("Failed to create task list");
    let task_list: serde_json::Value = response.json().await.unwrap();
    let list_id = task_list["id"].as_str().unwrap();

    // Test GET in table format - should show all fields including external_refs
    let get_table = get_task_list(&api_client, list_id, "table").await;
    assert!(get_table.is_ok());
    let table_output = get_table.unwrap();
    assert!(table_output.contains("Field"));
    assert!(table_output.contains("Value"));
    assert!(table_output.contains("Infrastructure Improvements"));
    assert!(table_output.contains("Upgrade Kubernetes cluster"));
    assert!(table_output.contains("INFRA-2026, owner/repo#999"));

    // Create task list WITHOUT external_refs to test the "-" display path
    let payload_no_refs = serde_json::json!({
        "title": "Simple Task List",
        "project_id": project_id,
        "description": "Basic list without external refs"
    });
    let response_no_refs = api_client
        .post("/api/v1/task-lists")
        .json(&payload_no_refs)
        .send()
        .await
        .expect("Failed to create task list");
    let task_list_no_refs: serde_json::Value = response_no_refs.json().await.unwrap();
    let list_id_no_refs = task_list_no_refs["id"].as_str().unwrap();

    // Test GET in table format for list WITHOUT external_refs - should show "-"
    let get_table_no_refs = get_task_list(&api_client, list_id_no_refs, "table").await;
    assert!(get_table_no_refs.is_ok());
    let table_output_no_refs = get_table_no_refs.unwrap();
    assert!(table_output_no_refs.contains("External Refs"));
    // The line "|| -" should appear when external_refs is empty
    assert!(
        table_output_no_refs.contains("| -") || table_output_no_refs.contains("│ -"),
        "Should show '-' for empty external_refs"
    );

    // Test LIST in table format
    let list_table = list_task_lists(
        &api_client,
        None,
        None,
        None,
        None,
        PageParams::default(),
        "table",
    )
    .await;
    assert!(list_table.is_ok());
    let list_output = list_table.unwrap();
    assert!(list_output.contains("ID"));
    assert!(list_output.contains("Title"));
    assert!(list_output.contains("Project"));
    assert!(list_output.contains("Status"));
    assert!(list_output.contains("Tags"));
    assert!(list_output.contains("Infrastructure Improvements"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_list_error_handling() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // GET: Non-existent task list
    let get_result = get_task_list(&api_client, "nonexist", "json").await;
    assert!(
        get_result.is_err(),
        "Should return error for non-existent task list"
    );
    let error = get_result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );

    // UPDATE: Non-existent task list
    let update_request = UpdateTaskListRequest {
        title: "Updated Title".to_string(),
        description: Some("Updated desc".to_string()),
        status: None,
        tags: None,
        project_id: None,
        repo_ids: None,
    };
    let update_result = update_task_list(&api_client, "nonexist", update_request).await;
    assert!(
        update_result.is_err(),
        "Should return error for non-existent task list"
    );
    let error = update_result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );

    // DELETE: Non-existent task list
    let delete_result = delete_task_list(&api_client, "nonexist", true).await;
    assert!(
        delete_result.is_err(),
        "Should return error for non-existent task list"
    );
    let error = delete_result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );

    // CREATE: Invalid project_id (foreign key constraint)
    let create_request = CreateTaskListRequest {
        title: "Test List".to_string(),
        project_id: "nonexist".to_string(),
        description: Some("Test description".to_string()),
        tags: Some(vec!["test".to_string()]),
        repo_ids: None,
    };
    let create_result = create_task_list(&api_client, create_request).await;
    assert!(
        create_result.is_err(),
        "Should fail with non-existent project_id"
    );
    let error = create_result.unwrap_err().to_string();
    assert!(
        error.contains("not found")
            || error.contains("404")
            || error.contains("project")
            || error.contains("foreign key")
            || error.contains("constraint")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_list_with_empty_title() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create task list
    let request = CreateTaskListRequest {
        title: "Original Title".to_string(),
        project_id: project_id.clone(),
        description: Some("Original description".to_string()),
        tags: Some(vec!["original".to_string()]),
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

    // Update with empty title - should preserve original title
    let update_request = UpdateTaskListRequest {
        title: "".to_string(),
        description: Some("Updated description".to_string()),
        status: None,
        tags: Some(vec!["updated".to_string()]),
        project_id: None,
        repo_ids: None,
    };
    let result = update_task_list(&api_client, list_id, update_request).await;
    assert!(result.is_ok(), "Should handle empty title gracefully");

    // Verify title was preserved
    let get_result = get_task_list(&api_client, list_id, "json")
        .await
        .expect("Failed to get task list");
    let updated_list: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(
        updated_list["title"], "Original Title",
        "Should preserve title when empty"
    );
    assert_eq!(updated_list["description"], "Updated description");
    assert_eq!(updated_list["tags"], json!(["updated"]));
}
