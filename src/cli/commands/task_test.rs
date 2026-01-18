use crate::cli::api_client::ApiClient;
use crate::cli::commands::task::*;
use crate::cli::commands::task_list::{CreateTaskListRequest, create_task_list};
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use serde_json::json;
use tokio::net::TcpListener;

// =============================================================================
// Unit Tests - Request Serialization
// =============================================================================

#[test]
fn test_update_request_parent_id_serialization() {
    // Test empty string converts to Some(None) - removes parent
    let parent_id_empty = Some("".to_string()).map(|s| if s.is_empty() { None } else { Some(s) });
    let req1 = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id: parent_id_empty,
        tags: None,
        external_refs: None,
        list_id: None,
    };
    assert_eq!(req1.parent_id, Some(None));
    assert!(
        serde_json::to_string(&req1)
            .unwrap()
            .contains("\"parent_id\":null")
    );

    // Test non-empty sets value
    let parent_id_set =
        Some("parent123".to_string()).map(|s| if s.is_empty() { None } else { Some(s) });
    let req2 = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id: parent_id_set,
        tags: None,
        external_refs: None,
        list_id: None,
    };
    assert_eq!(req2.parent_id, Some(Some("parent123".to_string())));

    // Test None omits field
    let req3 = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id: None,
        tags: None,
        external_refs: None,
        list_id: None,
    };
    assert!(!serde_json::to_string(&req3).unwrap().contains("parent_id"));
}

// =============================================================================
// Integration Tests - Consolidated Essential Tests
// =============================================================================

async fn spawn_test_server() -> (String, String, tokio::task::JoinHandle<()>) {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

    let project_id = sqlx::query_scalar::<_, String>(
        "INSERT INTO project (id, title, description, tags, created_at, updated_at) 
         VALUES ('test0000', 'Test Project', 'Test project for CLI tests', '[]', datetime('now'), datetime('now')) 
         RETURNING id"
    )
    .fetch_one(db.pool())
    .await
    .expect("Failed to create test project");

    let state = crate::api::AppState::new(
        db,
        crate::sync::SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
    );
    let app = crate::api::routes::create_router(state, false);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    (url, project_id, handle)
}

