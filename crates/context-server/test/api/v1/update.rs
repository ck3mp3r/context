//! Tests for the update endpoints.
//!
//! Covers the GET/POST contract: the persisted state becomes the response, and
//! the two triggers forward to the injected flow and map its errors.

use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use serial_test::serial;
use tempfile::TempDir;
use tower::ServiceExt;
use utoipa::OpenApi;

use crate::api::notifier::ChangeNotifier;
use crate::api::{AppState, routes};
use context_core::update::{
    UpdateFlow, UpdateState, UpdateStateError, UpdateStateResult, UpdateStatus, update_state_path,
    write,
};
use context_core::{Database, clear_base_path, set_base_path};
use context_db::SqliteDatabase;
use context_sync::{MockGitOps, SyncManager};

/// Outcome a fake flow answers with.
#[derive(Debug, Clone, Copy, Default)]
enum Answer {
    #[default]
    Ok,
    Busy,
    Unavailable,
}

impl Answer {
    fn outcome(self) -> UpdateStateResult<()> {
        match self {
            Self::Ok => Ok(()),
            Self::Busy => Err(UpdateStateError::Busy),
            Self::Unavailable => Err(UpdateStateError::Unavailable),
        }
    }
}

/// Flow that records every call and answers with a fixed outcome.
#[derive(Debug, Clone, Default)]
struct FakeFlow {
    answer: Answer,
    calls: Arc<Mutex<Vec<String>>>,
}

impl FakeFlow {
    fn with_answer(answer: Answer) -> Self {
        Self {
            answer,
            calls: Arc::default(),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls lock").clone()
    }
}

impl UpdateFlow for FakeFlow {
    fn check(&self, manual: bool) -> UpdateStateResult<()> {
        self.calls
            .lock()
            .expect("calls lock")
            .push(format!("check manual={manual}"));
        self.answer.outcome()
    }

    fn install(&self) -> UpdateStateResult<()> {
        self.calls
            .lock()
            .expect("calls lock")
            .push("install".to_string());
        self.answer.outcome()
    }
}

async fn test_app(flow: FakeFlow) -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    let temp_dir = TempDir::new().expect("Failed to create test temp dir");
    let state = AppState::new(
        db,
        SyncManager::new(MockGitOps::new()),
        ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    )
    .with_update(flow);

    routes::create_router(state, false)
}

async fn call(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(path);
    let request = match body {
        Some(values) => builder
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&values).expect("body serializes"),
            ))
            .expect("request builds"),
        None => builder.body(Body::empty()).expect("request builds"),
    };

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("request succeeds");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("body is JSON")
    };

    (status, value)
}

/// Base path this test alone owns; the state file lands under it.
fn base_path() -> TempDir {
    let base = TempDir::new().expect("Failed to create base temp dir");
    set_base_path(base.path().to_path_buf());
    base
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn get_update_status_reports_the_default_state() {
    let _base = base_path();
    let app = test_app(FakeFlow::default()).await;

    let (status, body) = call(&app, "GET", "/api/v1/update/status", None).await;
    clear_base_path();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["current_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body["status"], "idle");
    assert!(
        body.get("candidate_version").is_none(),
        "an absent candidate must be omitted, got: {body}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn get_update_status_reports_the_written_state() {
    let _base = base_path();
    let state = UpdateState {
        status: UpdateStatus::Available,
        current_version: "0.8.0".to_string(),
        candidate_version: Some("0.9.0".to_string()),
        candidate_tag: Some("v0.9.0".to_string()),
        candidate_notes: Some("Fixes and features".to_string()),
        candidate_is_prerelease: Some(false),
        message: None,
        updated_at: Some("2026-01-01T00:00:00Z".to_string()),
    };
    write(&update_state_path(), &state).expect("state file writes");
    let app = test_app(FakeFlow::default()).await;

    let (status, body) = call(&app, "GET", "/api/v1/update/status", None).await;
    clear_base_path();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "available");
    assert_eq!(body["current_version"], "0.8.0");
    assert_eq!(body["candidate_version"], "0.9.0");
    assert_eq!(body["candidate_tag"], "v0.9.0");
    assert_eq!(body["candidate_notes"], "Fixes and features");
    assert_eq!(body["candidate_is_prerelease"], false);
    assert_eq!(body["updated_at"], "2026-01-01T00:00:00Z");
    assert!(
        body.get("message").is_none(),
        "an absent message must be omitted, got: {body}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn get_update_status_reports_the_default_state_for_an_unreadable_file() {
    let _base = base_path();
    std::fs::create_dir_all(
        update_state_path()
            .parent()
            .expect("state path has a parent"),
    )
    .expect("config dir creates");
    std::fs::write(update_state_path(), "{ not json").expect("corrupt state file writes");
    let app = test_app(FakeFlow::default()).await;

    let (status, body) = call(&app, "GET", "/api/v1/update/status", None).await;
    clear_base_path();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "idle");
}

#[tokio::test(flavor = "multi_thread")]
async fn check_for_update_forwards_the_manual_flag() {
    let flow = FakeFlow::default();
    let app = test_app(flow.clone()).await;

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/update/check",
        Some(json!({ "manual": true })),
    )
    .await;

    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(flow.calls(), vec!["check manual=true".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn install_update_returns_accepted() {
    let flow = FakeFlow::default();
    let app = test_app(flow.clone()).await;

    let (status, _) = call(&app, "POST", "/api/v1/update/install", None).await;

    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(flow.calls(), vec!["install".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_busy_flow_maps_to_conflict() {
    let app = test_app(FakeFlow::with_answer(Answer::Busy)).await;

    let (check_status, check_body) =
        call(&app, "POST", "/api/v1/update/check", Some(json!({}))).await;
    let (install_status, install_body) = call(&app, "POST", "/api/v1/update/install", None).await;

    assert_eq!(check_status, StatusCode::CONFLICT);
    assert_eq!(install_status, StatusCode::CONFLICT);
    for body in [check_body, install_body] {
        let message = body["error"].as_str().expect("error envelope carries text");
        assert!(
            message.contains("already running"),
            "the message must name the conflict, got: {message}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unavailable_flow_maps_to_service_unavailable() {
    let app = test_app(FakeFlow::with_answer(Answer::Unavailable)).await;

    let (check_status, check_body) =
        call(&app, "POST", "/api/v1/update/check", Some(json!({}))).await;
    let (install_status, install_body) = call(&app, "POST", "/api/v1/update/install", None).await;

    assert_eq!(check_status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(install_status, StatusCode::SERVICE_UNAVAILABLE);
    for body in [check_body, install_body] {
        let message = body["error"].as_str().expect("error envelope carries text");
        assert!(
            message.contains("No update flow"),
            "the message must name the missing flow, got: {message}"
        );
    }
}

#[test]
fn update_paths_are_registered_in_the_openapi_document() {
    let document = routes::ApiDoc::openapi();

    for path in [
        "/api/v1/update/status",
        "/api/v1/update/check",
        "/api/v1/update/install",
    ] {
        assert!(
            document.paths.paths.contains_key(path),
            "OpenAPI paths must include {path}, got: {:?}",
            document.paths.paths.keys().collect::<Vec<_>>()
        );
    }
}
