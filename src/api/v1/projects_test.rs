//! Integration tests for Project API endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::{AppState, routes};
use crate::db::utils::generate_entity_id;
use crate::db::{Database, Project, ProjectRepository, SqliteDatabase};

/// Create a test app with an in-memory database
async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
        std::path::PathBuf::from("/tmp/skills"),
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
        std::path::PathBuf::from("/tmp/skills"),
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
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Project A");

    // Test 2b: List with sorting
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
    assert_eq!(body["items"][0]["title"], "Project B"); // B before A in desc order

    // Test 2c: List with pagination
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?limit=1&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["limit"], 1);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    let project_id = project_a_id;

    // Test 3: Update repo with project relationships (cross-entity relationship)
    let repo = app
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
    let repo_id = json_body(repo).await["id"].as_str().unwrap().to_string();

    let repo_update = app
        .clone()
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
                        "project_ids": [&project_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(repo_update.status(), StatusCode::OK);
    let repo_body = json_body(repo_update).await;
    assert_eq!(repo_body["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(repo_body["project_ids"][0], project_id);

    // Test 4: Update note with project relationships (cross-entity relationship)
    let note = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test Note",
                        "content": "Initial content"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let note_id = json_body(note).await["id"].as_str().unwrap().to_string();

    let note_update = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test Note",
                        "content": "Updated content",
                        "tags": ["note-tag"],
                        "project_ids": [&project_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(note_update.status(), StatusCode::OK);
    let note_body = json_body(note_update).await;
    assert_eq!(note_body["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(note_body["project_ids"][0], project_id);
}

// =============================================================================
// Comprehensive CRUD and PATCH Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn crud_and_patch_operations() {
    // Seed project with old timestamp using DB layer
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    let old_timestamp = "2020-01-01 00:00:00";
    let project = Project {
        id: generate_entity_id(),
        title: "Original Project".to_string(),
        description: Some("Original description".to_string()),
        tags: vec!["tag1".to_string(), "tag2".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some(old_timestamp.to_string()),
        updated_at: Some(old_timestamp.to_string()),
    };
    let created = db.projects().create(&project).await.unwrap();
    let project_id = created.id.clone();
    assert_eq!(created.updated_at.as_ref().unwrap(), old_timestamp);

    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
        std::path::PathBuf::from("/tmp/skills"),
    );
    let app = routes::create_router(state, false);

    // Test 2: GET project by ID
    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects/{}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let got = json_body(get_response).await;
    assert_eq!(got["id"], project_id);

    // Test 3: GET nonexistent project (404)
    let get_404 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects/notfound")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_404.status(), StatusCode::NOT_FOUND);

    // Test 4: PATCH partial update (title only) - verify timestamp updates
    let patch_title = app
        .clone()
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
    assert_eq!(patch_title.status(), StatusCode::OK);
    let patched = json_body(patch_title).await;
    assert_eq!(patched["title"], "Updated Title");
    assert_eq!(patched["description"], "Original description"); // Preserved

    // Verify updated_at changed from old seeded timestamp
    let patched_timestamp = patched["updated_at"].as_str().unwrap();
    assert_ne!(
        old_timestamp, patched_timestamp,
        "updated_at should change from '{}' after PATCH",
        old_timestamp
    );

    // Test 5: PATCH with field omission preserves existing values
    let patch_preserve = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "description": "New description"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_preserve.status(), StatusCode::OK);
    let preserved = json_body(patch_preserve).await;
    assert_eq!(preserved["title"], "Updated Title"); // Still preserved from previous update
    assert_eq!(preserved["description"], "New description");

    // Test 6: PATCH with empty body preserves all fields
    let patch_empty = app
        .clone()
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
    assert_eq!(patch_empty.status(), StatusCode::OK);
    let unchanged = json_body(patch_empty).await;
    assert_eq!(unchanged["title"], "Updated Title");
    assert_eq!(unchanged["description"], "New description");

    // Test 7: PATCH nonexistent project (404)
    let patch_404 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/projects/notfound")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({"title": "new"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_404.status(), StatusCode::NOT_FOUND);

    // Test 8: PUT full update
    let put_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Completely Replaced",
                        "description": "All new",
                        "tags": ["newtag"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);
    let updated = json_body(put_response).await;
    assert_eq!(updated["title"], "Completely Replaced");
    assert_eq!(updated["description"], "All new");

    // Test 9: DELETE project
    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/projects/{}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // Test 10: GET deleted project (404)
    let get_deleted = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/projects/{}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_deleted.status(), StatusCode::NOT_FOUND);

    // Test 11: DELETE nonexistent project (404)
    let delete_404 = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/projects/notfound")
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

    // Test 1: Create broadcasts ProjectCreated
    let mut subscriber = notifier.subscribe();
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Broadcast Test"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let project_id = created["id"].as_str().unwrap().to_string();

    let msg = subscriber
        .recv()
        .await
        .expect("Should receive create broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::ProjectCreated { project_id: id } => {
            assert_eq!(id, project_id);
        }
        _ => panic!("Expected ProjectCreated, got {:?}", msg),
    }

    // Test 2: Update broadcasts ProjectUpdated
    let mut subscriber = notifier.subscribe();
    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Updated Broadcast"
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
        crate::api::notifier::UpdateMessage::ProjectUpdated { project_id: id } => {
            assert_eq!(id, project_id);
        }
        _ => panic!("Expected ProjectUpdated, got {:?}", msg),
    }

    // Test 3: Delete broadcasts ProjectDeleted
    let mut subscriber = notifier.subscribe();
    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/projects/{}", project_id))
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
        crate::api::notifier::UpdateMessage::ProjectDeleted { project_id: id } => {
            assert_eq!(id, project_id);
        }
        _ => panic!("Expected ProjectDeleted, got {:?}", msg),
    }
}

