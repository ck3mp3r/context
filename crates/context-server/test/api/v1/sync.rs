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
