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
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
    );
    routes::create_router(state, false)
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
async fn list_projects_initially_empty() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let projects = body["items"].as_array().expect("Expected items array");

    // Should be empty - no default project in migration
    assert_eq!(projects.len(), 0);
    assert_eq!(body["total"].as_u64().unwrap(), 0);
}

// =============================================================================
// REPO RELATIONSHIP TESTS - TDD for Missing Functionality
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn update_repo_with_project_relationships_should_work() {
    let app = test_app().await;

    // Create a project first
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
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

    let body = json_body(response).await;
    let project_id = body["id"].as_str().unwrap();

    // Create a repo
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "https://github.com/test/repo.git"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let repo_body = json_body(response).await;
    let repo_id = repo_body["id"].as_str().unwrap();

    // Update repo with project relationship - THIS SHOULD WORK BUT CURRENTLY FAILS
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/repos/{}", repo_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "https://github.com/test/repo.git",
                        "path": "/tmp/test",
                        "tags": ["test"],
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
    // THIS ASSERTION WILL FAIL - project_ids field missing from UpdateRequest/Response DTOs
    assert!(body["project_ids"].is_array());
    assert_eq!(body["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["project_ids"][0], project_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_note_with_project_relationships_should_work() {
    let app = test_app().await;

    // Create a project
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
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

    let body = json_body(response).await;
    let project_id = body["id"].as_str().unwrap();

    // Create a repo first (notes require repo_id)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "https://github.com/test/repo.git"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let repo_body = json_body(response).await;
    let repo_id = repo_body["id"].as_str().unwrap();

    // Create a note
    let response = app
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
                        "repo_id": repo_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let note_body = json_body(response).await;
    let note_id = note_body["id"].as_str().unwrap();

    // Update note with project relationship - THIS SHOULD WORK BUT CURRENTLY FAILS
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated Note",
                        "content": "Updated content",
                        "tags": ["test"],
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
    // THIS ASSERTION WILL FAIL - project_ids field missing from UpdateRequest/Response DTOs
    assert!(body["project_ids"].is_array());
    assert_eq!(body["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["project_ids"][0], project_id);
}

// =============================================================================
// PATCH /v1/projects/{id} - Partial Update Project
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn patch_project_partial_title_update() {
    let app = test_app().await;

    // Create a project
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Original Title",
                        "description": "Original description",
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
    let project_id = created["id"].as_str().unwrap();

    // PATCH only the title
    let patch_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Updated Title"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);

    let patched = json_body(patch_response).await;

    // Title should be updated
    assert_eq!(patched["title"], "Updated Title");

    // Other fields should be preserved
    assert_eq!(patched["description"], "Original description");
    assert_eq!(patched["tags"].as_array().unwrap().len(), 2);
    assert_eq!(patched["tags"][0], "tag1");
    assert_eq!(patched["tags"][1], "tag2");
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_project_omit_field_preserves_it() {
    let app = test_app().await;

    // Create a project with description
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Test Project",
                        "description": "Has description"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(create_response).await;
    let project_id = created["id"].as_str().unwrap();
    assert!(created["description"].is_string());

    // PATCH - omit description field (no change)
    let patch_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Test Project Updated"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);

    let patched = json_body(patch_response).await;

    // Description should still be there (unchanged)
    assert_eq!(patched["description"], "Has description");

    // Title should be updated
    assert_eq!(patched["title"], "Test Project Updated");
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_project_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/projects/notfound")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "New Title"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_project_empty_body_preserves_all() {
    let app = test_app().await;

    // Create a project
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Original",
                        "description": "Desc",
                        "tags": ["tag1"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(create_response).await;
    let project_id = created["id"].as_str().unwrap();

    // PATCH with empty body
    let patch_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&json!({})).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);

    let patched = json_body(patch_response).await;

    // Everything should be unchanged
    assert_eq!(patched["title"], "Original");
    assert_eq!(patched["description"], "Desc");
    assert_eq!(patched["tags"].as_array().unwrap().len(), 1);
}

// =============================================================================
// WebSocket Broadcast Tests
// =============================================================================

/// Helper to create test app with access to notifier for broadcast testing
async fn test_app_with_notifier() -> (axum::Router, crate::api::notifier::ChangeNotifier) {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let notifier = crate::api::notifier::ChangeNotifier::new();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        notifier.clone(),
    );
    (routes::create_router(state, false), notifier)
}

#[tokio::test(flavor = "multi_thread")]
async fn create_project_broadcasts_notification() {
    let (app, notifier) = test_app_with_notifier().await;
    let mut subscriber = notifier.subscribe();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "New Project",
                        "description": "Test project"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_body(response).await;
    let project_id = created["id"].as_str().unwrap();

    // Should receive ProjectCreated broadcast
    let msg = subscriber.recv().await.expect("Should receive broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::ProjectCreated { project_id: id } => {
            assert_eq!(id, project_id);
        }
        _ => panic!("Expected ProjectCreated message, got {:?}", msg),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn update_project_broadcasts_notification() {
    let (app, notifier) = test_app_with_notifier().await;

    // Create a project first
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Original Project"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(create_response).await;
    let project_id = created["id"].as_str().unwrap().to_string();

    // Subscribe AFTER creation to avoid receiving create notification
    let mut subscriber = notifier.subscribe();

    // Update the project
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Updated Project",
                        "description": "Updated description"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Should receive ProjectUpdated broadcast
    let msg = subscriber.recv().await.expect("Should receive broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::ProjectUpdated { project_id: id } => {
            assert_eq!(id, project_id);
        }
        _ => panic!("Expected ProjectUpdated message, got {:?}", msg),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_project_broadcasts_notification() {
    let (app, notifier) = test_app_with_notifier().await;

    // Create a project first
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Project to Delete"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(create_response).await;
    let project_id = created["id"].as_str().unwrap().to_string();

    // Subscribe AFTER creation
    let mut subscriber = notifier.subscribe();

    // Delete the project
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/projects/{}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Should receive ProjectDeleted broadcast
    let msg = subscriber.recv().await.expect("Should receive broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::ProjectDeleted { project_id: id } => {
            assert_eq!(id, project_id);
        }
        _ => panic!("Expected ProjectDeleted message, got {:?}", msg),
    }
}

// =============================================================================
// External Reference Support
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_project_with_external_ref() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "GitHub Project",
                        "description": "Project linked to GitHub",
                        "external_refs": ["owner/repo#123"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["title"], "GitHub Project");
    assert_eq!(body["external_refs"], json!(["owner/repo#123"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn update_project_external_ref() {
    let app = test_app().await;

    // Create a project without external_ref
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project Without Ref"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(create_response).await;
    let project_id = created["id"].as_str().unwrap();
    assert!(created["external_refs"].as_array().unwrap().is_empty());

    // Update to add external_ref
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project With Ref",
                        "external_refs": json!(["JIRA-456"])
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["title"], "Project With Ref");
    assert_eq!(body["external_refs"], json!(["JIRA-456"]));

    // Verify via GET
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects/{}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["external_refs"], json!(["JIRA-456"]));
}

// =============================================================================
// FTS5 Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn search_projects_by_title() {
    let app = test_app().await;

    // Create test projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Backend API",
                        "description": "A web API built with Rust"
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
                        "title": "Python Data Pipeline",
                        "description": "Data processing pipeline"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search for "rust"
    let response = app
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
    let projects = body["items"].as_array().expect("Expected items array");

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["title"], "Rust Backend API");
}

#[tokio::test(flavor = "multi_thread")]
async fn search_projects_by_description() {
    let app = test_app().await;

    // Create test projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project Alpha",
                        "description": "Machine learning research project"
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
                        "title": "Project Beta",
                        "description": "Frontend web application"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search by description
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=machine+learning")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let projects = body["items"].as_array().expect("Expected items array");

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["title"], "Project Alpha");
}

#[tokio::test(flavor = "multi_thread")]
async fn search_projects_by_tags() {
    let app = test_app().await;

    // Create test projects with tags
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Frontend Project",
                        "tags": ["react", "typescript", "frontend"]
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
                        "title": "Backend Project",
                        "tags": ["rust", "api", "backend"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search by tag
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=typescript")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let projects = body["items"].as_array().expect("Expected items array");

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["title"], "Frontend Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn search_projects_by_external_refs() {
    let app = test_app().await;

    // Create projects with external refs
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "GitHub Integration",
                        "external_refs": ["owner/repo#123", "owner/repo#456"]
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
                        "title": "Jira Integration",
                        "external_refs": ["PROJ-789"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search by external ref
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=owner%2Frepo%23123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let projects = body["items"].as_array().expect("Expected items array");

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["title"], "GitHub Integration");
}

#[tokio::test(flavor = "multi_thread")]
async fn search_projects_with_boolean_operators() {
    let app = test_app().await;

    // Create test projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Web API",
                        "description": "Backend service"
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
                        "title": "Rust CLI Tool",
                        "description": "Command line utility"
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
                        "title": "Python API",
                        "description": "Backend service"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search with AND operator
    let response = app
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
    let projects = body["items"].as_array().expect("Expected items array");

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["title"], "Rust Web API");
}

#[tokio::test(flavor = "multi_thread")]
async fn search_projects_with_phrase_query() {
    let app = test_app().await;

    // Create test projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Backend Service",
                        "description": "RESTful API service implementation"
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
                        "title": "API Documentation",
                        "description": "Service documentation for API"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search with exact phrase
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=%22API+service%22")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let projects = body["items"].as_array().expect("Expected items array");

    // Should only match exact phrase "API service"
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["title"], "Backend Service");
}

#[tokio::test(flavor = "multi_thread")]
async fn search_projects_combines_with_tag_filter() {
    let app = test_app().await;

    // Create test projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Backend",
                        "tags": ["backend", "production"]
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
                        "tags": ["frontend", "production"]
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
                        "title": "Rust CLI",
                        "tags": ["backend", "experimental"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search "rust" with tag filter "backend,production" (OR logic - matches ANY tag)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=rust&tags=backend,production")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let projects = body["items"].as_array().expect("Expected items array");

    // Should match all "Rust" projects with "backend" OR "production" tags (3 projects):
    // - "Rust Backend" (has backend + production)
    // - "Rust Frontend" (has production)
    // - "Rust CLI" (has backend)
    assert_eq!(projects.len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn search_projects_empty_query_returns_all() {
    let app = test_app().await;

    // Create a project
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
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

    // Search with empty query (should return all)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let projects = body["items"].as_array().expect("Expected items array");

    assert_eq!(projects.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn search_projects_no_results() {
    let app = test_app().await;

    // Create a project
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
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

    // Search for something that doesn't exist
    let response = app
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
    let projects = body["items"].as_array().expect("Expected items array");

    assert_eq!(projects.len(), 0);
    assert_eq!(body["total"], 0);
}
