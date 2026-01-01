//! Embedded frontend assets for production builds.
//!
//! In release mode: Assets are embedded into the binary at compile time.
//! In debug mode: rust-embed reads from filesystem (dist/) at runtime.

use axum::{
    body::Body,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

/// Embedded frontend assets (WASM, JS, CSS, HTML, etc.)
///
/// Folder points to Trunk's output directory.
/// In debug: reads from dist/ at runtime (Trunk dev server preferred)
/// In release: embedded at compile time with compression
#[derive(RustEmbed)]
#[folder = "dist/"]
#[include = "*.html"]
#[include = "*.js"]
#[include = "*.wasm"]
#[include = "*.css"]
#[include = "snippets/**/*"]
struct FrontendAssets;

/// Serve embedded frontend assets with SPA fallback routing.
///
/// Routing logic:
/// 1. Skip if path starts with api/, mcp/, or docs (handled elsewhere)
/// 2. Try exact file match (e.g., /style.css, /app.wasm)
/// 3. Fallback to index.html for SPA routing (e.g., /notes, /projects)
/// 4. Return 500 if index.html not found (should never happen)
pub async fn serve_frontend(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Skip API routes (they're handled by other routers)
    if path.starts_with("api/") || path.starts_with("mcp") || path.starts_with("docs") {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
    }

    // Root path → index.html
    let asset_path = if path.is_empty() { "index.html" } else { path };

    // Try exact match first
    match FrontendAssets::get(asset_path) {
        Some(content) => {
            let mime = mime_guess::from_path(asset_path).first_or_octet_stream();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, "public, max-age=31536000") // 1 year for hashed assets
                .body(Body::from(content.data))
                .unwrap()
        }
        // SPA fallback: serve index.html for client-side routing
        None => match FrontendAssets::get("index.html") {
            Some(index) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html")
                .header(header::CACHE_CONTROL, "no-cache") // index.html should not be cached
                .body(Body::from(index.data))
                .unwrap(),
            None => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(
                    "Frontend assets not found. Run 'trunk build --release' first.",
                ))
                .unwrap(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
