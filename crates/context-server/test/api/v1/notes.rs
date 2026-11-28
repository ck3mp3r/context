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

/// Multi-line content with real newlines, including a blank line.
const MULTILINE_CONTENT: &str = "First line\nSecond line\n\nFourth line";

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
    create_note_with_content(app, title, NOTE_CONTENT).await
}

/// Create a note with custom content; return its ID.
async fn create_note_with_content(app: &axum::Router, title: &str, content: &str) -> String {
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
                        "content": content
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
async fn list_notes_include_content_returns_content() {
    let app = test_app().await;
    create_note(&app, "Listed note").await;

    let body = list_notes(&app, "include_content=true").await;

    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["content"], json!(NOTE_CONTENT));
}

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_search_returns_content() {
    let app = test_app().await;
    create_note(&app, "Rust Note").await;

    let body = list_notes(&app, "q=deliberately").await;

    assert_eq!(body["total"], 1);
    assert_eq!(
        body["items"][0]["content"],
        json!(NOTE_CONTENT),
        "search results must keep the stored content"
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

#[tokio::test(flavor = "multi_thread")]
async fn create_note_round_trips_multiline_content() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Multi-line note",
                        "content": MULTILINE_CONTENT
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    assert_eq!(
        body["content"],
        json!(MULTILINE_CONTENT),
        "create_note must return the multi-line content verbatim"
    );
    assert!(
        body["content"]
            .as_str()
            .expect("content must be a string")
            .contains('\n'),
        "create_note must keep U+000A characters in the response"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn update_note_round_trips_multiline_content() {
    let app = test_app().await;
    let note_id = create_note(&app, "Updated note").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/notes/{note_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated note",
                        "content": MULTILINE_CONTENT
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["content"],
        json!(MULTILINE_CONTENT),
        "update_note must return the multi-line content verbatim"
    );

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
    let fetched = json_body(response).await;
    assert_eq!(
        fetched["content"],
        json!(MULTILINE_CONTENT),
        "GET after update must return the exact multi-line content"
    );
    assert!(
        fetched["content"]
            .as_str()
            .expect("content must be a string")
            .contains('\n'),
        "GET must keep U+000A characters"
    );
}

/// PATCH a note with a raw JSON body; return the parsed response body.
async fn patch_note(app: &axum::Router, note_id: &str, body: Value) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/notes/{note_id}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

/// PUT a note with a raw JSON body; return the parsed response body.
async fn put_note(app: &axum::Router, note_id: &str, body: Value) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/notes/{note_id}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

/// GET a single note.
async fn get_note(app: &axum::Router, note_id: &str) -> Value {
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
    json_body(response).await
}

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_filters_by_status() {
    let app = test_app().await;
    let active_id = create_note(&app, "Active note").await;
    let archived_id = create_note(&app, "Archived note").await;
    patch_note(&app, &archived_id, json!({"status": "archived"})).await;

    let archived = list_notes(&app, "status=archived").await;
    assert_eq!(
        archived["total"], 1,
        "status=archived must return only archived notes"
    );
    assert_eq!(archived["items"][0]["id"], json!(archived_id));
    assert_eq!(archived["items"][0]["status"], "archived");
    assert!(
        !archived["items"][0]["archived_at"].is_null(),
        "an archived list item must carry archived_at"
    );

    let active = list_notes(&app, "status=active").await;
    assert_eq!(
        active["total"], 1,
        "status=active must return only active notes"
    );
    assert_eq!(active["items"][0]["id"], json!(active_id));

    let both = list_notes(&app, "").await;
    assert_eq!(
        both["total"], 2,
        "omitting status must return both statuses"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_notes_combines_query_and_status() {
    let app = test_app().await;
    let archived_id = create_note_with_content(
        &app,
        "Archived match",
        "The archived body mentions deliberately.",
    )
    .await;
    create_note_with_content(&app, "Active non-match", "Nothing relevant in this body.").await;
    patch_note(&app, &archived_id, json!({"status": "archived"})).await;

    let search_only = list_notes(&app, "q=deliberately").await;
    assert_eq!(
        search_only["total"], 1,
        "q alone must match the archived note"
    );

    let archived = list_notes(&app, "q=deliberately&status=archived").await;
    assert_eq!(
        archived["total"], 1,
        "q with status=archived must return the archived match"
    );
    assert_eq!(archived["items"][0]["id"], json!(archived_id));
    assert_eq!(archived["items"][0]["status"], "archived");

    let active = list_notes(&app, "q=deliberately&status=active").await;
    assert_eq!(
        active["total"], 0,
        "q with status=active must exclude the archived match"
    );
    assert_eq!(active["items"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_note_archives_unarchives_and_ignores_unknown_status() {
    let app = test_app().await;
    let note_id = create_note(&app, "Archivable note").await;

    let archived = patch_note(&app, &note_id, json!({"status": "archived"})).await;
    assert_eq!(archived["status"], "archived");
    assert!(
        !archived["archived_at"].is_null(),
        "PATCH archive must set archived_at"
    );

    let still_archived = patch_note(&app, &note_id, json!({"status": "bogus"})).await;
    assert_eq!(
        still_archived["status"], "archived",
        "an unrecognized status must be ignored"
    );
    assert!(
        !still_archived["archived_at"].is_null(),
        "an unrecognized status must keep archived_at"
    );

    let active = patch_note(&app, &note_id, json!({"status": "active"})).await;
    assert_eq!(active["status"], "active");
    assert!(
        active["archived_at"].is_null(),
        "PATCH active must clear archived_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn put_note_status_archives_and_unarchives() {
    let app = test_app().await;
    let note_id = create_note(&app, "Put archivable note").await;

    let archived = put_note(
        &app,
        &note_id,
        json!({"title": "Put archivable note", "content": NOTE_CONTENT, "status": "archived"}),
    )
    .await;
    assert_eq!(archived["status"], "archived");
    assert!(
        !archived["archived_at"].is_null(),
        "PUT archive must set archived_at in the response"
    );

    let fetched = get_note(&app, &note_id).await;
    assert_eq!(fetched["status"], "archived");
    assert!(
        !fetched["archived_at"].is_null(),
        "PUT archive must set archived_at through the repository"
    );

    let active = put_note(
        &app,
        &note_id,
        json!({"title": "Put archivable note", "content": NOTE_CONTENT, "status": "active"}),
    )
    .await;
    assert_eq!(active["status"], "active");
    assert!(
        active["archived_at"].is_null(),
        "PUT active must clear archived_at in the response"
    );

    let fetched = get_note(&app, &note_id).await;
    assert_eq!(fetched["status"], "active");
    assert!(
        fetched["archived_at"].is_null(),
        "PUT active must clear archived_at"
    );
}
