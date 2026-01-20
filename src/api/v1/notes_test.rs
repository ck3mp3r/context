//! Integration tests for Note API endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::notifier::{ChangeNotifier, UpdateMessage};
use crate::api::{AppState, routes};
use crate::db::{Database, SqliteDatabase};

/// Create a test app with an in-memory database
async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        ChangeNotifier::new(),
    );
    routes::create_router(state, false)
}

/// Helper to create test app with access to notifier for broadcast testing
async fn test_app_with_notifier() -> (axum::Router, ChangeNotifier) {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let notifier = ChangeNotifier::new();
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
// Comprehensive List and Filtering Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_comprehensive() {
    let app = test_app().await;

    // Test 1: Initially empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());

    // Test 2: Create projects and repos for relationship testing
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

    // Test 3: Create notes with various combinations for filtering
    // Parent note with project A
    let parent_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Parent Rust Note",
                        "content": "Learning Rust programming",
                        "tags": ["rust", "programming"],
                        "project_ids": [&project_a_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let parent_id = json_body(parent_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Child note 1
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Child Note 1",
                        "content": "Subnote content",
                        "parent_id": &parent_id,
                        "idx": 10
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Child note 2
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Child Note 2",
                        "content": "Another subnote",
                        "parent_id": &parent_id,
                        "idx": 20
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Note with project B
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Apple Python Note",
                        "content": "Python scripting guide",
                        "tags": ["python", "web"],
                        "project_ids": [&project_b_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Note with both projects
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Zebra Shared Note",
                        "content": "Content for both projects",
                        "tags": ["programming"],
                        "project_ids": [&project_a_id, &project_b_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test 4a: Filter by project_id (Project A should return 2: parent + shared)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes?project_id={}", project_a_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2, "Project A should have 2 notes");

    // Test 4b: Filter by project_id (Project B should return 2: python note + shared)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes?project_id={}", project_b_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2, "Project B should have 2 notes");

    // Test 4c: Filter by nonexistent project
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?project_id=nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 0);

    // Test 5a: Filter by tags
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?tags=rust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Parent Rust Note");

    // Test 5b: Filter by multiple tags (comma-separated)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?tags=rust,programming")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2, "Should match notes with ANY of the tags");

    // Test 6a: Filter by parent_id (should return children in idx order)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes?parent_id={}", parent_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
    assert_eq!(body["items"][0]["title"], "Child Note 1");
    assert_eq!(body["items"][0]["idx"], 10);
    assert_eq!(body["items"][1]["title"], "Child Note 2");
    assert_eq!(body["items"][1]["idx"], 20);

    // Test 7a: Filter by type=note (only parent notes)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?type=note")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 3, "Should return 3 parent notes");
    for item in body["items"].as_array().unwrap() {
        assert!(item["parent_id"].is_null());
    }

    // Test 7b: Filter by type=subnote (only child notes)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?type=subnote")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2, "Should return 2 subnotes");
    for item in body["items"].as_array().unwrap() {
        assert!(!item["parent_id"].is_null());
    }

    // Test 7c: Type omitted returns all
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 5, "Should return all notes");

    // Test 8: Ordering (asc by title)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?sort=title&order=asc&type=note")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["items"][0]["title"], "Apple Python Note");
    assert_eq!(body["items"][1]["title"], "Parent Rust Note");
    assert_eq!(body["items"][2]["title"], "Zebra Shared Note");

    // Test 9: Ordering (desc by title)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?sort=title&order=desc&type=note")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["items"][0]["title"], "Zebra Shared Note");
    assert_eq!(body["items"][1]["title"], "Parent Rust Note");
    assert_eq!(body["items"][2]["title"], "Apple Python Note");

    // Test 10a: Pagination
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?limit=2&offset=0&type=note")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    assert_eq!(body["total"], 3);
    assert_eq!(body["limit"], 2);
    assert_eq!(body["offset"], 0);

    // Test 10b: Second page
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?limit=2&offset=2&type=note")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["offset"], 2);

    // Test 11: Combined filters (project + search)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes?project_id={}&q=Rust", project_a_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Parent Rust Note");

    // Test 12: Empty search query returns empty results
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 0, "Empty search should return 0 results");
    assert!(body["items"].as_array().unwrap().is_empty());
}

// =============================================================================
// Comprehensive CRUD Operations and Relationship Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn crud_operations() {
    let app = test_app().await;

    // Create project and repo for relationship testing
    let project_response = app
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
    let project_id = json_body(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let repo_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"remote": "github:user/repo"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let repo_id = json_body(repo_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Test 1: CREATE with full data
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Full Note",
                        "content": "Complete content",
                        "tags": ["test", "comprehensive"],
                        "project_ids": [&project_id],
                        "repo_ids": [&repo_id],
                        "idx": 42
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let note_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["title"], "Full Note");
    assert_eq!(created["content"], "Complete content");
    assert_eq!(created["idx"], 42);
    assert_eq!(created["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(created["repo_ids"].as_array().unwrap().len(), 1);
    assert_eq!(created["tags"].as_array().unwrap().len(), 2);

    // Test 2: GET by ID
    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes/{}", note_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let retrieved = json_body(get_response).await;
    assert_eq!(retrieved["id"], note_id);
    assert_eq!(retrieved["title"], "Full Note");

    // Test 3: GET nonexistent returns 404
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 4: PATCH all fields (covers all merge_into branches)
    // Create a parent note first for parent_id testing
    let parent_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Parent Note",
                        "content": "Parent content"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let parent_id = json_body(parent_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated Title",
                        "content": "Updated content",
                        "tags": ["updated"],
                        "parent_id": &parent_id,
                        "idx": 99,
                        "project_ids": [&project_id],
                        "repo_ids": [&repo_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_response.status(), StatusCode::OK);
    let patched = json_body(patch_response).await;
    assert_eq!(patched["title"], "Updated Title");
    assert_eq!(patched["content"], "Updated content");
    assert_eq!(patched["idx"], 99);
    assert_eq!(patched["parent_id"], parent_id);
    assert_eq!(patched["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(patched["repo_ids"].as_array().unwrap().len(), 1);

    // Test 5: PATCH nonexistent returns 404
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/notes/nonexistent")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Updated"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 6: PUT full replacement
    let put_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Completely Replaced",
                        "content": "New content",
                        "idx": 100
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);
    let replaced = json_body(put_response).await;
    assert_eq!(replaced["title"], "Completely Replaced");
    assert_eq!(replaced["content"], "New content");
    assert_eq!(replaced["idx"], 100);

    // Test 6b: PUT nonexistent returns 404
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/notes/nonexistent")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated",
                        "content": "Content"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 7: DELETE
    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/notes/{}", note_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // Test 8: GET deleted returns 404
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes/{}", note_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 9: DELETE nonexistent returns 404
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/notes/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Comprehensive FTS5 Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_comprehensive() {
    let app = test_app().await;

    // Create project for combined search+filter testing
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Search Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create notes with various searchable content
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Web Development",
                        "content": "Learning Axum framework",
                        "tags": ["rust", "web"],
                        "project_ids": [&project_id]
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
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Python Guide",
                        "content": "Django web framework",
                        "tags": ["python", "web"],
                        "project_ids": [&project_id]
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
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust CLI Tools",
                        "content": "Command-line applications",
                        "tags": ["rust", "cli"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test 1: Search by title
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=Rust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);

    // Test 2: Search by content
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=framework")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);

    // Test 3: Search by tags
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=python")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Python Guide");

    // Test 4: Boolean AND operator
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=rust+AND+web")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Rust Web Development");

    // Test 5: Boolean OR operator
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=python+OR+cli")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);

    // Test 6: Combined search + project filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes?q=web&project_id={}", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2, "Should find 2 web notes in project");

    // Test 7: Combined search + tag filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=rust&tags=web")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);

    // Test 8: No match returns empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());
}

// =============================================================================
// WebSocket Broadcast Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn websocket_broadcasts() {
    let (app, notifier) = test_app_with_notifier().await;

    // Test 1: CREATE broadcasts
    let mut rx = notifier.subscribe();
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"title": "Broadcast Test", "content": "Content"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let note = json_body(create_response).await;
    let note_id = note["id"].as_str().unwrap().to_string();
    let msg = rx.try_recv().expect("Should receive create broadcast");
    assert_eq!(
        msg,
        UpdateMessage::NoteCreated {
            note_id: note_id.clone()
        }
    );

    // Test 2: UPDATE broadcasts
    let mut rx = notifier.subscribe();
    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"title": "Updated", "content": "Updated content"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let msg = rx.try_recv().expect("Should receive update broadcast");
    assert_eq!(
        msg,
        UpdateMessage::NoteUpdated {
            note_id: note_id.clone()
        }
    );

    // Test 3: DELETE broadcasts
    let mut rx = notifier.subscribe();
    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/notes/{}", note_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);
    let msg = rx.try_recv().expect("Should receive delete broadcast");
    assert_eq!(msg, UpdateMessage::NoteDeleted { note_id });
}
