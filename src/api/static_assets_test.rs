use crate::api::static_assets::*;
use axum::http::StatusCode;

#[tokio::test]
async fn test_root_serves_index_html() {
    let uri = "/".parse().unwrap();
    let response = serve_frontend(uri).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_routes_return_404() {
    let uri = "/api/v1/projects".parse().unwrap();
    let response = serve_frontend(uri).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_mcp_routes_return_404() {
    let uri = "/mcp".parse().unwrap();
    let response = serve_frontend(uri).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_docs_routes_return_404() {
    let uri = "/docs".parse().unwrap();
    let response = serve_frontend(uri).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_spa_fallback() {
    let uri = "/notes".parse().unwrap();
    let response = serve_frontend(uri).await;
    // Should serve index.html (200) for SPA routing
    assert_eq!(response.status(), StatusCode::OK);
}
