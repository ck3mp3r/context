//! Integration tests for Task API endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::{AppState, routes};
use crate::db::{Database, SqliteDatabase};

async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

    // Create a test project for task lists
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
        crate::api::notifier::ChangeNotifier::new(),
    );
    routes::create_router(state, false)
}

async fn test_app_with_notifier() -> (axum::Router, crate::api::notifier::ChangeNotifier) {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

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

    let notifier = crate::api::notifier::ChangeNotifier::new();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        notifier.clone(),
    );
    (routes::create_router(state, false), notifier)
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// Status Transitions and Cascade
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn status_and_cascade_operations() {
    let app = test_app().await;

    // Create task list
    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        list.status(),
        StatusCode::CREATED,
        "Task list creation failed"
    );
    let list_id = json_body(list).await["id"].as_str().unwrap().to_string();

    // Create parent task
    let parent = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Parent Task"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        parent.status(),
        StatusCode::CREATED,
        "Parent task creation failed"
    );
    let parent_id = json_body(parent).await["id"].as_str().unwrap().to_string();

    // Create subtasks
    let sub1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Subtask 1",
                        "parent_id": &parent_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        sub1.status(),
        StatusCode::CREATED,
        "Subtask 1 creation failed"
    );
    let sub1_id = json_body(sub1).await["id"].as_str().unwrap().to_string();

    let sub2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Subtask 2",
                        "parent_id": &parent_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        sub2.status(),
        StatusCode::CREATED,
        "Subtask 2 creation failed"
    );
    let sub2_id = json_body(sub2).await["id"].as_str().unwrap().to_string();

    // Test 1: PATCH status to done sets completed_at
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", sub1_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"status": "done"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "PATCH to done failed");
    let body = json_body(response).await;
    assert_eq!(body["status"], "done");
    assert!(!body["completed_at"].is_null());

    // Test 2: Move back from done clears completed_at
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", sub1_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"status": "todo"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "PATCH back to todo failed"
    );
    let body = json_body(response).await;
    assert!(
        !body["completed_at"].is_null(),
        "completed_at should be preserved as historical record"
    );

    // Test 3: No cascade - move sub1 to in_progress
    app.clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", sub1_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"status": "in_progress"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Move parent to in_progress - should NOT cascade to sub2
    app.clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", parent_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"status": "in_progress"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let sub2_check = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tasks/{}", sub2_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let sub2_body = json_body(sub2_check).await;
    assert_eq!(sub2_body["status"], "backlog"); // Remains in backlog - no cascading!
}

// =============================================================================
// Type Filtering
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn type_filtering_comprehensive() {
    let app = test_app().await;

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let list_id = json_body(list).await["id"].as_str().unwrap().to_string();

    // Create 2 parent tasks
    let p1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Parent 1"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let p1_id = json_body(p1).await["id"].as_str().unwrap().to_string();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Parent 2"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Create 2 subtasks
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Subtask 1",
                        "parent_id": &p1_id
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
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Subtask 2",
                        "parent_id": &p1_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test 1: type=task returns only parents
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks?type=task", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);

    // Test 2: type=subtask returns only subtasks
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks?type=subtask", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);

    // Test 3: no type returns all
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 4);
}

// =============================================================================
// CRUD and Relationships
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn crud_and_relationships() {
    let app = test_app().await;

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let list_id = json_body(list).await["id"].as_str().unwrap().to_string();

    // Test 1: CREATE task
    let task = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Test Task"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(task.status(), StatusCode::CREATED);
    let task_body = json_body(task).await;
    let task_id = task_body["id"].as_str().unwrap().to_string();

    // Test 2: GET task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test 3: UPDATE task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "list_id": &list_id,
                        "title": "Updated Task"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["title"], "Updated Task");

    // Test 4: PATCH to remove parent_id
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"parent_id": ""})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert!(body["parent_id"].is_null());

    // Test 5: DELETE task
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

// =============================================================================
// WebSocket Broadcasts
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn websocket_broadcasts() {
    let (app, notifier) = test_app_with_notifier().await;

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let list_id = json_body(list).await["id"].as_str().unwrap().to_string();

    // Test 1: Create broadcasts TaskCreated
    let mut subscriber = notifier.subscribe();
    let task = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Test"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let task_id = json_body(task).await["id"].as_str().unwrap().to_string();

    let msg = subscriber
        .recv()
        .await
        .expect("Should receive create broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::TaskCreated { task_id: id } => {
            assert_eq!(id, task_id);
        }
        _ => panic!("Expected TaskCreated, got {:?}", msg),
    }

    // Test 2: Update broadcasts TaskUpdated
    let mut subscriber = notifier.subscribe();
    app.clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "list_id": &list_id,
                        "title": "Updated"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let msg = subscriber
        .recv()
        .await
        .expect("Should receive update broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::TaskUpdated { task_id: id } => {
            assert_eq!(id, task_id);
        }
        _ => panic!("Expected TaskUpdated, got {:?}", msg),
    }

    // Test 3: Delete broadcasts TaskDeleted
    let mut subscriber = notifier.subscribe();
    app.oneshot(
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/tasks/{}", task_id))
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();

    let msg = subscriber
        .recv()
        .await
        .expect("Should receive delete broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::TaskDeleted { task_id: id } => {
            assert_eq!(id, task_id);
        }
        _ => panic!("Expected TaskDeleted, got {:?}", msg),
    }
}

// =============================================================================
// FTS5 Search
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_comprehensive() {
    let app = test_app().await;

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let list_id = json_body(list).await["id"].as_str().unwrap().to_string();

    // Create diverse tasks
    for (title, desc, tags, refs) in [
        (
            "Implement auth",
            "JWT authentication",
            vec!["backend", "security"],
            vec!["github:org/api#123"],
        ),
        (
            "Design login UI",
            "Responsive login form",
            vec!["frontend"],
            vec!["JIRA-456"],
        ),
        (
            "Write docs",
            "API reference",
            vec!["docs"],
            vec!["LINEAR-789"],
        ),
        (
            "Fix security bug",
            "XSS vulnerability",
            vec!["security", "urgent"],
            vec!["github:org/web#99"],
        ),
    ] {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "title": title,
                            "description": desc,
                            "tags": tags,
                            "external_refs": refs
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test 1: Search by title
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks?q=auth", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);

    // Test 2: Search by description
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks?q=Responsive", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);

    // Test 3: Search by tags
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks?q=frontend", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);

    // Test 4: Search by external refs
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks?q=JIRA", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);

    // Test 5: Boolean AND
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/task-lists/{}/tasks?q=security%20AND%20urgent",
                    list_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);

    // Test 6: Empty query returns all
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks?q=", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 4);
}
