//! Integration tests for TaskList API endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::notifier::{ChangeNotifier, UpdateMessage};
use crate::api::{AppState, routes};
use crate::db::{Database, SqliteDatabase};

/// Create a test app with an in-memory database
async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

    // Create a test project with known ID for API tests
    sqlx::query("INSERT OR IGNORE INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("test0000")
        .bind("Test Project")
        .bind("Default project for tests")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Create test project should succeed");

    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        ChangeNotifier::new(),
    );
    routes::create_router(state, false)
}

/// Helper to create test app with access to notifier for broadcast testing
async fn test_app_with_notifier() -> (axum::Router, ChangeNotifier) {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create a test project with known ID for API tests
    sqlx::query("INSERT OR IGNORE INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("test0000")
        .bind("Test Project")
        .bind("Default project for tests")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Create test project should succeed");

    let notifier = ChangeNotifier::new();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        notifier.clone(),
    );
    (routes::create_router(state, false), notifier)
}

/// Helper to parse JSON response body
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// List and Filter Comprehensive
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_and_filter_task_lists() {
    let app = test_app().await;

    // Test 1: Initially empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert!(body["items"].as_array().unwrap().is_empty());
    assert_eq!(body["total"], 0);

    // Create test project
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Test Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create task lists with various attributes
    for (i, (title, tags, status)) in [
        ("Alpha Sprint", vec!["work", "urgent"], "active"),
        ("Beta Sprint", vec!["work"], "active"),
        ("Gamma Sprint", vec!["personal"], "active"),
        ("Delta Sprint", vec!["work"], "archived"),
    ]
    .iter()
    .enumerate()
    {
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/task-lists")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "title": title,
                            "tags": tags,
                            "project_id": if i == 0 { &project_id } else { "test0000" }
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let created = json_body(create_response).await;
        let list_id = created["id"].as_str().unwrap();

        // Archive the last one
        if *status == "archived" {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("PATCH")
                        .uri(format!("/api/v1/task-lists/{}", list_id))
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&json!({"status": "archived"})).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
        }
    }

    // Test 2: Filter by project
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists?project_id={}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Alpha Sprint");

    // Test 3: Filter by tags
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?tags=urgent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Alpha Sprint");

    // Test 4: Combined filters (project + tags)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/task-lists?project_id={}&tags=work",
                    project_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Alpha Sprint");

    // Test 5: Filter by status
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?status=archived")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Delta Sprint");

    // Test 6: Ordering (title ascending)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?sort=title&order=asc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items[0]["title"], "Alpha Sprint");
    assert_eq!(items[1]["title"], "Beta Sprint");

    // Test 7: Pagination
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?limit=2&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    assert_eq!(body["total"], 4);
    assert_eq!(body["limit"], 2);
    assert_eq!(body["offset"], 0);
}

// =============================================================================
// CRUD Operations
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn crud_operations() {
    let app = test_app().await;

    // Create project and repo for relationship testing
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "CRUD Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let repo_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"remote": "github:test/repo"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let repo_id = json_body(repo_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Test 1: CREATE task list
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "New Task List",
                        "description": "Test description",
                        "tags": ["test"],
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let list_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["title"], "New Task List");
    assert_eq!(created["status"], "active");
    assert!(created["archived_at"].is_null());

    // Test 2: GET task list with relationships
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["id"], list_id);
    assert_eq!(body["project_id"], "test0000");
    assert!(body["repo_ids"].is_array());

    // Test 3: PATCH partial update - status to archived
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/task-lists/{}", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"status": "archived"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["status"], "archived");
    assert!(!body["archived_at"].is_null());
    assert_eq!(body["title"], "New Task List"); // Title unchanged

    // Test 4: PATCH relationship update
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/task-lists/{}", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "project_id": &project_id,
                        "repo_ids": [&repo_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["project_id"], project_id);
    assert_eq!(body["repo_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["repo_ids"][0], repo_id);

    // Test 5: PUT full update with relationships
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/task-lists/{}", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated Task List",
                        "description": "Updated description",
                        "project_id": &project_id,
                        "repo_ids": [&repo_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["title"], "Updated Task List");
    assert_eq!(body["project_id"], project_id);
    assert_eq!(body["repo_ids"][0], repo_id);

    // Test 6: GET stats endpoint
    // Create tasks with different statuses
    for (i, status) in ["backlog", "todo", "in_progress", "done"]
        .iter()
        .enumerate()
    {
        let task_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"title": format!("Task {}", i)})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let task = json_body(task_response).await;
        let task_id = task["id"].as_str().unwrap();

        if *status != "backlog" {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("PATCH")
                        .uri(format!("/api/v1/tasks/{}", task_id))
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&json!({"status": status})).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
        }
    }

    let stats_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/stats", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats = json_body(stats_response).await;
    assert_eq!(stats["total"], 4);
    assert_eq!(stats["backlog"], 1);
    assert_eq!(stats["todo"], 1);
    assert_eq!(stats["in_progress"], 1);
    assert_eq!(stats["done"], 1);

    // Test 7: DELETE task list
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/task-lists/{}", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

// =============================================================================
// FTS5 Search
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_task_lists() {
    let app = test_app().await;

    // Create task lists with searchable content
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Backend Sprint",
                        "description": "API development tasks",
                        "notes": "Critical deadline for stakeholders",
                        "tags": ["rust", "backend"],
                        "external_refs": ["owner/repo#123"],
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Python Data Pipeline",
                        "description": "ETL processing",
                        "tags": ["python"],
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust CLI Tools",
                        "description": "Command line utilities",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test 1: Search by title
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?q=rust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2);

    // Test 2: Search by description
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?q=ETL")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Python Data Pipeline");

    // Test 3: Search by notes
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?q=stakeholders")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Rust Backend Sprint");

    // Test 4: Search by tags
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?q=backend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);

    // Test 5: Search by external_refs
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?q=owner%2Frepo%23123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Rust Backend Sprint");

    // Test 6: Boolean operator AND
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?q=rust+AND+backend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Rust Backend Sprint");

    // Test 7: Search combined with status filter
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?q=rust&status=active")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
}

// =============================================================================
// WebSocket Broadcasts
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn websocket_broadcasts() {
    let (app, notifier) = test_app_with_notifier().await;

    // Test 1: CREATE broadcasts TaskListCreated
    let mut subscriber = notifier.subscribe();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "New Task List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let list_id = created["id"].as_str().unwrap().to_string();

    let msg = subscriber.recv().await.expect("Should receive broadcast");
    match msg {
        UpdateMessage::TaskListCreated { task_list_id } => {
            assert_eq!(task_list_id, list_id);
        }
        _ => panic!("Expected TaskListCreated, got {:?}", msg),
    }

    // Test 2: UPDATE broadcasts TaskListUpdated
    let mut subscriber = notifier.subscribe();

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/task-lists/{}", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Updated Task List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_response.status(), StatusCode::OK);

    let msg = subscriber.recv().await.expect("Should receive broadcast");
    match msg {
        UpdateMessage::TaskListUpdated { task_list_id } => {
            assert_eq!(task_list_id, list_id);
        }
        _ => panic!("Expected TaskListUpdated, got {:?}", msg),
    }

    // Test 3: DELETE broadcasts TaskListDeleted
    let mut subscriber = notifier.subscribe();

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/task-lists/{}", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let msg = subscriber.recv().await.expect("Should receive broadcast");
    match msg {
        UpdateMessage::TaskListDeleted { task_list_id } => {
            assert_eq!(task_list_id, list_id);
        }
        _ => panic!("Expected TaskListDeleted, got {:?}", msg),
    }
}