async fn create_test_task_list(api_url: &str, project_id: &str) -> String {
    let api_client = ApiClient::new(Some(api_url.to_string()));
    let request = CreateTaskListRequest {
        title: "Test Task List".to_string(),
        project_id: project_id.to_string(),
        description: Some("Task list for testing".to_string()),
        tags: Some(vec!["test".to_string()]),
        repo_ids: None,
    };
    let result = create_task_list(&api_client, request)
        .await
        .expect("Failed to create task list");
    result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap()
        .to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_crud_operations() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // CREATE: Task with ALL fields populated
    let create_req = CreateTaskRequest {
        title: "Implement OAuth2 Authentication".to_string(),
        description: Some("Add OAuth2 login with Google and GitHub providers".to_string()),
        priority: Some(1),
        tags: Some(vec![
            "security".to_string(),
            "auth".to_string(),
            "p1".to_string(),
        ]),
        external_refs: Some(vec![
            "AUTH-456".to_string(),
            "github.com/org/repo#123".to_string(),
        ]),
        parent_id: None,
    };
    let create_result = create_task(&api_client, &list_id, create_req).await;
    assert!(create_result.is_ok());
    let task_id = create_result
        .unwrap()
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap()
        .to_string();

    // GET: Verify all fields persisted (JSON format)
    let get_json = get_task(&api_client, &task_id, "json")
        .await
        .expect("Failed to get task");
    let task: serde_json::Value = serde_json::from_str(&get_json).unwrap();
    assert_eq!(task["title"], "Implement OAuth2 Authentication");
    assert_eq!(
        task["description"],
        "Add OAuth2 login with Google and GitHub providers"
    );
    assert_eq!(task["priority"], 1);
    assert_eq!(task["tags"], json!(["security", "auth", "p1"]));
    assert_eq!(
        task["external_refs"],
        json!(["AUTH-456", "github.com/org/repo#123"])
    );
    assert_eq!(task["status"], "backlog");

    // GET: Table format
    let get_table = get_task(&api_client, &task_id, "table")
        .await
        .expect("Failed to get table");
    assert!(get_table.contains("OAuth2 Authentication"));
    assert!(get_table.contains("Priority"));
    assert!(get_table.contains("External Refs"));

    // UPDATE: Change multiple fields
    let update_req = UpdateTaskRequest {
        title: Some("Implement OAuth2 + SAML Authentication".to_string()),
        description: Some("Extended to support SAML SSO".to_string()),
        priority: Some(2),
        tags: Some(vec![
            "security".to_string(),
            "auth".to_string(),
            "enterprise".to_string(),
        ]),
        status: None,
        parent_id: None,
        external_refs: None,
        list_id: None,
    };
    update_task(&api_client, &task_id, update_req)
        .await
        .expect("Failed to update");
    let updated = serde_json::from_str::<serde_json::Value>(
        &get_task(&api_client, &task_id, "json").await.unwrap(),
    )
    .unwrap();
    assert_eq!(updated["title"], "Implement OAuth2 + SAML Authentication");
    assert_eq!(updated["priority"], 2);
    assert_eq!(updated["tags"], json!(["security", "auth", "enterprise"]));

    // DELETE: Requires --force flag
    assert!(delete_task(&api_client, &task_id, false).await.is_err());
    assert!(delete_task(&api_client, &task_id, true).await.is_ok());
    assert!(get_task(&api_client, &task_id, "json").await.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_list_with_comprehensive_filters() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create diverse tasks
    for i in 1..=5 {
        create_task(
            &api_client,
            &list_id,
            CreateTaskRequest {
                title: format!("Backend Task {}", i),
                description: Some(format!("Description for task {}", i)),
                priority: Some(i),
                tags: Some(vec![
                    "backend".to_string(),
                    if i % 2 == 0 {
                        "api".to_string()
                    } else {
                        "db".to_string()
                    },
                ]),
                external_refs: Some(vec![format!("TASK-{}", i)]),
                parent_id: None,
            },
        )
        .await
        .expect("Create failed");
    }

    // Create frontend tasks
    for i in 1..=3 {
        create_task(
            &api_client,
            &list_id,
            CreateTaskRequest {
                title: format!("Frontend Task {}", i),
                description: Some("UI work".to_string()),
                priority: Some(2),
                tags: Some(vec!["frontend".to_string(), "react".to_string()]),
                external_refs: None,
                parent_id: None,
            },
        )
        .await
        .expect("Create failed");
    }

    // LIST with ALL filters applied: query, status, tags, type, limit, offset, sort, order
    let result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: Some("Backend"),
            status: Some("backlog"),
            tags: Some("backend"),
            r#type: Some("task"), // Only top-level tasks
            limit: Some(10),
            offset: Some(0),
            sort: Some("priority"),
            order: Some("asc"),
            parent_id: None,
        },
        "json",
    )
    .await
    .expect("List failed");

    let tasks: serde_json::Value = serde_json::from_str(&result).unwrap();
    let tasks_arr = tasks.as_array().unwrap();
    assert!(tasks_arr.len() <= 5, "Should only get backend tasks");
    assert!(
        tasks_arr
            .iter()
            .all(|t| t["title"].as_str().unwrap().contains("Backend"))
    );

    // Test table format with results (hits table rendering code)
    let table_result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: Some("Backend"),
            status: None,
            tags: None,
            r#type: None,
            limit: Some(3),
            offset: None,
            sort: None,
            order: None,
            parent_id: None,
        },
        "table",
    )
    .await
    .unwrap();
    assert!(table_result.contains("ID"));
    assert!(table_result.contains("Title"));
    assert!(table_result.contains("Status"));
    assert!(table_result.contains("Priority"));
    assert!(table_result.contains("Backend Task"));

    // Test table format with empty result
    let empty_result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            query: Some("NonexistentSearchTerm"),
            status: None,
            tags: None,
            r#type: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
            parent_id: None,
        },
        "table",
    )
    .await
    .unwrap();
    assert_eq!(empty_result, "No tasks found.");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_status_transitions() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create task with data
    let create_result = create_task(
        &api_client,
        &list_id,
        CreateTaskRequest {
            title: "Fix Database Connection Pool Leak".to_string(),
            description: Some("Pool not releasing connections properly".to_string()),
            priority: Some(1),
            tags: Some(vec![
                "bug".to_string(),
                "database".to_string(),
                "critical".to_string(),
            ]),
            external_refs: Some(vec!["BUG-789".to_string()]),
            parent_id: None,
        },
    )
    .await
    .unwrap();
    let task_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap();

    // Transition: backlog -> todo
    transition_task(&api_client, task_id, "todo")
        .await
        .expect("Transition failed");
    let task = serde_json::from_str::<serde_json::Value>(
        &get_task(&api_client, task_id, "json").await.unwrap(),
    )
    .unwrap();
    assert_eq!(task["status"], "todo");
    assert_eq!(task["tags"], json!(["bug", "database", "critical"])); // Tags persist

    // Transition: todo -> in_progress
    transition_task(&api_client, task_id, "in_progress")
        .await
        .expect("Transition failed");

    // Complete task
    complete_task(&api_client, task_id)
        .await
        .expect("Complete failed");
    let completed = serde_json::from_str::<serde_json::Value>(
        &get_task(&api_client, task_id, "json").await.unwrap(),
    )
    .unwrap();
    assert_eq!(completed["status"], "done");
    assert_eq!(completed["external_refs"], json!(["BUG-789"])); // External refs persist
}

