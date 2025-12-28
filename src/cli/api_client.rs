use reqwest::Client;
use std::env;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_explicit_url() {
        let client = ApiClient::new(Some("http://custom:8080".to_string()));
        assert_eq!(client.base_url(), "http://custom:8080");
    }

    #[test]
    fn test_new_with_env_var() {
        unsafe {
            env::set_var("C5T_API_URL", "http://from-env:9000");
        }
        let client = ApiClient::new(None);
        assert_eq!(client.base_url(), "http://from-env:9000");
        unsafe {
            env::remove_var("C5T_API_URL");
        }
    }

    #[test]
    fn test_new_with_default() {
        unsafe {
            env::remove_var("C5T_API_URL");
        }
        let client = ApiClient::new(None);
        assert_eq!(client.base_url(), "http://localhost:3737");
    }

    #[test]
    fn test_explicit_url_takes_precedence_over_env() {
        unsafe {
            env::set_var("C5T_API_URL", "http://from-env:9000");
        }
        let client = ApiClient::new(Some("http://explicit:7777".to_string()));
        assert_eq!(client.base_url(), "http://explicit:7777");
        unsafe {
            env::remove_var("C5T_API_URL");
        }
    }
}
