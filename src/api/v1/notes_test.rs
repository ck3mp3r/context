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

async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        ChangeNotifier::new(),
    );
    routes::create_router(state, false)
}

async fn test_app_with_notifier() -> (axum::Router, ChangeNotifier) {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");
    let notifier = ChangeNotifier::new();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        notifier.clone(),
    );
    (routes::create_router(state, false), notifier)
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// GET /v1/notes - List Notes (with optional search & pagination)
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_initially_empty() {
    let app = test_app().await;

    let response = app
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
    assert!(body["items"].as_array().unwrap().is_empty());
    assert_eq!(body["total"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_with_search_query() {
    let app = test_app().await;

    // Create notes with different content
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Programming",
                        "content": "Rust is a systems programming language"
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
                        "title": "Python Scripting",
                        "content": "Python is great for scripting"
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
                .uri("/api/v1/notes?q=rust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let notes = body["items"].as_array().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["title"], "Rust Programming");
    assert_eq!(body["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_with_pagination() {
    let app = test_app().await;

    // Create 5 notes
    for i in 1..=5 {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/notes")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "title": format!("Note {}", i),
                            "content": format!("Content {}", i)
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Get first page (limit 2)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?limit=2&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    assert_eq!(body["total"], 5);
    assert_eq!(body["limit"], 2);
    assert_eq!(body["offset"], 0);

    // Get second page
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?limit=2&offset=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    assert_eq!(body["offset"], 2);

    // Get last page (partial)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?limit=2&offset=4")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_search_no_match_returns_empty() {
    let app = test_app().await;

    // Create a note
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Some Note",
                        "content": "Some content"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search for non-existent term
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert!(body["items"].as_array().unwrap().is_empty());
    assert_eq!(body["total"], 0);
}

// =============================================================================
// GET /v1/notes?project_id=X - Filter by Project
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_filtered_by_project_id() {
    let app = test_app().await;

    // Create two projects
    let project_a_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
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
                .uri("/api/v1/projects")
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

    // Create notes: 2 for project A, 1 for project B, 1 for both
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project A Note 1",
                        "content": "Content for A",
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
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project A Note 2",
                        "content": "More content for A",
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
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project B Note",
                        "content": "Content for B",
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
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Shared Note",
                        "content": "Content for both",
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
                .uri(format!("/api/v1/notes?project_id={}", project_a_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    // Should return 3 notes: 2 exclusive to A + 1 shared
    assert_eq!(body["total"], 3);
    assert_eq!(body["items"].as_array().unwrap().len(), 3);

    // Filter by project_id for Project B
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes?project_id={}", project_b_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    // Should return 2 notes: 1 exclusive to B + 1 shared
    assert_eq!(body["total"], 2);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_filtered_by_project_id_and_search() {
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
    let project_id = project_body["id"].as_str().unwrap();

    // Create notes with different content
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Guide",
                        "content": "Learning Rust programming",
                        "project_ids": [project_id]
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
                        "content": "Learning Python programming",
                        "project_ids": [project_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Create a note in a different (non-existent) project
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Mastery",
                        "content": "Advanced Rust techniques",
                        "project_ids": []
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter by project_id AND search for "Rust"
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes?project_id={}&q=Rust", project_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    // Should return only 1 note: "Rust Guide" from the project
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["title"], "Rust Guide");
}

// =============================================================================
// POST /v1/notes - Create Note
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_note_returns_created() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "My Note",
                        "content": "This is the note content",
                        "tags": ["rust", "api"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["title"], "My Note");
    assert_eq!(body["content"], "This is the note content");
    assert_eq!(body["tags"], json!(["rust", "api"]));
    assert!(body["id"].as_str().unwrap().len() == 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn create_note_minimal() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Quick Note",
                        "content": "Some content"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["title"], "Quick Note");
    assert_eq!(body["content"], "Some content");
    assert_eq!(body["tags"], json!([]));
}

// =============================================================================
// GET /v1/notes/{id} - Get Note
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_note_returns_note() {
    let app = test_app().await;

    // Create first
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
                        "content": "Test content"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let note_id = body["id"].as_str().unwrap();

    // Get it
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes/{}", note_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["id"], note_id);
    assert_eq!(body["title"], "Test Note");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_note_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// PUT /v1/notes/{id} - Update Note
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn update_note_returns_updated() {
    let app = test_app().await;

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Original",
                        "content": "Original content"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let note_id = body["id"].as_str().unwrap();

    // Update
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated",
                        "content": "Updated content",
                        "tags": ["updated"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["title"], "Updated");
    assert_eq!(body["content"], "Updated content");
    assert_eq!(body["tags"], json!(["updated"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn update_note_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/notes/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Wont Work",
                        "content": "content"
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
// DELETE /v1/notes/{id} - Delete Note
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn delete_note_returns_no_content() {
    let app = test_app().await;

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "To Delete",
                        "content": "content"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let note_id = body["id"].as_str().unwrap();

    // Delete
    let response = app
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

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify gone
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes/{}", note_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_note_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/notes/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Tag Filtering
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_with_tag_filter() {
    let app = test_app().await;

    // Create notes with different tags
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Note",
                        "content": "About Rust",
                        "tags": ["rust", "programming"]
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
                        "title": "Python Note",
                        "content": "About Python",
                        "tags": ["python", "programming"]
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
                        "title": "Cooking Note",
                        "content": "About cooking",
                        "tags": ["cooking"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter by "rust" tag
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
    assert_eq!(body["items"][0]["title"], "Rust Note");

    // Filter by "programming" tag (should match 2)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?tags=programming")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
}

// =============================================================================
// Ordering
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_with_ordering() {
    let app = test_app().await;

    // Create notes with different titles
    for title in ["Zebra", "Apple", "Mango"] {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/notes")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "title": title,
                            "content": "content"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Sort by title ascending
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?sort=title&order=asc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items[0]["title"], "Apple");
    assert_eq!(items[1]["title"], "Mango");
    assert_eq!(items[2]["title"], "Zebra");

    // Sort by title descending
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?sort=title&order=desc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items[0]["title"], "Zebra");
    assert_eq!(items[1]["title"], "Mango");
    assert_eq!(items[2]["title"], "Apple");
}

// =============================================================================
// PATCH /v1/notes/{id} - Partial Update Note
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn patch_note_partial_title_update() {
    let app = test_app().await;

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
                        "title": "Original Title",
                        "content": "Original Content",
                        "tags": ["original", "tag"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let id = body["id"].as_str().unwrap();

    // Partially update only the title
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/notes/{}", id))
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

    // Title should be updated
    assert_eq!(body["title"], "Updated Title");

    // Other fields should remain unchanged
    assert_eq!(body["content"], "Original Content");
    assert_eq!(body["tags"], json!(["original", "tag"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_note_omit_field_preserves_it() {
    let app = test_app().await;

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
                        "content": "Test Content",
                        "tags": ["test"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let id = body["id"].as_str().unwrap();

    // PATCH with empty body should preserve all fields
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/notes/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({})).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;

    // All fields should remain unchanged
    assert_eq!(body["title"], "Test Note");
    assert_eq!(body["content"], "Test Content");
    assert_eq!(body["tags"], json!(["test"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_note_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/notes/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Won't Work"
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
// PATCH /v1/notes/{id} - Relationship Relinking
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn patch_note_link_to_project_and_repo() {
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

    // Create a repo
    let repo_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
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
    let repo_id = repo_body["id"].as_str().unwrap().to_string();

    // Create a note without relationships
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
                        "content": "Test content"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let note_body = json_body(note_response).await;
    let note_id = note_body["id"].as_str().unwrap();

    // Verify no relationships initially
    assert!(note_body["project_ids"].as_array().unwrap().is_empty());
    assert!(note_body["repo_ids"].as_array().unwrap().is_empty());

    // PATCH to link to both project and repo
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "project_ids": [project_id],
                        "repo_ids": [repo_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;

    // Verify relationships were added
    assert_eq!(body["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["project_ids"][0], project_id);
    assert_eq!(body["repo_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["repo_ids"][0], repo_id);
}

// =============================================================================
// FTS5 Tag Search Integration Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_notes_by_tags_via_api() {
    let app = test_app().await;

    // Create notes with specific tags
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Programming",
                        "content": "Learning async/await",
                        "tags": ["rust", "programming"]
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
                        "content": "Flask tutorial",
                        "tags": ["python", "web"]
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
                        "title": "JavaScript Basics",
                        "content": "ES6 features",
                        "tags": ["javascript", "programming"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search for "rust" - should find the note with "rust" tag
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=rust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["items"].as_array().unwrap().len(),
        1,
        "Should find note with 'rust' tag"
    );
    assert_eq!(body["items"][0]["title"], "Rust Programming");

    // Search for "programming" - should find 2 notes with "programming" tag
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=programming")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["items"].as_array().unwrap().len(),
        2,
        "Should find both notes with 'programming' tag"
    );

    // Search for "python" - should find note with "python" tag
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=python")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["items"].as_array().unwrap().len(),
        1,
        "Should find note with 'python' tag"
    );
    assert_eq!(body["items"][0]["title"], "Python Guide");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_boolean_operators_with_tags_via_api() {
    let app = test_app().await;

    // Create notes with different tag combinations
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Web Development",
                        "content": "Axum framework guide",
                        "tags": ["rust", "web"]
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
                        "title": "Python Web Development",
                        "content": "Django tutorial",
                        "tags": ["python", "web"]
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
                        "content": "Command-line parsing",
                        "tags": ["rust", "cli"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // FTS5 AND operator: search for notes with both "rust" AND "web"
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

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["items"].as_array().unwrap().len(),
        1,
        "Should find only note with both 'rust' AND 'web' tags"
    );
    assert_eq!(body["items"][0]["title"], "Rust Web Development");

    // FTS5 OR operator: search for notes with "python" OR "cli"
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=python+OR+cli")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["items"].as_array().unwrap().len(),
        2,
        "Should find notes with 'python' OR 'cli' tags"
    );
}

// =============================================================================
// WebSocket Broadcast Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_note_broadcasts_notification() {
    let (app, notifier) = test_app_with_notifier().await;
    let mut rx = notifier.subscribe();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Broadcast Test",
                        "content": "Testing broadcast",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    let note_id = body["id"].as_str().unwrap().to_string();

    // Should receive broadcast
    let msg = rx.try_recv().expect("Should receive broadcast");
    assert_eq!(msg, UpdateMessage::NoteCreated { note_id });
}

#[tokio::test(flavor = "multi_thread")]
async fn update_note_broadcasts_notification() {
    let (app, notifier) = test_app_with_notifier().await;

    // Create note first
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Original",
                        "content": "Content",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let note = json_body(create_response).await;
    let note_id = note["id"].as_str().unwrap();

    // Subscribe after create to only get update notification
    let mut rx = notifier.subscribe();

    // Update the note
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes/{}", note_id))
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Updated",
                        "content": "Updated content",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Should receive broadcast
    let msg = rx.try_recv().expect("Should receive broadcast");
    assert_eq!(
        msg,
        UpdateMessage::NoteUpdated {
            note_id: note_id.to_string()
        }
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_note_broadcasts_notification() {
    let (app, notifier) = test_app_with_notifier().await;

    // Create note first
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "To Delete",
                        "content": "Content",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let note = json_body(create_response).await;
    let note_id = note["id"].as_str().unwrap();

    // Subscribe after create
    let mut rx = notifier.subscribe();

    // Delete the note
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes/{}", note_id))
                .method("DELETE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Should receive broadcast
    let msg = rx.try_recv().expect("Should receive broadcast");
    assert_eq!(
        msg,
        UpdateMessage::NoteDeleted {
            note_id: note_id.to_string()
        }
    );
}

// =============================================================================
// Hierarchical Notes Tests (parent_id and idx)
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_note_with_parent() {
    let app = test_app().await;

    // Create parent note
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

    assert_eq!(parent_response.status(), StatusCode::CREATED);
    let parent = json_body(parent_response).await;
    let parent_id = parent["id"].as_str().unwrap();

    // Create child note with parent_id
    let child_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Child Note",
                        "content": "Child content",
                        "parent_id": parent_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(child_response.status(), StatusCode::CREATED);
    let child = json_body(child_response).await;
    assert_eq!(child["title"], "Child Note");
    assert_eq!(child["parent_id"], parent_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_note_idx() {
    let app = test_app().await;

    // Create note
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test Note",
                        "content": "Content",
                        "idx": 10
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let note = json_body(create_response).await;
    let note_id = note["id"].as_str().unwrap();
    assert_eq!(note["idx"], 10);

    // Update idx
    let update_response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test Note",
                        "content": "Content",
                        "idx": 20
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_response.status(), StatusCode::OK);
    let updated = json_body(update_response).await;
    assert_eq!(updated["idx"], 20);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_subnotes_filtered() {
    let app = test_app().await;

    // Create parent note
    let parent_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Parent",
                        "content": "Parent"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let parent = json_body(parent_response).await;
    let parent_id = parent["id"].as_str().unwrap();

    // Create child notes with different idx values
    let children = vec![("Child 1", 30), ("Child 2", 10), ("Child 3", 20)];

    for (title, idx) in children {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/notes")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "title": title,
                            "content": "Content",
                            "parent_id": parent_id,
                            "idx": idx
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // List subnotes filtered by parent_id
    let list_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes?parent_id={}", parent_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);
    let body = json_body(list_response).await;
    let items = body["items"].as_array().unwrap();

    assert_eq!(items.len(), 3);
    // Should be ordered by idx (10, 20, 30)
    assert_eq!(items[0]["title"], "Child 2");
    assert_eq!(items[0]["idx"], 10);
    assert_eq!(items[1]["title"], "Child 3");
    assert_eq!(items[1]["idx"], 20);
    assert_eq!(items[2]["title"], "Child 1");
    assert_eq!(items[2]["idx"], 30);
}
