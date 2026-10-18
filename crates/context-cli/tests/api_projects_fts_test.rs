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
// Search and FTS Query Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_search() {
    let app = test_app().await;

    // Create projects with searchable titles
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
                    serde_json::to_vec(&json!({"title": "React Frontend"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search for "rust"
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=rust")
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
async fn test_list_projects_with_query_and_tag_filter() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Backend",
                        "tags": ["rust", "backend"]
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
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Frontend",
                        "tags": ["rust", "frontend"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Combined query + tag filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=rust&tags=backend")
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
async fn test_list_projects_with_query_and_sort() {
    let app = test_app().await;

    // Create projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Zebra Project"})).unwrap(),
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
                    serde_json::to_vec(&json!({"title": "Alpha Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search + sort
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=project&sort=title&order=asc")
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
    assert_eq!(titles, vec!["Alpha Project", "Zebra Project"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_and_pagination() {
    let app = test_app().await;

    // Create 3 projects
    for i in 0..3 {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "title": format!("Project {}", i)
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Search + pagination
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=Project&limit=2&offset=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 3);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_no_results() {
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

    // Search for non-existent term
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_special_chars() {
    let app = test_app().await;

    // Create a project with special characters
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project with ñoño and 你好"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search with special characters
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=ñoño")
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
async fn test_list_projects_with_query_case_insensitive() {
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

    // Search with different case
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=rust")
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
async fn test_list_projects_with_query_partial_match() {
    let app = test_app().await;

    // Create a project
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

    // Partial match
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=Backend")
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
async fn test_list_projects_with_query_fts_boolean() {
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

    // Boolean AND search
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=rust+AND+backend")
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
async fn test_list_projects_with_query_fts_phrase() {
    let app = test_app().await;

    // Create a project
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "My Rust Backend Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Phrase search
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=%22Rust+Backend%22")
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
async fn test_list_projects_with_query_fts_prefix() {
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

    // Prefix search
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=Rust*")
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
async fn test_list_projects_with_query_fts_exclude() {
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

    // Exclude search
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
async fn test_list_projects_with_query_fts_complex() {
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

    // Complex: (rust OR python) AND backend
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
async fn test_list_projects_with_query_fts_empty() {
    let app = test_app().await;

    // Empty query should return all
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_whitespace() {
    let app = test_app().await;

    // Whitespace-only query should return all
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=%20%20")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_special_regex_chars() {
    let app = test_app().await;

    // Special regex characters should be handled
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=%5B.*%5D")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_very_long() {
    let app = test_app().await;

    // Very long query should be handled
    let long_query = "a".repeat(1000);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects?q={}", long_query))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_unicode() {
    let app = test_app().await;

    // Unicode query should be handled
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=%F0%9F%8E%89")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_query_fts_sql_injection() {
    let app = test_app().await;

    // SQL injection attempt should be handled safely
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=%27%3B%20DROP%20TABLE%20project%3B--")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
