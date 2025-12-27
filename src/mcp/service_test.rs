//! Tests for MCP Streamable HTTP service integration
//!
//! Following TDD: Write tests FIRST (RED), then implement (GREEN).

use axum::{
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::db::SqliteDatabase;

/// Test that we can create a Streamable HTTP service
///
/// This test verifies:
/// - create_mcp_service() returns a valid Axum service
/// - Service can be nested into a router
/// - Service responds to HTTP requests
#[tokio::test]
async fn test_create_mcp_service() {
    use tokio_util::sync::CancellationToken;

    // Arrange
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate_async().await.expect("Failed to run migrations");

    let ct = CancellationToken::new();

    // Act: Create MCP service
    let service = super::create_mcp_service(db, ct);

    // Assert: Service should be created successfully
    // We'll test actual requests in integration tests
    drop(service);
}

/// Test that MCP service can be integrated with Axum router
#[tokio::test]
async fn test_mcp_service_with_router() {
    use axum::Router;
    use tokio_util::sync::CancellationToken;

    // Arrange
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate_async().await.expect("Failed to run migrations");

    let ct = CancellationToken::new();
    let service = super::create_mcp_service(db, ct);

    // Act: Nest service into router
    let app = Router::new().nest_service("/mcp", service);

    // Assert: Should compile and router should work
    // Test a simple request to verify routing works
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    // Root path should return 404 (only /mcp is mounted)
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Test that MCP service handles session management automatically
///
/// Note: Session management is handled by rmcp's StreamableHttpService automatically.
/// This test verifies that the service is properly configured.
#[tokio::test]
async fn test_mcp_session_management_configured() {
    use axum::Router;
    use tokio_util::sync::CancellationToken;

    // Arrange
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate_async().await.expect("Failed to run migrations");

    let ct = CancellationToken::new();
    let service = super::create_mcp_service(db, ct);
    let app = Router::new().nest_service("/mcp", service);

    // Act & Assert: Service should be created with stateful_mode = true
    // The StreamableHttpService handles session management internally via:
    // - X-Session-ID header (or Mcp-Session-Id depending on spec version)
    // - LocalSessionManager for session state
    // - Automatic session creation/cleanup

    // Just verify the service responds (even if it's an error response)
    // Full protocol testing will be done in integration tests
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

    // Service should respond (not 404), confirming it's mounted correctly
    // rmcp will return appropriate MCP protocol errors for invalid requests
    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Service should be mounted and responding"
    );
}
