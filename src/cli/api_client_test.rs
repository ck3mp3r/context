use crate::cli::api_client::*;

// Initialize crypto provider once for all tests
fn init_crypto() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[test]
fn test_new_with_explicit_url() {
    init_crypto();
    let client = ApiClient::new(Some("http://custom:8080".to_string()));
    assert_eq!(client.base_url(), "http://custom:8080");
}

#[test]
fn test_new_with_default() {
    init_crypto();
    let client = ApiClient::new(None);
    // When no URL provided and no env var, defaults to localhost:3737
    // Note: actual value depends on C5T_API_URL env var if set
    assert!(!client.base_url().is_empty());
}

#[test]
fn test_explicit_url_is_used() {
    init_crypto();
    let client = ApiClient::new(Some("http://explicit:7777".to_string()));
    assert_eq!(client.base_url(), "http://explicit:7777");
}

#[tokio::test]
async fn test_get_method_exists() {
    init_crypto();
    let client = ApiClient::new(None);
    // Test that get() method exists and returns RequestBuilder
    let _builder = client.get("/api/v1/test");
}

#[tokio::test]
async fn test_post_method_exists() {
    init_crypto();
    let client = ApiClient::new(None);
    let _builder = client.post("/api/v1/test");
}

#[tokio::test]
async fn test_patch_method_exists() {
    init_crypto();
    let client = ApiClient::new(None);
    let _builder = client.patch("/api/v1/test");
}

#[tokio::test]
async fn test_delete_method_exists() {
    init_crypto();
    let client = ApiClient::new(None);
    let _builder = client.delete("/api/v1/test");
}

// Note: handle_response is tested via integration tests with real API
