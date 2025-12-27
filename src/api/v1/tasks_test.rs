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

    let state = AppState::new(db);
    routes::create_router(state)
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

/// Helper to create a task list and return its ID
async fn create_task_list(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Test List", "project_id": "test0000"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn list_tasks_for_list() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Initially empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/task-lists/{}/tasks", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert!(body["items"].as_array().unwrap().is_empty());
    assert_eq!(body["total"], 0);

    // Create a task
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"content": "Do something"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Now has one task
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/task-lists/{}/tasks", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn create_task_returns_created() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "content": "Complete the feature",
                        "priority": 2,
                        "status": "in_progress"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["content"], "Complete the feature");
    assert_eq!(body["priority"], 2);
    assert_eq!(body["status"], "in_progress");
    assert_eq!(body["list_id"], list_id);
    assert!(body["id"].as_str().unwrap().len() == 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn create_task_minimal() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"content": "Quick task"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["content"], "Quick task");
    assert_eq!(body["status"], "backlog");
    assert!(body["priority"].is_null());
}

#[tokio::test(flavor = "multi_thread")]
async fn get_task_returns_task() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"content": "Test task"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let task_id = body["id"].as_str().unwrap();

    // Get
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["id"], task_id);
    assert_eq!(body["content"], "Test task");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_task_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/tasks/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// PATCH /v1/tasks/{id} - Partial Update Task
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn patch_task_partial_content_update() {
    let app = test_app().await;

    // Create task list and task
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({"name": "Test List", "project_id": "test0000"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let list = json_body(list_response).await;
    let list_id = list["id"].as_str().unwrap();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "content": "Original content",
                        "status": "backlog",
                        "priority": 3,
                        "tags": ["tag1"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(create_response).await;
    let task_id = created["id"].as_str().unwrap();

    // PATCH only content
    let patch_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "content": "Updated content"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);
    let patched = json_body(patch_response).await;

    assert_eq!(patched["content"], "Updated content");
    assert_eq!(patched["status"], "backlog");
    assert_eq!(patched["priority"], 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_task_status_to_done_sets_completed_at() {
    let app = test_app().await;

    // Create task list and task
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({"name": "Test List", "project_id": "test0000"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let list = json_body(list_response).await;
    let list_id = list["id"].as_str().unwrap();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "content": "Task to complete",
                        "status": "todo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(create_response).await;
    let task_id = created["id"].as_str().unwrap();
    assert!(created["completed_at"].is_null());

    // PATCH status to done
    let patch_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "status": "done"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);
    let patched = json_body(patch_response).await;

    assert_eq!(patched["status"], "done");
    assert!(
        patched["completed_at"].is_string(),
        "completed_at should be auto-set when status changes to done"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_task_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/v1/tasks/notfound")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({"content": "New"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_task_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/tasks/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"content": "Wont work"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_task_returns_no_content() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"content": "To delete"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let task_id = body["id"].as_str().unwrap();

    // Delete
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify gone
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_task_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/v1/tasks/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
