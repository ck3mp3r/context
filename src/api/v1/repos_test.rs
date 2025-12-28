//! Integration tests for Repo API endpoints.

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
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
    );
    routes::create_router(state, false)
}

/// Helper to parse JSON response body
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// GET /v1/repos - List Repos
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_repos_initially_empty() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/repos")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let repos = body["items"].as_array().expect("Expected items array");
    assert!(repos.is_empty());
    assert_eq!(body["total"], 0);
}

// =============================================================================
// POST /v1/repos - Create Repo
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_repo_returns_created() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:user/project",
                        "path": "/home/user/project"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["remote"], "github:user/project");
    assert_eq!(body["path"], "/home/user/project");
    assert!(body["id"].is_string());
    assert_eq!(body["id"].as_str().unwrap().len(), 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn create_repo_without_path() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:minimal/repo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["remote"], "github:minimal/repo");
    assert!(body["path"].is_null());
}

// =============================================================================
// GET /v1/repos?project_id=X - Filter by Project
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_repos_filtered_by_project_id() {
    let app = test_app().await;

    // Create two projects
    let project_a_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project A"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let project_a_body = json_body(project_a_response).await;
    let project_a_id = project_a_body["id"].as_str().unwrap();

    let project_b_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project B"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let project_b_body = json_body(project_b_response).await;
    let project_b_id = project_b_body["id"].as_str().unwrap();

    // Create repos: 2 for project A, 1 for project B, 1 for both
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:org/repo-a1",
                        "project_ids": [project_a_id]
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
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:org/repo-a2",
                        "project_ids": [project_a_id]
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
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:org/repo-b1",
                        "project_ids": [project_b_id]
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
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:org/repo-shared",
                        "project_ids": [project_a_id, project_b_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter by project_id for Project A
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/repos?project_id={}", project_a_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    // Should return 3 repos: 2 exclusive to A + 1 shared
    assert_eq!(body["total"], 3);
    assert_eq!(body["items"].as_array().unwrap().len(), 3);

    // Filter by project_id for Project B
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/repos?project_id={}", project_b_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    // Should return 2 repos: 1 exclusive to B + 1 shared
    assert_eq!(body["total"], 2);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_repos_filtered_by_nonexistent_project() {
    let app = test_app().await;

    // Create a repo without projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:org/some-repo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter by a non-existent project_id
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/repos?project_id=nonexist")
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

// =============================================================================
// GET /v1/repos/{id} - Get Repo
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_repo_returns_repo() {
    let app = test_app().await;

    // Create a repo first
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:test/get-repo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let repo_id = body["id"].as_str().unwrap();

    // Get that specific repo
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/repos/{}", repo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["id"], repo_id);
    assert_eq!(body["remote"], "github:test/get-repo");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_repo_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/repos/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// PATCH /v1/repos/{id} - Partial Update Repo
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn patch_repo_partial_remote_update() {
    let app = test_app().await;

    // Create a repo
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "remote": "https://github.com/original/repo",
                        "path": "/original/path",
                        "tags": ["tag1", "tag2"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let repo_id = created["id"].as_str().unwrap();

    // PATCH only the remote
    let patch_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/repos/{}", repo_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "remote": "https://github.com/updated/repo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);

    let patched = json_body(patch_response).await;

    // Remote should be updated
    assert_eq!(patched["remote"], "https://github.com/updated/repo");

    // Other fields should be preserved
    assert_eq!(patched["path"], "/original/path");
    assert_eq!(patched["tags"].as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_repo_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/v1/repos/notfound")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "remote": "https://github.com/new/repo"
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
// PUT /v1/repos/{id} - Update Repo
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn update_repo_returns_updated() {
    let app = test_app().await;

    // Create a repo first
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:original/repo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let repo_id = body["id"].as_str().unwrap();

    // Update it
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/repos/{}", repo_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:updated/repo",
                        "path": "/new/path"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["remote"], "github:updated/repo");
    assert_eq!(body["path"], "/new/path");
}

#[tokio::test(flavor = "multi_thread")]
async fn update_repo_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/repos/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:wont/work",
                        "path": null
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
// DELETE /v1/repos/{id} - Delete Repo
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn delete_repo_returns_no_content() {
    let app = test_app().await;

    // Create a repo to delete
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:to/delete"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let repo_id = body["id"].as_str().unwrap();

    // Delete it
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/repos/{}", repo_id))
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
                .uri(format!("/v1/repos/{}", repo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_repo_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/v1/repos/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// PATCH /v1/repos/{id} - Relationship Relinking
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn patch_repo_link_to_project() {
    let app = test_app().await;

    // Create a project
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test Project"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let project_body = json_body(project_response).await;
    let project_id = project_body["id"].as_str().unwrap().to_string();

    // Create a repo without relationships
    let repo_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:user/test-repo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let repo_body = json_body(repo_response).await;
    let repo_id = repo_body["id"].as_str().unwrap();

    // Verify no relationships initially
    assert!(repo_body["project_ids"].as_array().unwrap().is_empty());

    // PATCH to link to project
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/repos/{}", repo_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "project_ids": [project_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;

    // Verify relationship was added
    assert_eq!(body["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["project_ids"][0], project_id);
}
