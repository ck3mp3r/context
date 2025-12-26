//! Integration tests for Note API endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::{AppState, routes};
use crate::db::{Database, SqliteDatabase};

fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory().expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");
    let state = AppState::new(db);
    routes::create_router(state)
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// GET /v1/notes - List Notes
// =============================================================================

#[tokio::test]
async fn list_notes_initially_empty() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert!(body.as_array().unwrap().is_empty());
}

// =============================================================================
// POST /v1/notes - Create Note
// =============================================================================

#[tokio::test]
async fn create_note_returns_created() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "My Note",
                        "content": "This is the note content",
                        "tags": ["rust", "api"],
                        "note_type": "manual"
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
    assert_eq!(body["note_type"], "manual");
    assert!(body["id"].as_str().unwrap().len() == 8);
}

#[tokio::test]
async fn create_note_minimal() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/notes")
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
    assert_eq!(body["note_type"], "manual");
}

// =============================================================================
// GET /v1/notes/{id} - Get Note
// =============================================================================

#[tokio::test]
async fn get_note_returns_note() {
    let app = test_app();

    // Create first
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/notes")
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
                .uri(format!("/v1/notes/{}", note_id))
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

#[tokio::test]
async fn get_note_not_found() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/notes/nonexist")
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

#[tokio::test]
async fn update_note_returns_updated() {
    let app = test_app();

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/notes")
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
                .uri(format!("/v1/notes/{}", note_id))
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

#[tokio::test]
async fn update_note_not_found() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/notes/nonexist")
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

#[tokio::test]
async fn delete_note_returns_no_content() {
    let app = test_app();

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/notes")
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
                .uri(format!("/v1/notes/{}", note_id))
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
                .uri(format!("/v1/notes/{}", note_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_note_not_found() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/v1/notes/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// GET /v1/notes/search?q= - Search Notes (FTS)
// =============================================================================

#[tokio::test]
async fn search_notes_returns_matching() {
    let app = test_app();

    // Create notes with different content
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/notes")
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
                .uri("/v1/notes")
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
                .uri("/v1/notes/search?q=rust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let notes = body.as_array().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["title"], "Rust Programming");
}

#[tokio::test]
async fn search_notes_empty_query_returns_empty() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/notes/search?q=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn search_notes_no_match_returns_empty() {
    let app = test_app();

    // Create a note
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/notes")
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
                .uri("/v1/notes/search?q=nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert!(body.as_array().unwrap().is_empty());
}
