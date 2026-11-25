use crate::cli::api_client::*;

use context_core::{clear_base_path, get_config_dir, get_config_path, set_base_path};
use serial_test::serial;
use tempfile::TempDir;

#[cfg(debug_assertions)]
const PROFILE_PORT: u16 = 3738;
#[cfg(not(debug_assertions))]
const PROFILE_PORT: u16 = 3737;

// Initialize crypto provider once for all tests
fn init_crypto() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Base path this test alone owns, with the settings file written.
fn base_with_config(contents: &str) -> TempDir {
    let base = TempDir::new().expect("Failed to create base temp dir");
    set_base_path(base.path().to_path_buf());

    let config_dir = get_config_dir();
    std::fs::create_dir_all(&config_dir).expect("config dir creates");
    std::fs::write(get_config_path(), contents).expect("settings file writes");

    base
}

/// `C5T_API_URL` state this test alone owns, restored when dropped.
struct EnvUrlGuard(Option<String>);

impl EnvUrlGuard {
    fn clear() -> Self {
        let previous = std::env::var("C5T_API_URL").ok();
        unsafe { std::env::remove_var("C5T_API_URL") };
        Self(previous)
    }

    fn set(value: &str) -> Self {
        let previous = std::env::var("C5T_API_URL").ok();
        unsafe { std::env::set_var("C5T_API_URL", value) };
        Self(previous)
    }
}

impl Drop for EnvUrlGuard {
    fn drop(&mut self) {
        match &self.0 {
            Some(value) => unsafe { std::env::set_var("C5T_API_URL", value) },
            None => unsafe { std::env::remove_var("C5T_API_URL") },
        }
    }
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

// =============================================================================
// Base URL precedence: --api-url > C5T_API_URL > stored port > profile port
// =============================================================================

#[test]
#[serial]
fn stored_port_is_the_base_url_when_no_override_is_set() {
    init_crypto();
    let _env = EnvUrlGuard::clear();
    let base = base_with_config("host = \"0.0.0.0\"\nport = 4000\n");

    let base_url = ApiClient::new(None).base_url().to_string();

    clear_base_path();
    drop(base);

    assert_eq!(base_url, "http://localhost:4000");
}

#[test]
#[serial]
fn explicit_url_beats_the_stored_port() {
    init_crypto();
    let _env = EnvUrlGuard::clear();
    let base = base_with_config("port = 4000\n");

    let base_url = ApiClient::new(Some("http://x:1".to_string()))
        .base_url()
        .to_string();

    clear_base_path();
    drop(base);

    assert_eq!(base_url, "http://x:1");
}

#[test]
#[serial]
fn env_url_beats_the_stored_port() {
    init_crypto();
    let _env = EnvUrlGuard::set("http://env:9");
    let base = base_with_config("port = 4000\n");

    let base_url = ApiClient::new(None).base_url().to_string();

    clear_base_path();
    drop(base);

    assert_eq!(base_url, "http://env:9");
}

#[test]
#[serial]
fn absent_settings_file_uses_the_profile_port() {
    init_crypto();
    let _env = EnvUrlGuard::clear();
    let base = TempDir::new().expect("Failed to create base temp dir");
    set_base_path(base.path().to_path_buf());

    let base_url = ApiClient::new(None).base_url().to_string();

    clear_base_path();
    drop(base);

    assert_eq!(base_url, format!("http://localhost:{PROFILE_PORT}"));
}

#[test]
#[serial]
fn settings_file_without_port_uses_the_profile_port() {
    init_crypto();
    let _env = EnvUrlGuard::clear();
    let base = base_with_config("verbosity = 2\n");

    let base_url = ApiClient::new(None).base_url().to_string();

    clear_base_path();
    drop(base);

    assert_eq!(base_url, format!("http://localhost:{PROFILE_PORT}"));
}

#[test]
#[serial]
fn corrupt_settings_file_uses_the_profile_port() {
    init_crypto();
    let _env = EnvUrlGuard::clear();
    let base = base_with_config("port = \"not-a-number\"\n");

    let base_url = ApiClient::new(None).base_url().to_string();

    clear_base_path();
    drop(base);

    assert_eq!(base_url, format!("http://localhost:{PROFILE_PORT}"));
}

#[test]
#[serial]
fn invalid_stored_port_uses_the_profile_port() {
    init_crypto();
    let _env = EnvUrlGuard::clear();
    let base = base_with_config("port = 0\n");

    let base_url = ApiClient::new(None).base_url().to_string();

    clear_base_path();
    drop(base);

    assert_eq!(base_url, format!("http://localhost:{PROFILE_PORT}"));
}