#[tokio::test(flavor = "multi_thread")]
async fn test_subtasks_with_full_data() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create parent task with full data
    let parent_result = create_task(
        &api_client,
        &list_id,
        CreateTaskRequest {
            title: "Build User Management System".to_string(),
            description: Some("Complete user CRUD with permissions".to_string()),
            priority: Some(1),
            tags: Some(vec!["epic".to_string(), "users".to_string()]),
            external_refs: Some(vec!["EPIC-100".to_string()]),
            parent_id: None,
        },
    )
    .await
    .unwrap();
    let parent_id = parent_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap()
        .to_string();

    // Create subtask with full data
    let subtask_result = create_task(
        &api_client,
        &list_id,
        CreateTaskRequest {
            title: "Implement User Creation API".to_string(),
            description: Some("POST /api/users endpoint with validation".to_string()),
            priority: Some(2),
            tags: Some(vec!["api".to_string(), "backend".to_string()]),
            external_refs: Some(vec!["TASK-101".to_string()]),
            parent_id: Some(parent_id.clone()),
        },
    )
    .await
    .unwrap();
    let subtask_id = subtask_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap()
        .to_string();

    // Verify subtask relationship (JSON format)
    let subtask = serde_json::from_str::<serde_json::Value>(
        &get_task(&api_client, &subtask_id, "json").await.unwrap(),
    )
    .unwrap();
    assert_eq!(subtask["parent_id"], parent_id);
    assert_eq!(subtask["title"], "Implement User Creation API");
    assert_eq!(subtask["tags"], json!(["api", "backend"]));

    // Verify subtask in table format (hits parent_id display code)
    let subtask_table = get_task(&api_client, &subtask_id, "table").await.unwrap();
    assert!(subtask_table.contains("Parent ID"));
    assert!(subtask_table.contains(&parent_id));

    // List subtasks filtered by parent_id
    let subtasks_result = list_tasks(
        &api_client,
        &list_id,
        ListTasksFilter {
            parent_id: Some(&parent_id),
            query: None,
            status: None,
            tags: None,
            r#type: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
        "json",
    )
    .await
    .unwrap();
    let subtasks = serde_json::from_str::<serde_json::Value>(&subtasks_result).unwrap();
    assert_eq!(subtasks.as_array().unwrap().len(), 1);
    assert_eq!(subtasks[0]["parent_id"], parent_id);

    // Remove parent (convert subtask to task)
    update_task(
        &api_client,
        &subtask_id,
        UpdateTaskRequest {
            parent_id: Some(None), // Explicitly remove parent
            title: None,
            description: None,
            status: None,
            priority: None,
            tags: None,
            external_refs: None,
            list_id: None,
        },
    )
    .await
    .expect("Update failed");
    let converted = serde_json::from_str::<serde_json::Value>(
        &get_task(&api_client, &subtask_id, "json").await.unwrap(),
    )
    .unwrap();
    assert!(converted["parent_id"].is_null());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_error_handling() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Get non-existent task
    let get_result = get_task(&api_client, "nonexistent", "json").await;
    assert!(get_result.is_err());

    // Update non-existent task
    let update_result = update_task(
        &api_client,
        "nonexistent",
        UpdateTaskRequest {
            title: Some("New Title".to_string()),
            description: None,
            status: None,
            priority: None,
            parent_id: None,
            tags: None,
            external_refs: None,
            list_id: None,
        },
    )
    .await;
    assert!(update_result.is_err());

    // Delete non-existent task
    let delete_result = delete_task(&api_client, "nonexistent", true).await;
    assert!(delete_result.is_err());
}
