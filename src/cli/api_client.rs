use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use std::env;

use crate::cli::error::CliResult;

/// API client for communicating with the c5t REST API
pub struct ApiClient {
    base_url: String,
    client: Client,
}

impl ApiClient {
    /// Create a new API client
    ///
    /// Priority for base URL:
    /// 1. Explicit `api_url` parameter
    /// 2. C5T_API_URL environment variable
    /// 3. Default: http://localhost:3737
    pub fn new(api_url: Option<String>) -> Self {
        let base_url = api_url
            .or_else(|| env::var("C5T_API_URL").ok())
            .unwrap_or_else(|| "http://localhost:3737".to_string());

        Self {
            base_url,
            client: Client::new(),
        }
    }

    /// Get the base URL being used
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Create a GET request builder
    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.client.get(&url)
    }

    /// Create a POST request builder
    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.client.post(&url)
    }

    /// Create a PATCH request builder
    pub fn patch(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.client.patch(&url)
    }

    /// Create a DELETE request builder
    pub fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.client.delete(&url)
    }

    /// Handle API response with standardized error handling
    ///
    /// Returns the deserialized response body on success,
    /// or a CliError::ApiError on non-success status codes.
    pub async fn handle_response<T: DeserializeOwned>(response: Response) -> CliResult<T> {
        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| crate::cli::error::CliError::InvalidResponse {
                    message: e.to_string(),
                })
        } else {
            let status = response.status().as_u16();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(crate::cli::error::CliError::ApiError {
                status,
                message: error_text,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
