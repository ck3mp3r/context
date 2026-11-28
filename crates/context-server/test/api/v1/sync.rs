//! Co-located tests for sync REST handlers.
//!
//! Covers the incremental sync behavior exposed by the API: the status
//! endpoint reports `last_synced_at` and `schema_version`, and the export
//! endpoint reports the number of entities exported in the current
//! incremental sync rather than the total row count in the database.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::api::{AppState, routes};
use context_core::{Database, HasProjects, HasSync, Project, ProjectRepository, SyncRepository};
use context_db::SqliteDatabase;
use context_sync::{GitError, MockGitOps, SyncManager};

fn mock_output(code: i32, stdout: &str, stderr: &str) -> Output {
    Output {
        status: ExitStatus::from_raw(code),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

fn test_app(db: SqliteDatabase, sync_dir: &std::path::Path) -> axum::Router {
    let temp_dir = TempDir::new().unwrap();
    let state = AppState::new(
        db,
        SyncManager::with_sync_dir(MockGitOps::new(), sync_dir.to_path_buf()),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    routes::create_router(state, false)
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn get_status(app: &axum::Router) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/sync/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

// =============================================================================
// Criterion: status endpoint includes last_synced_at and schema_version
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_sync_status_endpoint() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let app = test_app(db, &temp_dir.path().join("sync"));

    let body = get_status(&app).await;
    let data = &body["data"];

    assert_eq!(data["initialized"], false);
    assert!(
        data.get("last_synced_at").is_some(),
        "status must include last_synced_at, got: {}",
        data
    );
    assert!(
        data["schema_version"].as_i64().unwrap_or(0) > 0,
        "status must include a positive schema_version, got: {}",
        data
    );
    assert!(
        data.get("tombstones").is_none(),
        "not-initialized status must not include tombstones, got: {}",
        data
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sync_status_endpoint_initialized() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Err(GitError::GitNotFound));
    mock_git
        .expect_status_porcelain()
        .returning(|_| Ok(mock_output(0, "", "")));

    let state = AppState::new(
        db,
        SyncManager::with_sync_dir(mock_git, sync_dir),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    let app = routes::create_router(state, false);

    let body = get_status(&app).await;
    let data = &body["data"];

    assert_eq!(data["initialized"], true);
    assert!(data.get("last_synced_at").is_some());
    assert!(data["schema_version"].as_i64().unwrap_or(0) > 0);

    let tombstones = &data["tombstones"];
    for key in [
        "repos",
        "projects",
        "task_lists",
        "tasks",
        "notes",
        "skills",
        "attachments",
        "total",
    ] {
        assert!(
            tombstones.get(key).is_some(),
            "tombstones must include {key}, got: {tombstones}"
        );
    }

    assert!(
        data["database"].get("total").is_some(),
        "database object must keep its keys, got: {}",
        data["database"]
    );
    assert!(
        data["sync_files"].get("total").is_some(),
        "sync_files object must keep its keys, got: {}",
        data["sync_files"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sync_status_endpoint_reports_tombstones() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();

    db.projects()
        .create(&Project {
            id: "proj0002".to_string(),
            title: "Deleted Project".to_string(),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some("2024-01-01 00:00:00".to_string()),
            updated_at: Some("2024-01-01 00:00:00".to_string()),
            deleted_at: None,
        })
        .await
        .unwrap();
    db.projects().delete("proj0002").await.unwrap();

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Err(GitError::GitNotFound));
    mock_git
        .expect_status_porcelain()
        .returning(|_| Ok(mock_output(0, "", "")));

    let state = AppState::new(
        db,
        SyncManager::with_sync_dir(mock_git, sync_dir),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    let app = routes::create_router(state, false);

    let body = get_status(&app).await;
    let data = &body["data"];

    assert_eq!(
        data["tombstones"]["projects"], 1,
        "one soft-deleted project must be counted, got: {}",
        data["tombstones"]
    );
    assert_eq!(
        data["tombstones"]["total"], 1,
        "tombstone total must count only the soft-deleted project, got: {}",
        data["tombstones"]
    );
    assert_eq!(
        data["database"]["projects"], 0,
        "live project count must exclude the tombstone, got: {}",
        data["database"]
    );
}

// =============================================================================
// Criterion: export reports incremental count, not total database count
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_sync_export_reports_incremental_count() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();

    db.projects()
        .create(&Project {
            id: "proj0001".to_string(),
            title: "Incremental Project".to_string(),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some("2024-01-01 00:00:00".to_string()),
            updated_at: Some("2024-01-01 00:00:00".to_string()),
            deleted_at: None,
        })
        .await
        .unwrap();

    // A cutoff in the future makes the incremental export match no rows even
    // though the database holds one project.
    db.sync()
        .set_last_synced_at("2999-01-01 00:00:00")
        .await
        .unwrap();

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_add_files()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "", "")));
    mock_git
        .expect_commit()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "commit successful", "")));

    let state = AppState::new(
        db,
        SyncManager::with_sync_dir(mock_git, sync_dir),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    let app = routes::create_router(state, false);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sync/export")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "remote": false })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let data = &body["data"];
    assert_eq!(
        data["changes"], 0,
        "export must report incremental changes, not the database total: {}",
        data
    );
    assert_eq!(data["exported"]["total"], 0);
}

