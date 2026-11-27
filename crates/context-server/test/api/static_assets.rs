use std::path::PathBuf;

use crate::api::static_assets::*;
use axum::http::{StatusCode, header};

/// Asset paths the emitted `index.html` references, as paths inside `dist/`.
///
/// A release ships the emitted document, and Trunk rewrites every vendored and
/// hashed reference into it, so that document is the only durable contract: the
/// paths carry build-time hashes that no source file names.
fn referenced_asset_paths() -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist/index.html");
    let html = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "failed to read {}: {error} (run `trunk build --release` first)",
            path.display()
        )
    });

    let mut paths: Vec<String> = ["src=", "href="]
        .into_iter()
        .flat_map(|marker| attribute_values(&html, marker))
        .filter(|value| value.starts_with('/'))
        .map(|value| value.trim_start_matches('/').to_string())
        .collect();
    paths.sort();
    paths.dedup();

    paths
}

/// Values of every `marker` attribute in `html`, quoted or bare.
fn attribute_values(html: &str, marker: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = html;

    while let Some(index) = rest.find(marker) {
        rest = &rest[index + marker.len()..];
        let unquoted = rest.trim_start_matches(['"', '\'']);
        let value: String = unquoted
            .chars()
            .take_while(|character| !matches!(character, ' ' | '>' | '"' | '\''))
            .collect();
        values.push(value);
    }

    values
}

/// Serve `path` the way the embedded router does.
async fn serve(path: &str) -> axum::response::Response {
    let uri = format!("/{path}")
        .parse()
        .expect("asset path parses as a URI");

    serve_frontend(uri).await
}

fn content_type(response: &axum::response::Response) -> String {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn cache_control(response: &axum::response::Response) -> String {
    response
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

const ONE_YEAR: &str = "public, max-age=31536000";

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

#[tokio::test]
async fn every_asset_the_emitted_index_references_is_embedded() {
    let paths = referenced_asset_paths();

    assert!(
        paths.len() >= 8,
        "the emitted index should reference its assets, got: {paths:?}"
    );

    for path in &paths {
        assert!(
            FrontendAssets::get(path).is_some(),
            "the embed must carry {path}, referenced by dist/index.html"
        );
    }
}

#[tokio::test]
async fn the_vendored_scripts_and_stylesheet_are_embedded() {
    for path in ["mermaid.min.js", "mermaid-bridge.js", "highlight.min.js"] {
        assert!(
            FrontendAssets::get(path).is_some(),
            "the vendored script {path} must be embedded"
        );
    }

    assert!(
        FrontendAssets::get("hljs-theme.css").is_some(),
        "the vendored highlight.js stylesheet must be embedded"
    );
}

#[tokio::test]
async fn served_scripts_answer_javascript_and_stylesheets_answer_css() {
    let paths = referenced_asset_paths();
    let script = paths
        .iter()
        .find(|path| path.contains("mermaid.min") && path.ends_with(".js"))
        .expect("the emitted index references the vendored mermaid script");
    let stylesheet = paths
        .iter()
        .find(|path| path.contains("hljs-theme") && path.ends_with(".css"))
        .expect("the emitted index references the vendored highlight.js theme");

    let script_response = serve(script).await;
    assert_eq!(
        script_response.status(),
        StatusCode::OK,
        "{script} must be served, not the SPA fallback"
    );
    assert!(
        content_type(&script_response).ends_with("javascript"),
        "{script} must answer javascript, got: {}",
        content_type(&script_response)
    );

    let stylesheet_response = serve(stylesheet).await;
    assert_eq!(
        stylesheet_response.status(),
        StatusCode::OK,
        "{stylesheet} must be served, not the SPA fallback"
    );
    assert!(
        content_type(&stylesheet_response).ends_with("text/css"),
        "{stylesheet} must answer text/css, got: {}",
        content_type(&stylesheet_response)
    );
}

#[tokio::test]
async fn the_root_document_answers_no_cache() {
    let response = serve("").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(cache_control(&response), "no-cache");
    assert!(
        content_type(&response).starts_with("text/html"),
        "the root document must answer text/html, got: {}",
        content_type(&response)
    );
}

#[tokio::test]
async fn the_index_document_answers_no_cache() {
    let response = serve("index.html").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(cache_control(&response), "no-cache");
}

#[tokio::test]
async fn unhashed_embedded_assets_answer_no_cache() {
    for (path, expected_type) in [
        ("mermaid.min.js", "javascript"),
        ("mermaid-bridge.js", "javascript"),
        ("highlight.min.js", "javascript"),
        ("hljs-theme.css", "text/css"),
    ] {
        let response = serve(path).await;

        assert_eq!(response.status(), StatusCode::OK, "{path} must be embedded");
        assert_eq!(
            cache_control(&response),
            "no-cache",
            "{path} is not content-hashed, so a change must not stay stale"
        );
        assert!(
            content_type(&response).ends_with(expected_type),
            "{path} must answer {expected_type}, got: {}",
            content_type(&response)
        );
    }
}

#[tokio::test]
async fn content_hashed_assets_answer_one_year() {
    let unhashed = [
        "mermaid.min.js",
        "mermaid-bridge.js",
        "highlight.min.js",
        "hljs-theme.css",
    ];
    let hashed: Vec<String> = referenced_asset_paths()
        .into_iter()
        .filter(|path| !path.starts_with("snippets/"))
        .filter(|path| !unhashed.contains(&path.as_str()))
        .collect();

    assert!(
        hashed.len() >= 5,
        "the emitted index references hashed assets, got: {hashed:?}"
    );

    for path in hashed {
        let response = serve(&path).await;

        assert_eq!(response.status(), StatusCode::OK, "{path} must be served");
        assert_eq!(
            cache_control(&response),
            ONE_YEAR,
            "{path} carries a build-time hash and must be cached for a year"
        );
    }
}
