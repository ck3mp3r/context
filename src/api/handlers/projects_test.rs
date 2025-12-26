//! Integration tests for Project API endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::{AppState, routes};
use crate::db::{Database, SqliteDatabase};

/// Create a test app with an in-memory database
fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory().expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");
    let state = AppState::new(db);
    routes::create_router(state)
}

/// Helper to parse JSON response body
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// GET /projects - List Projects
// =============================================================================

#[tokio::test]
async fn list_projects_returns_default_project() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let projects = body.as_array().expect("Expected array");

    // Should have at least the default project from migrations
    assert!(!projects.is_empty());
    assert!(projects.iter().any(|p| p["title"] == "Default"));
}

// =============================================================================
// POST /projects - Create Project
// =============================================================================

#[tokio::test]
async fn create_project_returns_created() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "New Project",
                        "description": "A test project"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["title"], "New Project");
    assert_eq!(body["description"], "A test project");
    assert!(body["id"].is_string());
    assert_eq!(body["id"].as_str().unwrap().len(), 8);
}

#[tokio::test]
async fn create_project_without_description() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Minimal Project"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["title"], "Minimal Project");
    assert!(body["description"].is_null());
}

// =============================================================================
// GET /projects/{id} - Get Project
// =============================================================================

#[tokio::test]
async fn get_project_returns_project() {
    let app = test_app();

    // First, list projects to get the default project ID
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let default_id = body[0]["id"].as_str().unwrap();

    // Now get that specific project
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/projects/{}", default_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["id"], default_id);
}

#[tokio::test]
async fn get_project_not_found() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/projects/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = json_body(response).await;
    assert!(body["error"].as_str().unwrap().contains("not found"));
}

// =============================================================================
// PUT /projects/{id} - Update Project
// =============================================================================

#[tokio::test]
async fn update_project_returns_updated() {
    let app = test_app();

    // Get the default project ID
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let default_id = body[0]["id"].as_str().unwrap();

    // Update it
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/projects/{}", default_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated Title",
                        "description": "Updated description"
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
    assert_eq!(body["description"], "Updated description");
}

#[tokio::test]
async fn update_project_not_found() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/projects/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Won't Work",
                        "description": null
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// DELETE /projects/{id} - Delete Project
// =============================================================================

#[tokio::test]
async fn delete_project_returns_no_content() {
    let app = test_app();

    // Create a project to delete
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "To Delete"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let project_id = body["id"].as_str().unwrap();

    // Delete it
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/projects/{}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/projects/{}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_project_not_found() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/projects/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