// =============================================================================
// Criterion: POST /api/v1/sync/disable and /enable flip the machine-local marker
// =============================================================================

async fn post_sync_action(app: &axum::Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, json_body(response).await)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_disable_sync_keeps_files_and_writes_marker() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();
    std::fs::write(sync_dir.join("projects.jsonl"), "{\"id\":\"proj0001\"}\n").unwrap();
    std::fs::write(sync_dir.join("sync_meta.json"), "{\"schema_version\": 7}").unwrap();

    let app = test_app(db, &sync_dir);

    let (status, body) = post_sync_action(&app, "/api/v1/sync/disable").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "success");
    assert_eq!(body["data"]["initialized"], true);
    assert_eq!(body["data"]["enabled"], false);
    assert!(
        temp_dir.path().join("sync_disabled").exists(),
        "disable must create the marker next to the sync directory"
    );
    assert!(
        !sync_dir.join("sync_disabled").exists(),
        "the marker must live outside the git repository"
    );
    assert!(
        sync_dir.exists(),
        "disable must keep the sync directory on disk"
    );
    assert!(
        sync_dir.join(".git").exists(),
        "disable must keep the .git directory on disk"
    );
    assert!(
        sync_dir.join("projects.jsonl").exists(),
        "disable must keep the JSONL data on disk"
    );

    let meta = std::fs::read_to_string(sync_dir.join("sync_meta.json")).unwrap();
    assert_eq!(
        meta, "{\"schema_version\": 7}",
        "disable must leave sync_meta.json untouched"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_enable_sync_removes_marker() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();
    std::fs::write(sync_dir.join("projects.jsonl"), "").unwrap();

    let app = test_app(db, &sync_dir);

    let (disable_status, _) = post_sync_action(&app, "/api/v1/sync/disable").await;
    assert_eq!(disable_status, StatusCode::OK);
    assert!(
        temp_dir.path().join("sync_disabled").exists(),
        "disable must create the marker"
    );

    let (status, body) = post_sync_action(&app, "/api/v1/sync/enable").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "success");
    assert_eq!(body["data"]["enabled"], true);
    assert!(
        !temp_dir.path().join("sync_disabled").exists(),
        "enable must delete the marker"
    );
    assert!(
        sync_dir.join(".git").exists(),
        "enable must keep the .git directory on disk"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_disable_sync_uninitialized_returns_conflict() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");

    let app = test_app(db, &sync_dir);

    let (status, body) = post_sync_action(&app, "/api/v1/sync/disable").await;

    assert_eq!(status, StatusCode::CONFLICT);
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("initialized"),
        "the conflict body must name initialization, got: {}",
        body
    );
    assert!(
        !temp_dir.path().join("sync_disabled").exists(),
        "a refused disable must not create the marker"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_while_disabled_returns_conflict() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();

    let app = test_app(db, &sync_dir);

    let (disable_status, _) = post_sync_action(&app, "/api/v1/sync/disable").await;
    assert_eq!(disable_status, StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sync/export")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({})).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = json_body(response).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("disabled"),
        "the conflict body must name the disabled state, got: {}",
        body
    );
}

// =============================================================================
// Criterion: POST /api/v1/sync/remote replaces the configured remote
// =============================================================================

