//! Co-located tests for note REST handlers.
//!
//! Covers the list payload rule: list and search results are metadata-only;
//! single-entity responses keep the full content.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::{AppState, routes};
use context_core::Database;
use context_db::SqliteDatabase;
use context_sync::{MockGitOps, SyncManager};
use tempfile::TempDir;

/// Content longer than 100 chars so a blanked placeholder is distinguishable.
const NOTE_CONTENT: &str = "This note body is deliberately longer than one hundred characters so that a blanked response field is easy to tell apart from the stored value.";

/// Create a test app backed by an in-memory SQLite database.
async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    let temp_dir = TempDir::new().unwrap();
    let state = AppState::new(
        db,
        SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    routes::create_router(state, false)
}

/// Parse a JSON response body.
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

/// Create a note with the shared content; return its ID.
async fn create_note(app: &axum::Router, title: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": title,
                        "content": NOTE_CONTENT
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// GET the note list with an optional raw query string.
async fn list_notes(app: &axum::Router, query: &str) -> Value {
    let uri = if query.is_empty() {
        "/api/v1/notes".to_string()
    } else {
        format!("/api/v1/notes?{query}")
    };
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_returns_metadata_only() {
    let app = test_app().await;
    let note_id = create_note(&app, "Listed note").await;

    let body = list_notes(&app, "").await;

    assert_eq!(body["total"], 1);
    assert_eq!(body["limit"], 50);
    assert_eq!(body["offset"], 0);
    assert_eq!(body["items"][0]["id"], json!(note_id));
    assert_eq!(
        body["items"][0]["content"],
        json!(""),
        "list must omit note content"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_search_returns_metadata_only() {
    let app = test_app().await;
    create_note(&app, "Rust Note").await;

    let body = list_notes(&app, "q=deliberately").await;

    assert_eq!(body["total"], 1);
    assert_eq!(
        body["items"][0]["content"],
        json!(""),
        "search results must project to metadata"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_empty_query_returns_empty_list() {
    let app = test_app().await;
    create_note(&app, "Listed note").await;

    let body = list_notes(&app, "q=").await;

    assert_eq!(body["total"], 0);
    assert_eq!(body["items"], json!([]));
    assert_eq!(body["offset"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_note_still_returns_content() {
    let app = test_app().await;
    let note_id = create_note(&app, "Listed note").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/notes/{note_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["content"],
        json!(NOTE_CONTENT),
        "single-entity GET must keep the stored content"
    );
}
