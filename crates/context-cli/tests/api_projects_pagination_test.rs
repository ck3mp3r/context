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
// Pagination, Sorting, and Filter Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_pagination() {
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

    // Test limit=1
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?limit=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["total"], 3);

    // Test offset=1
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?offset=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    assert_eq!(body["total"], 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_sorting() {
    let app = test_app().await;

    // Create projects with different titles
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Charlie"})).unwrap(),
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
                    serde_json::to_vec(&json!({"title": "Alpha"})).unwrap(),
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
                    serde_json::to_vec(&json!({"title": "Bravo"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test sort=title&order=asc
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?sort=title&order=asc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    let titles: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Alpha", "Bravo", "Charlie"]);

    // Test sort=title&order=desc
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?sort=title&order=desc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    let titles: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Charlie", "Bravo", "Alpha"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_invalid_sort_field() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?sort=invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Should handle gracefully - either return error or default sort
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_invalid_order() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?order=invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Should handle gracefully
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_negative_limit() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?limit=-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Should handle gracefully
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_negative_offset() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?offset=-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Should handle gracefully
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_large_limit() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?limit=9999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    // Should cap at max limit
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_large_offset() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?offset=9999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    // Should return empty items
    let body = json_body(response).await;
    assert!(body["items"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_repo_ids_filter() {
    let app = test_app().await;

    // Create project first
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Linked Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create repo linked to project
    let repo_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "https://github.com/test/repo",
                        "project_ids": [project_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let repo_id = json_body(repo_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create project not linked
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Unlinked Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter by repo_id
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects?repo_id={}", repo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Linked Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_note_ids_filter() {
    let app = test_app().await;

    // Create project first
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Note Linked Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create note linked to project
    let note_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test Note",
                        "content": "Test content",
                        "project_ids": [project_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let note_id = json_body(note_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Filter by note_id
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects?note_id={}", note_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Note Linked Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_task_list_ids_filter() {
    let app = test_app().await;

    // Create a project first
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Task List Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create a task list
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test List",
                        "project_id": project_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let list_id = json_body(list_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Filter by task_list_id
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects?task_list_id={}", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["id"], project_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_task_ids_filter() {
    let app = test_app().await;

    // Create a project
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Task Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create a task list
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test List",
                        "project_id": project_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let list_id = json_body(list_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create a task
    let task_response = app
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
    let task_id = json_body(task_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Filter by task_id
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects?task_id={}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["id"], project_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_skill_ids_filter() {
    let app = test_app().await;

    // Create a project
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Skill Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create a skill linked to project
    let skill_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "test-skill",
                        "description": "A test skill",
                        "content": "---\nname: test-skill\ndescription: A test skill\n---\n# Test\n\nTest content",
                        "project_ids": [project_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let skill_id = json_body(skill_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Filter by skill_id
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects?skill_id={}", skill_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["id"], project_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_combined_filters() {
    let app = test_app().await;

    // Create project with tags first
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Filtered Project",
                        "tags": ["backend", "rust"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create repo linked to project
    let repo_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "https://github.com/test/repo",
                        "project_ids": [project_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let repo_id = json_body(repo_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create another project
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Other Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Combined filter: tags + repo_id
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects?tags=backend&repo_id={}", repo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Filtered Project");
}
