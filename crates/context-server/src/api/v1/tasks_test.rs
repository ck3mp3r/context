//! Co-located tests for task REST handlers.
//!
//! Covers the state-machine bypass fix: PUT/PATCH ignore `status`, and
//! `POST /api/v1/tasks/{id}/transitions` enforces the state machine.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::{AppState, routes};
use context_core::Database;
use context_db::SqliteDatabase;
use context_sync::{MockGitOps, SyncManager};
use tempfile::TempDir;

/// Create a test app backed by an in-memory SQLite database.
async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    let temp_dir = TempDir::new().unwrap();
    let state = AppState::new(
        db,
        SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    routes::create_router(state, false)
}

/// Parse a JSON response body.
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

/// Create a project, task list, and task; return the task ID.
async fn create_task(app: &axum::Router) -> String {
    let project = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Transition Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project).await["id"].as_str().unwrap().to_string();

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Transition List",
                        "project_id": project_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let list_id = json_body(list).await["id"].as_str().unwrap().to_string();

    let task = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Transition Task"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    json_body(task).await["id"].as_str().unwrap().to_string()
}

/// Send a POST transition request.
async fn post_transition(
    app: &axum::Router,
    task_id: &str,
    status: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{}/transitions", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "status": status })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

// =============================================================================
// Criterion 1: PUT ignores the status field
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn put_ignores_status_field() {
    let app = test_app().await;
    let task_id = create_task(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated Title",
                        "status": "done"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["title"], "Updated Title");
    assert_eq!(body["status"], "backlog", "status must be ignored on PUT");
}

// =============================================================================
// Criterion 2: PATCH ignores the status field
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn patch_ignores_status_field() {
    let app = test_app().await;
    let task_id = create_task(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Patched Title",
                        "status": "done"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["title"], "Patched Title");
    assert_eq!(body["status"], "backlog", "status must be ignored on PATCH");
}

// =============================================================================
// Criterion 3: valid transition returns HTTP 200
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn post_transition_valid_returns_200() {
    let app = test_app().await;
    let task_id = create_task(&app).await;

    // backlog -> todo
    let response = post_transition(&app, &task_id, "todo").await;
    assert_eq!(response.status(), StatusCode::OK);

    // todo -> in_progress
    let response = post_transition(&app, &task_id, "in_progress").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["task_id"], task_id);
    assert_eq!(body["status"], "in_progress");

    // Verify the status persisted
    let task = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json_body(task).await["status"], "in_progress");
}

// =============================================================================
// Criterion 4: invalid transition returns HTTP 400 with invalid_transition
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn post_transition_invalid_returns_400() {
    let app = test_app().await;
    let task_id = create_task(&app).await;

    // backlog -> done is not allowed
    let response = post_transition(&app, &task_id, "done").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json_body(response).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("invalid_transition"),
        "error should mention invalid_transition, got: {}",
        body["error"]
    );
}

// =============================================================================
// Criterion 5: non-existent task returns HTTP 404
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn post_transition_nonexistent_task_returns_404() {
    let app = test_app().await;

    let response = post_transition(&app, "nonexist", "todo").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Criterion 6: invalid status string returns HTTP 400
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn post_transition_invalid_status_returns_400() {
    let app = test_app().await;
    let task_id = create_task(&app).await;

    let response = post_transition(&app, &task_id, "not_a_status").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =============================================================================
// Transition graph endpoint
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_transitions_graph_from_status() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-transitions?from=backlog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["from"], "backlog");
    let allowed: Vec<&str> = body["allowed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(allowed, vec!["todo", "in_progress", "cancelled"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_transitions_graph_full() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-transitions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    let transitions = body["transitions"].as_object().unwrap();
    assert_eq!(transitions.len(), 6);
    for status in [
        "backlog",
        "todo",
        "in_progress",
        "review",
        "done",
        "cancelled",
    ] {
        assert!(
            transitions.contains_key(status),
            "missing status {}",
            status
        );
    }
    let in_progress: Vec<&str> = transitions["in_progress"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(in_progress, vec!["todo", "review", "done", "cancelled"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_transitions_graph_invalid_status_returns_400() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-transitions?from=invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =============================================================================
// OpenAPI registration
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn openapi_doc_includes_transition_endpoint() {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    let temp_dir = TempDir::new().unwrap();
    let state = AppState::new(
        db,
        SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    // Building the router with docs enabled exercises ApiDoc::openapi(),
    // which panics if a referenced schema is not registered.
    let _app = routes::create_router(state, true);
}
