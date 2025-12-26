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
async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
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
// GET /v1/projects - List Projects
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_projects_returns_default_project() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let projects = body["items"].as_array().expect("Expected items array");

    // Should have at least the default project from migrations
    assert!(!projects.is_empty());
    assert!(projects.iter().any(|p| p["title"] == "Default"));
    assert!(body["total"].as_u64().unwrap() >= 1);
}

// =============================================================================
// POST /v1/projects - Create Project
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_project_returns_created() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/projects")
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

    // Validate timestamps are realistic (not hardcoded 2025-01-01 00:00:00)
    let created_at = body["created_at"].as_str().unwrap();
    let updated_at = body["updated_at"].as_str().unwrap();
    assert!(
        !created_at.ends_with("01-01 00:00:00"),
        "created_at should not be hardcoded: {}",
        created_at
    );
    assert!(
        !updated_at.ends_with("01-01 00:00:00"),
        "updated_at should not be hardcoded: {}",
        updated_at
    );

    // Validate timestamps are valid datetime strings (basic format check)
    assert!(
        created_at.len() >= 19,
        "created_at should be at least YYYY-MM-DD HH:MM:SS format"
    );
    assert!(
        updated_at.len() >= 19,
        "updated_at should be at least YYYY-MM-DD HH:MM:SS format"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_project_without_description() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/projects")
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
// GET /v1/projects/{id} - Get Project
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_project_returns_project() {
    let app = test_app().await;

    // First, list projects to get the default project ID
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let default_id = body["items"][0]["id"].as_str().unwrap();

    // Now get that specific project
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/projects/{}", default_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["id"], default_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_project_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/projects/nonexist")
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
// PUT /v1/projects/{id} - Update Project
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn update_project_returns_updated() {
    let app = test_app().await;

    // Get the default project ID
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let default_id = body["items"][0]["id"].as_str().unwrap();

    // Update it
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/projects/{}", default_id))
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

#[tokio::test(flavor = "multi_thread")]
async fn update_project_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/projects/nonexist")
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
// DELETE /v1/projects/{id} - Delete Project
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn delete_project_returns_no_content() {
    let app = test_app().await;

    // Create a project to delete
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/projects")
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
                .uri(format!("/v1/projects/{}", project_id))
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
                .uri(format!("/v1/projects/{}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_project_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/v1/projects/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
