//! Co-located tests for the restore REST handlers.
//!
//! Covers `POST /api/v1/{entity}/{id}/restore` for the six soft-deletable
//! entity types: the happy path returns the restored entity with HTTP 200,
//! an unknown id returns 404, and a restore blocked by a still-deleted
//! ancestor returns 400.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use utoipa::OpenApi;

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

/// Send a POST request with no body.
async fn post(app: &axum::Router, uri: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

/// Send a DELETE request.
async fn delete(app: &axum::Router, uri: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

/// Send a GET request.
async fn get(app: &axum::Router, uri: &str) -> axum::response::Response {
    app.clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

/// Create a project and return its ID.
async fn create_project(app: &axum::Router, title: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "title": title })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Create a task list under a project and return its ID.
async fn create_task_list(app: &axum::Router, project_id: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Restore List",
                        "project_id": project_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Create a task in a list and return its ID.
async fn create_task(app: &axum::Router, list_id: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{list_id}/tasks"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "title": "Restore Task" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

// =============================================================================
// Criterion 1: restoring a soft-deleted project returns it with HTTP 200
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn post_restore_project_returns_restored_project() {
    let app = test_app().await;
    let project_id = create_project(&app, "Restore Project").await;

    let response = delete(&app, &format!("/api/v1/projects/{project_id}")).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = get(&app, &format!("/api/v1/projects/{project_id}")).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = post(&app, &format!("/api/v1/projects/{project_id}/restore")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["id"], project_id);
    assert_eq!(body["title"], "Restore Project");

    let response = get(&app, &format!("/api/v1/projects/{project_id}")).await;
    assert_eq!(response.status(), StatusCode::OK);
}

// =============================================================================
// Criterion 2: restoring a soft-deleted task returns it with HTTP 200
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn post_restore_task_returns_restored_task() {
    let app = test_app().await;
    let project_id = create_project(&app, "Task Restore Project").await;
    let list_id = create_task_list(&app, &project_id).await;
    let task_id = create_task(&app, &list_id).await;

    let response = delete(&app, &format!("/api/v1/tasks/{task_id}")).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = post(&app, &format!("/api/v1/tasks/{task_id}/restore")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["id"], task_id);
    assert_eq!(body["title"], "Restore Task");

    let response = get(&app, &format!("/api/v1/tasks/{task_id}")).await;
    assert_eq!(response.status(), StatusCode::OK);
}

// =============================================================================
// Criterion 3: an unknown id returns HTTP 404
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn post_restore_unknown_id_returns_404() {
    let app = test_app().await;

    let response = post(&app, "/api/v1/projects/nonexist/restore").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = json_body(response).await;
    assert!(
        body["error"].as_str().unwrap().contains("not found"),
        "error should mention not found, got: {}",
        body["error"]
    );
}

// =============================================================================
// Criterion 4: a still-deleted ancestor returns HTTP 400
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn post_restore_task_with_deleted_list_returns_400() {
    let app = test_app().await;
    let project_id = create_project(&app, "Blocked Restore Project").await;
    let list_id = create_task_list(&app, &project_id).await;
    let task_id = create_task(&app, &list_id).await;

    // Deleting the list cascades to the task; the list stays soft-deleted.
    let response = delete(&app, &format!("/api/v1/task-lists/{list_id}")).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = post(&app, &format!("/api/v1/tasks/{task_id}/restore")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json_body(response).await;
    assert!(
        body["error"].as_str().unwrap().contains("soft-deleted"),
        "error should name the deleted ancestor, got: {}",
        body["error"]
    );
}

// =============================================================================
// Criterion 6: the OpenAPI document includes all six restore paths
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn openapi_doc_includes_restore_endpoints() {
    let doc = serde_json::to_value(routes::ApiDoc::openapi()).expect("OpenAPI doc serializes");
    let paths = doc["paths"].as_object().expect("paths object");

    for path in [
        "/api/v1/projects/{id}/restore",
        "/api/v1/repos/{id}/restore",
        "/api/v1/task-lists/{id}/restore",
        "/api/v1/tasks/{id}/restore",
        "/api/v1/notes/{id}/restore",
        "/api/v1/skills/{id}/restore",
    ] {
        assert!(
            paths.contains_key(path),
            "OpenAPI document must include {path}, got: {:?}",
            paths.keys().collect::<Vec<_>>()
        );
    }
}
