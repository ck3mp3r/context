//! Integration tests for Project API endpoints.
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use context_server::api::{AppState, routes};
use context_sync::{MockGitOps, SyncManager};
use tempfile::TempDir;

/// Create a test app with an in-memory database
async fn test_app() -> axum::Router {
    let db = common::setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let state = AppState::new(
        db,
        SyncManager::new(MockGitOps::new()),
        context_server::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    routes::create_router(state, false)
}

/// Helper to parse JSON response body
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// FTS Boolean Operator Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operators() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test OR operator
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=rust+OR+backend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_and_or() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend API"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Python Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Complex boolean: (rust OR python) AND backend
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=(rust+OR+python)+AND+backend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
    let titles: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Rust Backend API"));
    assert!(titles.contains(&"Python Backend"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_not() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test NOT operator
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=rust+NOT+backend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Rust Frontend");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_combined() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend API"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Python Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Combined: (rust OR python) AND backend NOT frontend
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=(rust+OR+python)+AND+backend+NOT+frontend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
    let titles: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Rust Backend API"));
    assert!(titles.contains(&"Python Backend"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_nested() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend API"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Python Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Nested: (rust AND (backend OR frontend)) OR python
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=(rust+AND+(backend+OR+frontend))+OR+python")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_invalid() {
    let app = test_app().await;

    // Invalid boolean expression should be handled gracefully
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=AND+OR+NOT")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_mixed_case() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Mixed case operators
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=rust+And+backend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_as_word() {
    let app = test_app().await;

    // Create a project with "and" in the title
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust and Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search for "and" as a word (not operator)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=%22and%22")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_escape() {
    let app = test_app().await;

    // Create a project
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Escaped operators
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=%22rust%22+%22and%22+%22backend%22")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 0); // "and" not in title
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_phrase() {
    let app = test_app().await;

    // Create a project
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust and Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Phrase search with "and"
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=%22rust+and+backend%22")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_not_phrase() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // NOT with phrase
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=rust+NOT+%22frontend%22")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Rust Backend");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_and_or_not() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend API"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Python Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // AND + OR + NOT combined
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=(rust+OR+python)+AND+backend+NOT+frontend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
    let titles: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Rust Backend API"));
    assert!(titles.contains(&"Python Backend"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_nested_and_or() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend API"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Python Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Nested: (rust AND (backend OR frontend)) OR python
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=(rust+AND+(backend+OR+frontend))+OR+python")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_nested_and_or_not() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend API"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Python Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Nested: (rust AND (backend OR frontend)) OR (python NOT backend)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=(rust+AND+(backend+OR+frontend))+OR+(python+NOT+backend)")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
    let titles: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Rust Backend API"));
    assert!(titles.contains(&"Rust Frontend"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_nested_deep() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend API"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Python Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Deeply nested: ((rust AND backend) OR (python AND backend)) NOT frontend
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=((rust+AND+backend)+OR+(python+AND+backend))+NOT+frontend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
    let titles: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Rust Backend API"));
    assert!(titles.contains(&"Python Backend"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_nested_mixed() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend API"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Python Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Mixed: (rust AND (backend OR frontend)) OR (python AND backend)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=(rust+AND+(backend+OR+frontend))+OR+(python+AND+backend)")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_nested_all() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend API"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Python Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // All operators: (rust AND (backend OR frontend)) OR (python AND backend) NOT frontend
    // Note: FTS5 gives NOT higher precedence than OR, so this is parsed as:
    //   (rust AND (backend OR frontend)) OR ((python AND backend) NOT frontend)
    // Which returns 3 results (Rust Backend API, Rust Frontend, Python Backend)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(
                    "/api/v1/projects?q=(rust+AND+(backend+OR+frontend))+OR+(python+AND+backend)+NOT+frontend",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 3);
    let titles: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Rust Backend API"));
    assert!(titles.contains(&"Rust Frontend"));
    assert!(titles.contains(&"Python Backend"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_boolean_operator_nested_all_three() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Backend API"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Rust Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Python Backend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // All three: (rust AND (backend OR frontend)) OR (python AND backend) NOT frontend
    // Note: FTS5 gives NOT higher precedence than OR, so this returns 3 results
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(
                    "/api/v1/projects?q=(rust+AND+(backend+OR+frontend))+OR+(python+AND+backend)+NOT+frontend",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 3);
    let titles: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Rust Backend API"));
    assert!(titles.contains(&"Rust Frontend"));
    assert!(titles.contains(&"Python Backend"));
}
