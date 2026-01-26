//! Integration tests for the Skill API endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::notifier::ChangeNotifier;
use crate::api::{AppState, routes};
use crate::db::{Database, SqliteDatabase};

/// Setup test app with real in-memory DB and skills API routes
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

/// Helper for parsing JSON response body
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_empty() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
    assert_eq!(body["total"], 0);
    assert_eq!(body["limit"], 50);
    assert_eq!(body["offset"], 0);
}

// --- ==== CRUD & Validation Tests ==== ---

#[tokio::test(flavor = "multi_thread")]
async fn test_create_skill() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Test Skill",
                        "description": "A skill description",
                        "instructions": "Follow these steps",
                        "tags": ["tag1", "tag2"],
                        "project_ids": []
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_body(response).await;
    assert_eq!(created["name"], "Test Skill");
    assert_eq!(created["description"], "A skill description");
    assert_eq!(created["instructions"], "Follow these steps");
    assert_eq!(created["tags"], json!(["tag1", "tag2"]));
    assert!(!created["id"].as_str().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_skill_missing_field() {
    let app = test_app().await;

    // Missing required "name"
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "description": "oops",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_by_id() {
    let app = test_app().await;
    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Skill1"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // GET
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/skills/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let got = json_body(response).await;
    assert_eq!(got["id"], id);
    assert_eq!(got["name"], "Skill1");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_not_found() {
    let app = test_app().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills/skl_NONEXISTENT")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_patch() {
    let app = test_app().await;
    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(json!({"name": "PatchMe"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // PATCH
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/skills/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "PatchedName", "tags": ["edited"]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let patched = json_body(response).await;
    assert_eq!(patched["id"], id);
    assert_eq!(patched["name"], "PatchedName");
    assert_eq!(patched["tags"], json!(["edited"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_not_found() {
    let app = test_app().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/skills/skl_NOTREAL")
                .header("content-type", "application/json")
                .body(Body::from(json!({"name": "X"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_put() {
    let app = test_app().await;
    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(json!({"name": "PutMe"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // PUT (full replacement)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/skills/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "PutName", "description": "desc"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let put = json_body(response).await;
    assert_eq!(put["id"], id);
    assert_eq!(put["name"], "PutName");
    assert_eq!(put["description"], "desc");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_skill() {
    let app = test_app().await;
    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(json!({"name": "DeleteMe"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // DELETE
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/skills/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Confirm deleted
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/skills/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_skill_not_found() {
    let app = test_app().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/skills/skl_FAKEDELETE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_non_empty() {
    let app = test_app().await;

    // Insert multiple
    for i in 0..3 {
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/skills")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name": format!("Skill{}", i)}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 3);
    assert_eq!(body["total"], 3);
    assert_eq!(body["limit"], 50);
    assert_eq!(body["offset"], 0);
}

// Edge: Invalid UUID (should be consistent, but uses string keys like "skl_", so skip if UUID not enforced)
// Edge: Add/modify more detailed validation tests if Skill fields are restricted further

// If filtering, sorting, or pagination implemented for Skill, add tests mirroring notes_test.rs here as well.
