//! Integration tests for WebSocket handler.
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use context_server::api::{AppState, routes};
use context_sync::{MockGitOps, SyncManager};
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread")]
async fn test_websocket_route_exists() {
    // Create test app
    let db = common::setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_manager = SyncManager::new(MockGitOps::new());
    let notifier = context_server::api::notifier::ChangeNotifier::new();

    let state = AppState::new(db, sync_manager, notifier, temp_dir.path().join("skills"));
    let app = routes::create_router(state, false);

    // Create WebSocket upgrade request
    let request = Request::builder()
        .uri("/ws")
        .header("upgrade", "websocket")
        .header("connection", "upgrade")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("sec-websocket-version", "13")
        .body(Body::empty())
        .unwrap();

    // Send request
    let response = app.oneshot(request).await.unwrap();

    // Should accept upgrade (426 Upgrade Required means handler exists but needs actual connection)
    // or 101 if upgrade completes (depends on test harness)
    assert!(
        response.status() == StatusCode::SWITCHING_PROTOCOLS
            || response.status() == StatusCode::UPGRADE_REQUIRED
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_websocket_rejects_non_upgrade_requests() {
    // Create test app
    let db = common::setup_db().await;
    let temp_dir = TempDir::new().unwrap();
    let sync_manager = SyncManager::new(MockGitOps::new());
    let notifier = context_server::api::notifier::ChangeNotifier::new();

    let state = AppState::new(db, sync_manager, notifier, temp_dir.path().join("skills"));
    let app = routes::create_router(state, false);

    // Create regular GET request (no WebSocket headers)
    let request = Request::builder().uri("/ws").body(Body::empty()).unwrap();

    // Send request
    let response = app.oneshot(request).await.unwrap();

    // Should reject with 400 or 405
    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::METHOD_NOT_ALLOWED
    );
}
