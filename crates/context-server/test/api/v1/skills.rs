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
    skills_dir: std::path::PathBuf,
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
    let skills_dir = temp_dir.path().join("skills");
    let state = AppState::new(
        db,
        SyncManager::new(MockGitOps::new()),
        notifier.clone(),
        skills_dir.clone(),
    );

    TestApp {
        router: routes::create_router(state, false),
        notifier,
        skills_dir,
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

// =============================================================================
// Refresh helpers
// =============================================================================

/// Write a skill source directory: SKILL.md plus the given attachment files.
fn write_skill_source(dir: &std::path::Path, name: &str, description: &str, attachments: &[&str]) {
    std::fs::create_dir_all(dir).expect("source directory should be created");
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n# Body\n"),
    )
    .expect("SKILL.md should be written");
    for attachment in attachments {
        std::fs::write(dir.join(attachment), format!("content of {attachment}"))
            .expect("attachment should be written");
    }
}

/// Remove an attachment file from a skill source directory.
fn remove_skill_source_file(dir: &std::path::Path, file: &str) {
    std::fs::remove_file(dir.join(file)).expect("attachment should be removed");
}

/// Import a skill from a local source directory; return the response body.
async fn import_skill_from(
    app: &axum::Router,
    source: &str,
    tags: &[&str],
    project_ids: &[&str],
) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills/import")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "source": source,
                        "tags": tags,
                        "project_ids": project_ids
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await
}

/// Create a project; return its ID.
async fn create_project(app: &axum::Router, title: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "title": title })).unwrap(),
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

/// POST to the refresh endpoint.
async fn post_refresh(app: &axum::Router, skill_id: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/skills/{skill_id}/refresh"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

// =============================================================================
// Refresh: criteria 1, 2, 6, 7
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn refresh_skill_reimports_source_and_rebuilds_cache() {
    let app = test_app().await;
    let project_id = create_project(&app.router, "Refresh Project").await;

    let source_dir = TempDir::new().expect("source temp dir should be created");
    let source = source_dir
        .path()
        .to_str()
        .expect("source path should be valid UTF-8")
        .to_string();

    // v1: description D1 with attachment a.txt.
    write_skill_source(source_dir.path(), "refresh-skill", "D1", &["a.txt"]);
    let imported = import_skill_from(&app.router, &source, &["keep"], &[&project_id]).await;
    let skill_id = imported["id"]
        .as_str()
        .expect("id must be a string")
        .to_string();
    assert_eq!(imported["source"], json!(source));

    let cache_dir = app.skills_dir.join("refresh-skill");
    assert!(
        cache_dir.join("a.txt").exists(),
        "import must cache the v1 attachment"
    );

    // v2: description D2, a.txt dropped, b.txt added.
    write_skill_source(source_dir.path(), "refresh-skill", "D2", &["b.txt"]);
    remove_skill_source_file(source_dir.path(), "a.txt");

    let mut rx = app.notifier.subscribe();
    let response = post_refresh(&app.router, &skill_id).await;
    assert_eq!(response.status(), StatusCode::OK);
    let refreshed = json_body(response).await;
    assert_eq!(refreshed["description"], json!("D2"));
    assert_eq!(refreshed["source"], json!(source));
    assert!(
        refreshed["enabled"]
            .as_bool()
            .expect("enabled must be a bool"),
        "a skill that was enabled must stay enabled"
    );

    let message = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("refresh must broadcast within the timeout")
        .expect("refresh must broadcast a message");
    assert_eq!(
        message,
        UpdateMessage::SkillUpdated {
            skill_id: skill_id.clone()
        }
    );

    // The stored row carries the refreshed description and keeps tags and
    // project links.
    let body = get_skill(&app.router, &skill_id).await;
    assert_eq!(body["description"], json!("D2"));
    assert_eq!(body["tags"], json!(["keep"]));
    assert_eq!(body["project_ids"], json!([project_id]));
    assert_eq!(body["source"], json!(source));

    // The cache holds the refreshed content and only the current attachments.
    let cached_skill_md = std::fs::read_to_string(cache_dir.join("SKILL.md"))
        .expect("cached SKILL.md should be readable");
    assert!(
        cached_skill_md.contains("D2"),
        "cached SKILL.md must hold the refreshed content"
    );
    assert!(
        cache_dir.join("b.txt").exists(),
        "the refreshed attachment must be cached"
    );
    assert!(
        !cache_dir.join("a.txt").exists(),
        "an attachment dropped from the source must not survive in the cache"
    );
}

// =============================================================================
// Refresh: criterion 3
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn refresh_skill_does_not_create_cache_when_disabled() {
    let app = test_app().await;

    let source_dir = TempDir::new().expect("source temp dir should be created");
    let source = source_dir
        .path()
        .to_str()
        .expect("source path should be valid UTF-8")
        .to_string();

    write_skill_source(source_dir.path(), "disabled-skill", "D1", &[]);
    let imported = import_skill_from(&app.router, &source, &[], &[]).await;
    let skill_id = imported["id"]
        .as_str()
        .expect("id must be a string")
        .to_string();

    let response = post_toggle(&app.router, &skill_id, "disable").await;
    assert_eq!(response.status(), StatusCode::OK);
    let cache_dir = app.skills_dir.join("disabled-skill");
    assert!(
        !cache_dir.exists(),
        "disable must remove the cache directory"
    );

    write_skill_source(source_dir.path(), "disabled-skill", "D2", &[]);
    let response = post_refresh(&app.router, &skill_id).await;
    assert_eq!(response.status(), StatusCode::OK);
    let refreshed = json_body(response).await;
    assert_eq!(refreshed["description"], json!("D2"));
    assert!(
        !refreshed["enabled"]
            .as_bool()
            .expect("enabled must be a bool"),
        "a skill that was disabled must stay disabled"
    );
    assert!(
        !cache_dir.exists(),
        "refresh must not create a cache directory for a disabled skill"
    );
}

// =============================================================================
// Refresh: criteria 4, 5, 7
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn refresh_skill_without_source_returns_bad_request() {
    let app = test_app().await;
    let skill_id = create_skill(&app.router, "sourceless-skill").await;

    let body = get_skill(&app.router, &skill_id).await;
    assert!(
        body["source"].is_null(),
        "a skill created through the API must report a null source"
    );

    let response = post_refresh(&app.router, &skill_id).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json_body(response).await;
    let error = body["error"].as_str().expect("error must be a string");
    assert!(
        error.contains("no import source"),
        "error must mention the missing source, got: {error}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn refresh_skill_unknown_id_returns_not_found() {
    let app = test_app().await;

    let response = post_refresh(&app.router, "nonexist").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
