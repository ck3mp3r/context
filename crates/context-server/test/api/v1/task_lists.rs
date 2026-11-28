//! Co-located tests for task-list REST handlers.
//!
//! Covers the list payload rule: container listings omit the freeform notes
//! markdown; single-entity responses keep it. Also covers PUT archive state:
//! the repository owns archived_at and the response reflects the stored row.

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

/// PUT a task-list body and return the raw response.
async fn put_task_list(app: &axum::Router, id: &str, body: Value) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/task-lists/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

/// PUT a task-list body, assert HTTP 200, and return the response body.
async fn put_task_list_ok(app: &axum::Router, id: &str, body: Value) -> Value {
    let response = put_task_list(app, id, body).await;
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

/// GET one task list, assert HTTP 200, and return the response body.
async fn get_task_list(app: &axum::Router, id: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
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

#[tokio::test(flavor = "multi_thread")]
async fn put_task_list_archive_sets_archived_at() {
    let app = test_app().await;
    let list_id = create_list_with_notes(&app, "Archive Me").await;

    let body = put_task_list_ok(
        &app,
        &list_id,
        json!({"title": "Updated", "status": "archived"}),
    )
    .await;

    let archived_at = body["archived_at"]
        .as_str()
        .expect("PUT archive response must carry a non-null archived_at")
        .to_string();
    assert_eq!(body["status"], "archived");

    let stored = get_task_list(&app, &list_id).await;
    assert_eq!(
        stored["archived_at"],
        json!(archived_at),
        "GET must return the same archived_at the PUT response reported"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn put_task_list_unarchive_clears_archived_at() {
    let app = test_app().await;
    let list_id = create_list_with_notes(&app, "Unarchive Me").await;

    put_task_list_ok(
        &app,
        &list_id,
        json!({"title": "Updated", "status": "archived"}),
    )
    .await;

    let body = put_task_list_ok(
        &app,
        &list_id,
        json!({"title": "Updated", "status": "active"}),
    )
    .await;

    assert!(
        body["archived_at"].is_null(),
        "unarchive response must report archived_at null, got: {}",
        body["archived_at"]
    );
    assert_eq!(body["status"], "active");

    let stored = get_task_list(&app, &list_id).await;
    assert!(
        stored["archived_at"].is_null(),
        "unarchived task list must stay cleared, got: {}",
        stored["archived_at"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn put_task_list_rearchive_preserves_archived_at() {
    let app = test_app().await;
    let list_id = create_list_with_notes(&app, "Re-archive Me").await;

    let first = put_task_list_ok(
        &app,
        &list_id,
        json!({"title": "Updated", "status": "archived"}),
    )
    .await;
    let first_archived_at = first["archived_at"]
        .as_str()
        .expect("first archive response must carry a non-null archived_at")
        .to_string();

    let second = put_task_list_ok(
        &app,
        &list_id,
        json!({"title": "Updated", "status": "archived"}),
    )
    .await;

    assert_eq!(
        second["archived_at"],
        json!(first_archived_at),
        "repository idempotence must preserve the original archived_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn put_task_list_without_status_leaves_archived_at() {
    let app = test_app().await;
    let list_id = create_list_with_notes(&app, "Keep Archive").await;

    let archived = put_task_list_ok(
        &app,
        &list_id,
        json!({"title": "Updated", "status": "archived"}),
    )
    .await;
    let archived_at = archived["archived_at"]
        .as_str()
        .expect("archive response must carry a non-null archived_at")
        .to_string();

    let renamed = put_task_list_ok(&app, &list_id, json!({"title": "Renamed"})).await;

    assert_eq!(
        renamed["archived_at"],
        json!(archived_at),
        "PUT without status must leave archived_at unchanged"
    );
    assert_eq!(renamed["status"], "archived");
    assert_eq!(renamed["title"], "Renamed");
}
