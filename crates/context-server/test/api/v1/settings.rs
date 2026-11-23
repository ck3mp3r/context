//! Co-located tests for the settings REST handlers.
//!
//! Covers the GET/PUT contract: the triple `{values, defaults, running}`, the
//! write to `get_config_path()`, and the validation that must reject bad input
//! without touching the file.

use std::path::PathBuf;

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

use crate::api::{AppState, Config, RunningSettings, routes};
use context_core::settings::{StoredSettings, load};
use context_core::{Database, clear_base_path, get_config_path, set_base_path};
use context_db::SqliteDatabase;
use context_sync::{MockGitOps, SyncManager};

/// Running settings a test server reports; distinct from the file values.
fn test_running() -> RunningSettings {
    RunningSettings {
        host: "127.0.0.1".parse().expect("host parses"),
        port: 4000,
        verbosity: 2,
        enable_docs: true,
        host_fixed: false,
    }
}

async fn test_app(running: RunningSettings) -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    let temp_dir = TempDir::new().expect("Failed to create test temp dir");
    let state = AppState::new(
        db,
        SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    )
    .with_running(running);
    routes::create_router(state, false)
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    serde_json::from_slice(&body).expect("body is JSON")
}

async fn get_settings(app: &axum::Router) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/settings")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("request succeeds");
    let status = response.status();
    (status, json_body(response).await)
}

async fn put_settings(app: &axum::Router, values: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/settings")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&values).expect("values serialize"),
                ))
                .expect("request builds"),
        )
        .await
        .expect("request succeeds");
    let status = response.status();
    (status, json_body(response).await)
}

/// Base path this test alone owns; the config file lands under it.
fn base_path() -> TempDir {
    let base = TempDir::new().expect("Failed to create base temp dir");
    set_base_path(base.path().to_path_buf());
    base
}

#[cfg(debug_assertions)]
const DEFAULT_PORT: u16 = 3738;
#[cfg(not(debug_assertions))]
const DEFAULT_PORT: u16 = 3737;

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn get_settings_reports_values_defaults_and_running() {
    let base = base_path();
    let app = test_app(test_running()).await;

    let (status, body) = get_settings(&app).await;
    clear_base_path();

    assert_eq!(status, StatusCode::OK);

    let values = &body["values"];
    for field in [
        "host",
        "port",
        "verbosity",
        "enable_docs",
        "skills_dir",
        "start_at_login",
    ] {
        assert!(
            values.get(field).is_some(),
            "values must carry {field}, got: {values}"
        );
        assert!(
            values[field].is_null(),
            "an absent file leaves {field} empty, got: {values}"
        );
    }

    let defaults = &body["defaults"];
    assert_eq!(defaults["host"], "0.0.0.0");
    assert_eq!(defaults["port"], DEFAULT_PORT);
    assert_eq!(defaults["verbosity"], 0);
    assert_eq!(defaults["enable_docs"], false);
    assert_eq!(defaults["start_at_login"], true);

    let running = &body["running"];
    assert_eq!(running["host"], "127.0.0.1");
    assert_eq!(running["port"], 4000);
    assert_eq!(running["verbosity"], 2);
    assert_eq!(running["enable_docs"], true);
    assert!(
        running["skills_dir"].is_string(),
        "running must name the skills dir, got: {running}"
    );

    assert!(
        !get_config_path().starts_with(base.path()),
        "clearing the base path must restore the real config root"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn get_settings_reports_a_fixed_host() {
    let _base = base_path();
    let app = test_app(RunningSettings {
        host_fixed: true,
        ..test_running()
    })
    .await;

    let (status, body) = get_settings(&app).await;
    clear_base_path();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["running"]["host_fixed"], true,
        "running must report the fixed host, got: {}",
        body["running"]
    );
    assert_eq!(
        body["defaults"]["host"], "0.0.0.0",
        "defaults must stay the built-in settings, got: {}",
        body["defaults"]
    );
}

#[test]
fn running_settings_copy_the_config_fields() {
    let config = Config {
        host: "10.0.0.5".parse().expect("host parses"),
        port: 5050,
        verbosity: 3,
        enable_docs: true,
        skills_dir: PathBuf::from("/tmp/skills"),
        host_fixed: true,
    };

    let running = RunningSettings::from_config(&config);

    assert!(running.host_fixed, "host_fixed must come from the config");
    assert_eq!(running.host, config.host);
    assert_eq!(running.port, config.port);
    assert_eq!(running.verbosity, config.verbosity);
    assert_eq!(running.enable_docs, config.enable_docs);
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn put_settings_writes_the_file_and_keeps_running_untouched() {
    let _base = base_path();
    let app = test_app(test_running()).await;

    let (status, body) = put_settings(&app, json!({ "port": 4040, "verbosity": 1 })).await;
    let path = get_config_path();
    let written = load(&path).expect("the settings file loads");
    clear_base_path();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["values"]["port"], 4040);
    assert_eq!(body["values"]["verbosity"], 1);
    assert!(
        body["values"]["host"].is_null(),
        "a field left out of the body stays empty, got: {}",
        body["values"]
    );
    assert_eq!(body["running"]["port"], 4000);

    assert_eq!(
        written,
        StoredSettings {
            port: Some(4040),
            verbosity: Some(1),
            ..StoredSettings::default()
        },
        "the file must carry exactly the submitted values"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn put_settings_rejects_port_zero_without_writing() {
    let _base = base_path();
    let app = test_app(test_running()).await;

    let (status, body) = put_settings(&app, json!({ "port": 0 })).await;
    let path = get_config_path();
    let exists = path.exists();
    clear_base_path();

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let message = body["error"].as_str().expect("error envelope carries text");
    assert!(
        message.contains("port"),
        "the message must name the field, got: {message}"
    );
    assert!(!exists, "a rejected write must not create {path:?}");
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn put_settings_rejects_verbosity_above_three_without_writing() {
    let _base = base_path();
    let app = test_app(test_running()).await;

    let (status, body) = put_settings(&app, json!({ "verbosity": 4 })).await;
    let path = get_config_path();
    let exists = path.exists();
    clear_base_path();

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let message = body["error"].as_str().expect("error envelope carries text");
    assert!(
        message.contains("verbosity"),
        "the message must name the field, got: {message}"
    );
    assert!(!exists, "a rejected write must not create {path:?}");
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn put_settings_rejects_an_invalid_host_without_writing() {
    let _base = base_path();
    let app = test_app(test_running()).await;

    let (status, body) = put_settings(&app, json!({ "host": "not-an-ip" })).await;
    let path = get_config_path();
    let exists = path.exists();
    clear_base_path();

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let message = body["error"].as_str().expect("error envelope carries text");
    assert!(
        message.contains("host"),
        "the message must name the field, got: {message}"
    );
    assert!(!exists, "a rejected write must not create {path:?}");
}

#[test]
fn settings_paths_are_registered_in_the_openapi_document() {
    let document = routes::ApiDoc::openapi();

    assert!(
        document.paths.paths.contains_key("/api/v1/settings"),
        "OpenAPI paths must include /api/v1/settings, got: {:?}",
        document.paths.paths.keys().collect::<Vec<_>>()
    );
}
