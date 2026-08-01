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

use context_core::generate_entity_id;
use context_core::{Database, HasProjects, Project, ProjectRepository};
use context_db::SqliteDatabase;
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

/// Helper to create test app with access to notifier for broadcast testing
async fn test_app_with_notifier() -> (axum::Router, context_server::api::notifier::ChangeNotifier) {
    let db = common::setup_db().await;
    let notifier = context_server::api::notifier::ChangeNotifier::new();
    let temp_dir = TempDir::new().unwrap();
    let state = AppState::new(
        db,
        SyncManager::new(MockGitOps::new()),
        notifier.clone(),
        temp_dir.path().join("skills"),
    );
    (routes::create_router(state, false), notifier)
}

/// Helper to parse JSON response body
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// Comprehensive List and Relationship Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_and_relationships_comprehensive() {
    let app = test_app().await;

    // Test 1: Initially empty
    let response = app
        .clone()
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
    assert_eq!(body["total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());

    // Test 2: Create projects with different tags for filtering tests
    let project_a = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project A",
                        "tags": ["backend", "rust"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_a_id = json_body(project_a).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let project_b = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project B",
                        "tags": ["frontend", "react"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    json_body(project_b).await;

    // Test 2a: List with tag filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?tags=backend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1, "Should find 1 project with 'backend' tag");

    // Test 2b: List with multiple tag filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?tags=backend,frontend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2, "Should find 2 projects with either tag");

    // Test 3: Get project by ID
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/api/v1/projects/{}", project_a_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["title"], "Project A");
    assert_eq!(body["id"], project_a_id);

    // Test 4: Get non-existent project
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 5: Update project
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&format!("/api/v1/projects/{}", project_a_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated Project A",
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
    assert_eq!(body["title"], "Updated Project A");
    assert_eq!(body["description"], "Updated description");

    // Test 6: Patch project (partial update)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/api/v1/projects/{}", project_a_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "description": "Patched description"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["title"], "Updated Project A"); // Title unchanged
    assert_eq!(body["description"], "Patched description"); // Description updated

    // Test 7: Delete project
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/api/v1/projects/{}", project_a_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Test 8: Verify deletion
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/api/v1/projects/{}", project_a_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 9: List after deletion
    let response = app
        .clone()
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
    assert_eq!(body["total"], 1, "Should have 1 project after deletion");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_validation() {
    let app = test_app().await;

    // Test: Empty title should fail
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": ""
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test: Missing title should fail
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "description": "no title"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_external_refs() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project with refs",
                        "external_refs": ["owner/repo#123", "PROJ-456"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    assert_eq!(body["title"], "Project with refs");
    assert_eq!(body["external_refs"], json!(["owner/repo#123", "PROJ-456"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_repo_ids() {
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
                    serde_json::to_vec(&json!({
                        "title": "Project with repos"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(project_response.status(), StatusCode::CREATED);
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
    assert_eq!(repo_response.status(), StatusCode::CREATED);
    let repo_id = json_body(repo_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Get project to verify repo_ids
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/api/v1/projects/{}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["repo_ids"], json!([repo_id]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_project_preserves_unchanged_fields() {
    let app = test_app().await;

    // Create project
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
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
    let project_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Update with all fields (PUT replaces everything)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated Title",
                        "description": "Original description",
                        "tags": ["tag1", "tag2"]
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
    assert_eq!(body["description"], "Original description");
    assert_eq!(body["tags"], json!(["tag1", "tag2"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_project_clears_field_with_null() {
    let app = test_app().await;

    // Create project with description
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project to patch",
                        "description": "Will be cleared"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Patch with null description
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "description": null
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["description"], Value::Null);
}

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
async fn test_create_project_duplicate_title() {
    let app = test_app().await;

    // Create first project
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Unique Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Create second project with same title (should succeed - titles don't need to be unique)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Unique Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_nonexistent_project() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/projects/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_nonexistent_project() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/projects/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Nope"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_nonexistent_project() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/projects/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Nope"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_long_title() {
    let app = test_app().await;

    let long_title = "A".repeat(500);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": long_title})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_special_chars() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project with <script>alert('xss')</script> & special chars: ñoño 你好 🎉"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    assert!(body["title"].as_str().unwrap().contains("special chars"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_all_fields() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Full Project",
                        "description": "A project with all fields",
                        "tags": ["rust", "api", "test"],
                        "external_refs": ["owner/repo#1", "PROJ-789"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    assert_eq!(body["title"], "Full Project");
    assert_eq!(body["description"], "A project with all fields");
    assert_eq!(body["tags"], json!(["rust", "api", "test"]));
    assert_eq!(body["external_refs"], json!(["owner/repo#1", "PROJ-789"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_without_optional_fields() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Minimal Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    assert_eq!(body["title"], "Minimal Project");
    assert_eq!(body["description"], Value::Null);
    assert_eq!(body["tags"], json!([]));
    assert_eq!(body["external_refs"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_empty_tags() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Empty Tags",
                        "tags": []
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    assert_eq!(body["tags"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_empty_external_refs() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Empty Refs",
                        "external_refs": []
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    assert_eq!(body["external_refs"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_project_clears_all_fields() {
    let app = test_app().await;

    // Create project with all fields
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Full Project",
                        "description": "Has description",
                        "tags": ["tag1"],
                        "external_refs": ["ref1"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Update with only title (should clear optional fields)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated Title"
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
    assert_eq!(body["description"], Value::Null);
    assert_eq!(body["tags"], json!([]));
    assert_eq!(body["external_refs"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_empty_tags_filter() {
    let app = test_app().await;

    // Create a project with tags
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Tagged Project",
                        "tags": ["backend"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter with empty tag should return all
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?tags=")
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
async fn test_list_projects_nonexistent_tag() {
    let app = test_app().await;

    // Create a project
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Test"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter with non-existent tag
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?tags=nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 0);
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
                .uri(&format!("/api/v1/projects?repo_id={}", repo_id))
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
                .uri(&format!("/api/v1/projects?note_id={}", note_id))
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
                .uri(&format!("/api/v1/projects?task_list_id={}", list_id))
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
                .uri(&format!("/api/v1/task-lists/{}/tasks", list_id))
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
                .uri(&format!("/api/v1/projects?task_id={}", task_id))
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
                .uri(&format!("/api/v1/projects?skill_id={}", skill_id))
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
                .uri(&format!(
                    "/api/v1/projects?tags=backend&repo_id={}",
                    repo_id
                ))
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
                .uri(&format!("/api/v1/projects?q={}", long_query))
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