async fn post_remote(app: &axum::Router, payload: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sync/remote")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, json_body(response).await)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_set_sync_remote_replaces_url_and_status_reports_it() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();

    let current = Arc::new(Mutex::new("git@example.com:old.git".to_string()));
    let read_state = Arc::clone(&current);
    let write_state = Arc::clone(&current);

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .times(2)
        .returning(move |_, _| {
            let url = read_state.lock().unwrap().clone();
            Ok(mock_output(0, &format!("{url}\n"), ""))
        });
    mock_git
        .expect_set_url()
        .withf(|_, name, url| name == "origin" && url == "git@example.com:new.git")
        .times(1)
        .returning(move |_, _, url| {
            *write_state.lock().unwrap() = url.to_string();
            Ok(mock_output(0, "", ""))
        });
    mock_git
        .expect_status_porcelain()
        .returning(|_| Ok(mock_output(0, "", "")));

    let state = AppState::new(
        db,
        SyncManager::with_sync_dir(mock_git, sync_dir),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    let app = routes::create_router(state, false);

    let (status, body) =
        post_remote(&app, json!({ "remote_url": "  git@example.com:new.git  " })).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "success");
    assert_eq!(
        body["data"]["remote_url"], "git@example.com:new.git",
        "the response must carry the trimmed URL, got: {}",
        body
    );

    let status_body = get_status(&app).await;
    assert_eq!(
        status_body["data"]["remote_url"], "git@example.com:new.git",
        "the status endpoint must report the new remote URL, got: {}",
        status_body
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_set_sync_remote_adds_remote_to_sync_without_origin() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .times(1)
        .returning(|_, _| Err(GitError::GitNotFound));
    mock_git
        .expect_add_remote()
        .withf(|_, name, url| name == "origin" && url == "git@example.com:new.git")
        .times(1)
        .returning(|_, _, _| Ok(mock_output(0, "", "")));
    mock_git.expect_set_url().times(0);

    let state = AppState::new(
        db,
        SyncManager::with_sync_dir(mock_git, sync_dir),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    let app = routes::create_router(state, false);

    let (status, body) =
        post_remote(&app, json!({ "remote_url": "git@example.com:new.git" })).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["remote_url"], "git@example.com:new.git");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_set_sync_remote_uninitialized_returns_conflict() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let app = test_app(db, &temp_dir.path().join("sync"));

    let (status, body) =
        post_remote(&app, json!({ "remote_url": "git@example.com:new.git" })).await;

    assert_eq!(status, StatusCode::CONFLICT);
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("initialized"),
        "the conflict body must name initialization, got: {}",
        body
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_set_sync_remote_blank_url_returns_bad_request() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();

    let app = test_app(db, &sync_dir);

    let (status, body) = post_remote(&app, json!({ "remote_url": "   " })).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("remote_url"),
        "the error body must name the field, got: {}",
        body
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_clear_sync_remote_removes_origin_and_status_reports_null() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();
    std::fs::write(sync_dir.join("projects.jsonl"), "{\"id\":\"proj0001\"}\n").unwrap();
    std::fs::write(sync_dir.join("sync_meta.json"), "{\"schema_version\": 7}").unwrap();

    let current = Arc::new(Mutex::new(Some("git@example.com:old.git".to_string())));
    let read_state = Arc::clone(&current);
    let write_state = Arc::clone(&current);

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .times(2)
        .returning(move |_, _| match read_state.lock().unwrap().as_deref() {
            Some(url) => Ok(mock_output(0, &format!("{url}\n"), "")),
            None => Err(GitError::GitNotFound),
        });
    mock_git
        .expect_remove_remote()
        .withf(|_, name| name == "origin")
        .times(1)
        .returning(move |_, _| {
            *write_state.lock().unwrap() = None;
            Ok(mock_output(0, "", ""))
        });
    mock_git
        .expect_status_porcelain()
        .returning(|_| Ok(mock_output(0, "", "")));

    let state = AppState::new(
        db,
        SyncManager::with_sync_dir(mock_git, sync_dir.clone()),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    let app = routes::create_router(state, false);

    let (status, body) = post_remote(&app, json!({ "remote_url": null })).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "success");
    assert_eq!(body["message"], "Remote removed");
    assert!(
        body["data"]["remote_url"].is_null(),
        "the clear response must report a null remote_url, got: {}",
        body
    );

    let status_body = get_status(&app).await;
    assert!(
        status_body["data"]["remote_url"].is_null(),
        "the status endpoint must report the remote as absent, got: {}",
        status_body
    );

    assert!(
        sync_dir.join(".git").exists(),
        "clearing the remote must keep the .git directory"
    );
    assert_eq!(
        std::fs::read_to_string(sync_dir.join("projects.jsonl")).unwrap(),
        "{\"id\":\"proj0001\"}\n",
        "clearing the remote must keep the JSONL mirror"
    );
    assert_eq!(
        std::fs::read_to_string(sync_dir.join("sync_meta.json")).unwrap(),
        "{\"schema_version\": 7}",
        "clearing the remote must leave sync_meta.json untouched"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_clear_sync_remote_without_origin_runs_no_remove() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .times(1)
        .returning(|_, _| Err(GitError::GitNotFound));
    mock_git.expect_remove_remote().times(0);

    let state = AppState::new(
        db,
        SyncManager::with_sync_dir(mock_git, sync_dir),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    let app = routes::create_router(state, false);

    let (status, body) = post_remote(&app, json!({ "remote_url": null })).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body["data"]["remote_url"].is_null(),
        "an already remote-less sync must answer with a null remote_url, got: {}",
        body
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_clear_and_set_sync_remote_while_disabled_return_conflict() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");
    std::fs::create_dir_all(sync_dir.join(".git")).unwrap();

    let app = test_app(db, &sync_dir);

    let (disable_status, _) = post_sync_action(&app, "/api/v1/sync/disable").await;
    assert_eq!(disable_status, StatusCode::OK);

    let (clear_status, clear_body) = post_remote(&app, json!({ "remote_url": null })).await;

    assert_eq!(clear_status, StatusCode::CONFLICT);
    let clear_error = clear_body["error"].as_str().unwrap_or_default();
    assert!(
        clear_error.contains("disabled"),
        "the clear conflict must name the disabled state, got: {}",
        clear_body
    );

    let (set_status, _) =
        post_remote(&app, json!({ "remote_url": "git@example.com:new.git" })).await;

    assert_eq!(set_status, StatusCode::CONFLICT);
}
