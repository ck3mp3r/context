//! Tests for MCP Streamable HTTP service integration
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use context_server::api::notifier::ChangeNotifier;
use tempfile::TempDir;

/// Test that we can create a Streamable HTTP service
#[tokio::test(flavor = "multi_thread")]
async fn test_create_mcp_service() {
    use tokio_util::sync::CancellationToken;

    let db = common::setup_db().await;

    let ct = CancellationToken::new();
    let temp_dir = TempDir::new().unwrap();

    let service = context_server::mcp::create_mcp_service(
        db,
        ChangeNotifier::new(),
        temp_dir.path().join("skills"),
        ct,
    );

    drop(service);
}

/// Test that MCP service can be integrated with Axum router
#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_service_with_router() {
    use axum::Router;
    use tokio_util::sync::CancellationToken;

    let db = common::setup_db().await;

    let ct = CancellationToken::new();
    let temp_dir = TempDir::new().unwrap();
    let service = context_server::mcp::create_mcp_service(
        db,
        ChangeNotifier::new(),
        temp_dir.path().join("skills"),
        ct,
    );

    let app = Router::new().nest_service("/mcp", service);

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Test that MCP service handles session management automatically
#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_session_management_configured() {
    use axum::Router;
    use tokio_util::sync::CancellationToken;

    let db = common::setup_db().await;

    let ct = CancellationToken::new();
    let temp_dir = TempDir::new().unwrap();
    let service = context_server::mcp::create_mcp_service(
        db,
        ChangeNotifier::new(),
        temp_dir.path().join("skills"),
        ct,
    );
    let app = Router::new().nest_service("/mcp", service);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mcp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Service should be mounted and responding"
    );
}
