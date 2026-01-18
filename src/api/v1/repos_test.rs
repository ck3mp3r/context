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
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
    );
    routes::create_router(state, false)
}

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

/// Helper to parse JSON response body
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// Comprehensive List/Filter Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_repos_comprehensive() {
    let app = test_app().await;

    // Test 1: Initially empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/repos")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());

    // Test 2: Create projects for filtering
    let project_a = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Project A"})).unwrap(),
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
                    serde_json::to_vec(&json!({"title": "Project B"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_b_id = json_body(project_b).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Test 3: Create repos with different project associations
    for (remote, projects) in [
        ("github:org/repo-a1", vec![&project_a_id]),
        ("github:org/repo-a2", vec![&project_a_id]),
        ("github:org/repo-b1", vec![&project_b_id]),
        ("github:org/repo-shared", vec![&project_a_id, &project_b_id]),
    ] {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/repos")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "remote": remote,
                            "project_ids": projects
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test 4: Filter by project A (should return 3: 2 exclusive + 1 shared)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/repos?project_id={}", project_a_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 3);

    // Test 5: Filter by project B (should return 2: 1 exclusive + 1 shared)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/repos?project_id={}", project_b_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);

    // Test 6: Filter by nonexistent project (should return 0)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/repos?project_id=nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 0);
}

// =============================================================================
// Comprehensive CRUD Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn crud_operations() {
    let app = test_app().await;

    // Create a project for relationship testing
    let project = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Test Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project).await["id"].as_str().unwrap().to_string();

    // Test 1: CREATE repo with full data
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
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
    let repo_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["remote"], "https://github.com/original/repo");
    assert_eq!(created["path"], "/original/path");
    assert_eq!(created["tags"].as_array().unwrap().len(), 2);

    // Test 2: GET repo by ID
    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/repos/{}", repo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let got = json_body(get_response).await;
    assert_eq!(got["id"], repo_id);

    // Test 3: GET nonexistent repo (404)
    let get_404 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/repos/notfound")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_404.status(), StatusCode::NOT_FOUND);

    // Test 4: PATCH partial update (remote only)
    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/repos/{}", repo_id))
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
    assert_eq!(patched["remote"], "https://github.com/updated/repo");
    assert_eq!(patched["path"], "/original/path"); // Preserved
    assert_eq!(patched["tags"].as_array().unwrap().len(), 2); // Preserved

    // Test 5: PATCH to add project relationship
    let patch_project = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/repos/{}", repo_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "project_ids": [&project_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_project.status(), StatusCode::OK);
    let with_project = json_body(patch_project).await;
    assert_eq!(with_project["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(with_project["project_ids"][0], project_id);

    // Test 6: PATCH nonexistent repo (404)
    let patch_404 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/repos/notfound")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({"remote": "new"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_404.status(), StatusCode::NOT_FOUND);

    // Test 7: PUT full update
    let put_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/repos/{}", repo_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "remote": "https://github.com/replaced/repo",
                        "path": "/new/path",
                        "tags": ["newtag"],
                        "project_ids": []
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);
    let updated = json_body(put_response).await;
    assert_eq!(updated["remote"], "https://github.com/replaced/repo");
    assert_eq!(updated["path"], "/new/path");

    // Test 8: DELETE repo
    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/repos/{}", repo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // Test 9: GET deleted repo (404)
    let get_deleted = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/repos/{}", repo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_deleted.status(), StatusCode::NOT_FOUND);

    // Test 10: DELETE nonexistent repo (404)
    let delete_404 = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/repos/notfound")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_404.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// WebSocket Broadcast Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn websocket_broadcasts() {
    let (app, notifier) = test_app_with_notifier().await;

    // Test 1: Create broadcasts RepoCreated
    let mut subscriber = notifier.subscribe();
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "remote": "github:user/test-repo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let repo_id = created["id"].as_str().unwrap().to_string();

    let msg = subscriber
        .recv()
        .await
        .expect("Should receive create broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::RepoCreated { repo_id: id } => {
            assert_eq!(id, repo_id);
        }
        _ => panic!("Expected RepoCreated, got {:?}", msg),
    }

    // Test 2: Update broadcasts RepoUpdated (subscribe after create to avoid create notification)
    let mut subscriber = notifier.subscribe();
    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/repos/{}", repo_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "remote": "github:user/updated-repo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);

    let msg = subscriber
        .recv()
        .await
        .expect("Should receive update broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::RepoUpdated { repo_id: id } => {
            assert_eq!(id, repo_id);
        }
        _ => panic!("Expected RepoUpdated, got {:?}", msg),
    }

    // Test 3: Delete broadcasts RepoDeleted
    let mut subscriber = notifier.subscribe();
    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/repos/{}", repo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let msg = subscriber
        .recv()
        .await
        .expect("Should receive delete broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::RepoDeleted { repo_id: id } => {
            assert_eq!(id, repo_id);
        }
        _ => panic!("Expected RepoDeleted, got {:?}", msg),
    }
}

// =============================================================================
// FTS5 Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_comprehensive() {
    let app = test_app().await;

    // Create test repos for comprehensive search testing
    for (remote, path, tags) in [
        (
            "https://github.com/rust-lang/rust.git",
            "/home/user/rust",
            vec!["language", "system"],
        ),
        (
            "https://github.com/python/cpython.git",
            "/home/user/python",
            vec!["language", "scripting"],
        ),
        (
            "https://github.com/company/api-backend.git",
            "/srv/projects/backend-api",
            vec!["api", "production"],
        ),
        (
            "https://github.com/company/web-frontend.git",
            "/srv/projects/frontend-app",
            vec!["frontend", "production"],
        ),
    ] {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/repos")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "remote": remote,
                            "path": path,
                            "tags": tags
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test 1: Search by remote URL
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/repos?q=rust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert!(
        body["items"][0]["remote"]
            .as_str()
            .unwrap()
            .contains("rust-lang")
    );

    // Test 2: Search by path
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/repos?q=backend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert!(body["total"].as_u64().unwrap() >= 1);
    let items = body["items"].as_array().unwrap();
    let has_backend = items.iter().any(|item| {
        let remote = item["remote"].as_str().unwrap_or("");
        let path = item["path"].as_str().unwrap_or("");
        remote.contains("backend") || path.contains("backend")
    });
    assert!(has_backend);

    // Test 3: Boolean AND operator
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/repos?q=api%20AND%20production")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert!(
        body["items"][0]["remote"]
            .as_str()
            .unwrap()
            .contains("api-backend")
    );

    // Test 4: Empty query returns all
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/repos?q=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 4); // All 4 repos we created
}
