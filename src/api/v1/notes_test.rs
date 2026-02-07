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
        std::path::PathBuf::from("/tmp/skills"),
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
        std::path::PathBuf::from("/tmp/skills"),
    );
    (routes::create_router(state, false), notifier)
}

/// Helper to parse JSON response body
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn list_and_filter_notes() {
    let app = test_app().await;

    // Create test data: 2 projects, 3 parent notes, 2 subnotes
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
                        "content": "Rust programming",
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

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Child Note",
                        "content": "Subnote",
                        "parent_id": &parent_id,
                        "idx": 10
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
                        "content": "Python guide",
                        "tags": ["python"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter by project_id
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
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);

    // Filter by tags
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

    // Filter by parent_id (returns subnotes in idx order)
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
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["idx"], 10);

    // Filter by type=note (parent notes only)
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
    assert_eq!(body["total"], 2);

    // Filter by type=subnote (child notes only)
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
    assert_eq!(body["total"], 1);

    // Ordering (asc by title)
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
    assert_eq!(body["items"][0]["title"], "Parent Note");
    assert_eq!(body["items"][1]["title"], "Python Note");

    // Pagination
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?limit=1&offset=0&type=note")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["total"], 2);
    assert_eq!(body["limit"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn crud_operations() {
    let app = test_app().await;

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

    // CREATE
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
                        "tags": ["test"],
                        "project_ids": [&project_id],
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
    assert_eq!(created["title"], "Test Note");
    assert_eq!(created["idx"], 42);

    // GET
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

    // GET 404
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

    // Create parent note for hierarchy testing
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

    // PATCH partial update (only title)
    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Updated"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_response.status(), StatusCode::OK);
    let patched = json_body(patch_response).await;
    assert_eq!(patched["title"], "Updated");
    assert_eq!(patched["content"], "Content"); // unchanged

    // PATCH with parent_id (tests relationships)
    app.clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "parent_id": &parent_id,
                        "tags": ["updated"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // PATCH clear optional field (idx to null)
    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"idx": null})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let patched = json_body(patch_response).await;
    assert!(patched["idx"].is_null());

    // PATCH 404
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/notes/nonexistent")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "X"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // PUT full replacement
    let put_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Replaced",
                        "content": "New"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);
    let replaced = json_body(put_response).await;
    assert_eq!(replaced["title"], "Replaced");

    // DELETE
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

    // Verify deleted (404)
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
async fn fts5_search() {
    let app = test_app().await;

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Rust Programming",
                        "content": "Learning Axum",
                        "tags": ["rust"]
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
                        "content": "Django framework",
                        "tags": ["python"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search by title
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
    assert_eq!(body["total"], 1);

    // Search by content
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=Axum")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);

    // Search by tags
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

    // Boolean AND
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=rust+AND+Axum")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);

    // Boolean OR
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/notes?q=python+OR+rust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn websocket_broadcasts() {
    let (app, notifier) = test_app_with_notifier().await;

    // CREATE broadcasts
    let mut rx = notifier.subscribe();
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"title": "Test", "content": "Content"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let note = json_body(create_response).await;
    let note_id = note["id"].as_str().unwrap().to_string();
    let msg = rx.try_recv().expect("Should receive create broadcast");
    assert_eq!(
        msg,
        UpdateMessage::NoteCreated {
            note_id: note_id.clone()
        }
    );

    // UPDATE broadcasts
    let mut rx = notifier.subscribe();
    app.clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/notes/{}", note_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"title": "Updated", "content": "New"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let msg = rx.try_recv().expect("Should receive update broadcast");
    assert_eq!(
        msg,
        UpdateMessage::NoteUpdated {
            note_id: note_id.clone()
        }
    );

    // DELETE broadcasts
    let mut rx = notifier.subscribe();
    app.oneshot(
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/notes/{}", note_id))
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    let msg = rx.try_recv().expect("Should receive delete broadcast");
    assert_eq!(msg, UpdateMessage::NoteDeleted { note_id });
}
