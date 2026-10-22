//! Co-located tests for skill REST handlers.
//!
//! Covers the `enabled` field on skill responses (computed from cache
//! directory presence) and the `SkillUpdated` broadcasts from the
//! enable/disable handlers.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::notifier::{ChangeNotifier, UpdateMessage};
use crate::api::{AppState, routes};
use context_core::Database;
use context_db::SqliteDatabase;
use context_sync::{MockGitOps, SyncManager};
use tempfile::TempDir;

/// Test app plus the notifier used to observe broadcasts.
struct TestApp {
    router: axum::Router,
    notifier: ChangeNotifier,
    _temp_dir: TempDir,
}

/// Create a test app backed by an in-memory SQLite database.
async fn test_app() -> TestApp {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    let temp_dir = TempDir::new().expect("Failed to create test temp dir");
    let notifier = ChangeNotifier::new();
    let state = AppState::new(
        db,
        SyncManager::new(MockGitOps::new()),
        notifier.clone(),
        temp_dir.path().join("skills"),
    );

    TestApp {
        router: routes::create_router(state, false),
        notifier,
        _temp_dir: temp_dir,
    }
}

/// Parse a JSON response body.
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

/// Create a skill with valid frontmatter; return its ID.
async fn create_skill(app: &axum::Router, name: &str) -> String {
    let content = format!("---\nname: {name}\ndescription: d\n---\n# Body");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": name,
                        "description": "Test skill",
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

/// Create a skill whose content has no parseable frontmatter name.
async fn create_unnamed_skill(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "unnamed",
                        "description": "Test skill",
                        "content": "---\ndescription: d\n---\n# Body"
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

/// Send a POST to an enable/disable endpoint.
async fn post_toggle(app: &axum::Router, skill_id: &str, action: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/skills/{skill_id}/{action}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

/// GET a single skill.
async fn get_skill(app: &axum::Router, skill_id: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/skills/{skill_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

// =============================================================================
// Criterion 1: enabled is true when the cache directory exists
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_skill_reports_enabled_after_enable() {
    let app = test_app().await;
    let skill_id = create_skill(&app.router, "test-skill").await;

    let response = post_toggle(&app.router, &skill_id, "enable").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = get_skill(&app.router, &skill_id).await;
    assert!(
        body["enabled"].as_bool().expect("enabled must be a bool"),
        "enabled must be true after enable"
    );
}

// =============================================================================
// Criterion 2: enabled is false when the cache directory is absent
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_skill_reports_disabled_when_cache_missing() {
    let app = test_app().await;
    let skill_id = create_skill(&app.router, "test-skill").await;

    let body = get_skill(&app.router, &skill_id).await;
    assert!(
        !body["enabled"].as_bool().expect("enabled must be a bool"),
        "enabled must be false before enable"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_skill_reports_disabled_after_disable() {
    let app = test_app().await;
    let skill_id = create_skill(&app.router, "test-skill").await;

    let response = post_toggle(&app.router, &skill_id, "enable").await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = post_toggle(&app.router, &skill_id, "disable").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = get_skill(&app.router, &skill_id).await;
    assert!(
        !body["enabled"].as_bool().expect("enabled must be a bool"),
        "enabled must be false after disable"
    );
}

// =============================================================================
// Criterion 3: list items carry the enabled field
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_skills_includes_enabled_state() {
    let app = test_app().await;
    let enabled_id = create_skill(&app.router, "enabled-skill").await;
    let disabled_id = create_skill(&app.router, "disabled-skill").await;

    let response = post_toggle(&app.router, &enabled_id, "enable").await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .router
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
    let items = body["items"].as_array().expect("items must be an array");
    assert_eq!(items.len(), 2);

    for item in items {
        let id = item["id"].as_str().expect("item must have an id");
        let enabled = item["enabled"].as_bool().expect("enabled must be a bool");
        assert_eq!(
            enabled,
            id == enabled_id,
            "enabled must match cache presence for {id}"
        );
    }

    let ids: Vec<&str> = items.iter().filter_map(|i| i["id"].as_str()).collect();
    assert!(ids.contains(&enabled_id.as_str()));
    assert!(ids.contains(&disabled_id.as_str()));
}

// =============================================================================
// Criterion 4: unparseable content reports enabled false
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_skill_without_parseable_name_reports_disabled() {
    let app = test_app().await;
    let skill_id = create_unnamed_skill(&app.router).await;

    let body = get_skill(&app.router, &skill_id).await;
    assert!(
        !body["enabled"].as_bool().expect("enabled must be a bool"),
        "unparseable content must report disabled"
    );
}

// =============================================================================
// Criterion 5: enable broadcasts SkillUpdated
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn enable_skill_broadcasts_skill_updated() {
    let app = test_app().await;
    let skill_id = create_skill(&app.router, "test-skill").await;
    let mut rx = app.notifier.subscribe();

    let response = post_toggle(&app.router, &skill_id, "enable").await;
    assert_eq!(response.status(), StatusCode::OK);

    let message = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("enable must broadcast within the timeout")
        .expect("enable must broadcast a message");
    assert_eq!(
        message,
        UpdateMessage::SkillUpdated {
            skill_id: skill_id.clone()
        }
    );
}

// =============================================================================
// Criterion 6: disable broadcasts SkillUpdated
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn disable_skill_broadcasts_skill_updated() {
    let app = test_app().await;
    let skill_id = create_skill(&app.router, "test-skill").await;

    let response = post_toggle(&app.router, &skill_id, "enable").await;
    assert_eq!(response.status(), StatusCode::OK);

    let mut rx = app.notifier.subscribe();
    let response = post_toggle(&app.router, &skill_id, "disable").await;
    assert_eq!(response.status(), StatusCode::OK);

    let message = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("disable must broadcast within the timeout")
        .expect("disable must broadcast a message");
    assert_eq!(
        message,
        UpdateMessage::SkillUpdated {
            skill_id: skill_id.clone()
        }
    );
}