// =============================================================================
// External References Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn external_refs_operations() {
    let app = test_app().await;

    // Test 1: Create project with external refs
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "External Ref Project",
                        "external_refs": ["github:org/repo#123", "JIRA-456"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let project_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["external_refs"].as_array().unwrap().len(), 2);
    assert_eq!(created["external_refs"][0], "github:org/repo#123");
    assert_eq!(created["external_refs"][1], "JIRA-456");

    // Test 2: Update external refs
    let update_response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/projects/{}", project_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "External Ref Project",
                        "external_refs": ["github:org/repo#456", "JIRA-789", "LINEAR-123"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated = json_body(update_response).await;
    assert_eq!(updated["external_refs"].as_array().unwrap().len(), 3);
    assert_eq!(updated["external_refs"][0], "github:org/repo#456");
    assert_eq!(updated["external_refs"][2], "LINEAR-123");
}

// =============================================================================
// FTS5 Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_comprehensive() {
    let app = test_app().await;

    // Create test projects for comprehensive search testing
    for (title, description, tags, external_refs) in [
        (
            "Rust Backend API",
            "High-performance async backend",
            vec!["rust", "backend", "api"],
            vec!["github:company/rust-api#1"],
        ),
        (
            "Python Data Pipeline",
            "ETL pipeline for analytics",
            vec!["python", "data", "etl"],
            vec!["github:company/pipeline#2"],
        ),
        (
            "Frontend Dashboard",
            "React-based monitoring dashboard",
            vec!["react", "frontend", "monitoring"],
            vec!["JIRA-101"],
        ),
        (
            "Mobile App",
            "Cross-platform mobile application",
            vec!["mobile", "react-native"],
            vec!["LINEAR-202"],
        ),
    ] {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "title": title,
                            "description": description,
                            "tags": tags,
                            "external_refs": external_refs
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test 1: Search by title
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
    assert!(body["items"][0]["title"].as_str().unwrap().contains("Rust"));

    // Test 2: Search by description
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=analytics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert!(
        body["items"][0]["description"]
            .as_str()
            .unwrap()
            .contains("analytics")
    );

    // Test 3: Search by tags
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=frontend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    let tags = body["items"][0]["tags"].as_array().unwrap();
    assert!(tags.iter().any(|t| t.as_str().unwrap() == "frontend"));

    // Test 4: Search by external refs
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=JIRA")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert!(
        body["items"][0]["external_refs"][0]
            .as_str()
            .unwrap()
            .contains("JIRA")
    );

    // Test 5: Boolean AND operator
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=react%20AND%20monitoring")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert!(
        body["items"][0]["title"]
            .as_str()
            .unwrap()
            .contains("Dashboard")
    );

    // Test 6: Phrase query
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=%22mobile%20application%22")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert!(
        body["items"][0]["description"]
            .as_str()
            .unwrap()
            .contains("mobile application")
    );

    // Test 7: Combine search with tag filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=backend&tags=rust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert!(body["items"][0]["title"].as_str().unwrap().contains("Rust"));

    // Test 8: Empty query returns all
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
    let body = json_body(response).await;
    assert_eq!(body["total"], 4); // All 4 projects we created

    // Test 9: No results
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects?q=nonexistent_term_xyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 0);
}
