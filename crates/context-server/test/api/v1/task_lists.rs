//! Co-located tests for task-list REST handlers.
//!
//! Covers the list payload rule: container listings omit the freeform notes
//! markdown; single-entity responses keep it.

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

/// Multi-paragraph notes so a null placeholder is distinguishable.
const LIST_NOTES: &str = "## Context\n\nThis container note block is deliberately longer than one line so that a null response field is easy to distinguish from the stored value.\n\n## Decisions\n\nKeep the notes out of list payloads.";

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

/// Create a project and a task list carrying the shared notes; return the list ID.
async fn create_list_with_notes(app: &axum::Router, title: &str) -> String {
    let project = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Notes Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_id = json_body(project).await["id"].as_str().unwrap().to_string();

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": title,
                        "description": "Short description",
                        "notes": LIST_NOTES,
                        "project_id": project_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::CREATED);
    json_body(list).await["id"].as_str().unwrap().to_string()
}

/// GET the task-list collection with an optional raw query string.
async fn list_task_lists(app: &axum::Router, query: &str) -> Value {
    let uri = if query.is_empty() {
        "/api/v1/task-lists".to_string()
    } else {
        format!("/api/v1/task-lists?{query}")
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
async fn list_task_lists_omits_notes() {
    let app = test_app().await;
    let list_id = create_list_with_notes(&app, "Notes List").await;

    let body = list_task_lists(&app, "").await;

    assert_eq!(body["total"], 1);
    assert_eq!(body["limit"], 50);
    assert_eq!(body["offset"], 0);
    assert_eq!(body["items"][0]["id"], json!(list_id));
    assert!(
        body["items"][0]["notes"].is_null(),
        "list must omit notes, got: {}",
        body["items"][0]["notes"]
    );
    assert_eq!(
        body["items"][0]["description"], "Short description",
        "description stays in list responses"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_task_lists_search_omits_notes() {
    let app = test_app().await;
    create_list_with_notes(&app, "Rust Container").await;

    let body = list_task_lists(&app, "q=rust").await;

    assert_eq!(body["total"], 1);
    assert!(
        body["items"][0]["notes"].is_null(),
        "search results must omit notes without the flag, got: {}",
        body["items"][0]["notes"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_task_lists_include_notes_returns_notes() {
    let app = test_app().await;
    create_list_with_notes(&app, "Notes List").await;

    let body = list_task_lists(&app, "include_notes=true").await;

    assert_eq!(body["total"], 1);
    assert_eq!(
        body["items"][0]["notes"],
        json!(LIST_NOTES),
        "include_notes=true must return the stored notes"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_task_lists_search_include_notes_returns_notes() {
    let app = test_app().await;
    create_list_with_notes(&app, "Rust Container").await;

    let body = list_task_lists(&app, "q=rust&include_notes=true").await;

    assert_eq!(body["total"], 1);
    assert_eq!(
        body["items"][0]["notes"],
        json!(LIST_NOTES),
        "search with include_notes=true must return the stored notes"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_task_list_still_returns_notes() {
    let app = test_app().await;
    let list_id = create_list_with_notes(&app, "Notes List").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{list_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["notes"],
        json!(LIST_NOTES),
        "single-entity GET must keep the stored notes"
    );
}
