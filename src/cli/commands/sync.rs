//! Sync command implementations.

use crate::cli::api_client::ApiClient;
use crate::cli::error::{CliError, CliResult};
use serde::{Deserialize, Serialize};

/// Request to initialize sync
#[derive(Debug, Serialize)]
struct InitSyncRequest {
    remote_url: Option<String>,
}

/// Request to export sync
#[derive(Debug, Serialize)]
struct ExportSyncRequest {
    message: Option<String>,
}

/// Response from sync operations
#[derive(Debug, Deserialize)]
struct SyncResponse {
    status: String,
    message: String,
    data: Option<serde_json::Value>,
}

/// Initialize sync repository
pub async fn init(api_client: &ApiClient, remote_url: Option<String>) -> CliResult<String> {
    let url = format!("{}/v1/sync/init", api_client.base_url());
    let req = InitSyncRequest { remote_url };

    let response = reqwest::Client::new()
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| CliError::ConnectionFailed { source: e })?;

    let status_code = response.status().as_u16();
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(CliError::ApiError {
            status: status_code,
            message: format!("Failed to initialize sync: {}", error_text),
        });
    }

    let sync_response: SyncResponse =
        response
            .json()
            .await
            .map_err(|e| CliError::InvalidResponse {
                message: e.to_string(),
            })?;

    Ok(format!(
        "{}\n{}",
        sync_response.message,
        serde_json::to_string_pretty(&sync_response.data).unwrap_or_default()
    ))
}

/// Export database to sync
pub async fn export(api_client: &ApiClient, message: Option<String>) -> CliResult<String> {
    let url = format!("{}/v1/sync/export", api_client.base_url());
    let req = ExportSyncRequest { message };

    let response = reqwest::Client::new()
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| CliError::ConnectionFailed { source: e })?;

    let status_code = response.status().as_u16();
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(CliError::ApiError {
            status: status_code,
            message: format!("Failed to export: {}", error_text),
        });
    }

    let sync_response: SyncResponse =
        response
            .json()
            .await
            .map_err(|e| CliError::InvalidResponse {
                message: e.to_string(),
            })?;

    Ok(format!(
        "{}\n{}",
        sync_response.message,
        serde_json::to_string_pretty(&sync_response.data).unwrap_or_default()
    ))
}

/// Import from sync to database
pub async fn import(api_client: &ApiClient) -> CliResult<String> {
    let url = format!("{}/v1/sync/import", api_client.base_url());

    let response = reqwest::Client::new()
        .post(&url)
        .send()
        .await
        .map_err(|e| CliError::ConnectionFailed { source: e })?;

    let status_code = response.status().as_u16();
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(CliError::ApiError {
            status: status_code,
            message: format!("Failed to import: {}", error_text),
        });
    }

    let sync_response: SyncResponse =
        response
            .json()
            .await
            .map_err(|e| CliError::InvalidResponse {
                message: e.to_string(),
            })?;

    Ok(format!(
        "{}\n{}",
        sync_response.message,
        serde_json::to_string_pretty(&sync_response.data).unwrap_or_default()
    ))
}

/// Get sync status
pub async fn status(api_client: &ApiClient) -> CliResult<String> {
    let url = format!("{}/v1/sync/status", api_client.base_url());

    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| CliError::ConnectionFailed { source: e })?;

    let status_code = response.status().as_u16();
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(CliError::ApiError {
            status: status_code,
            message: format!("Failed to get status: {}", error_text),
        });
    }

    let sync_response: SyncResponse =
        response
            .json()
            .await
            .map_err(|e| CliError::InvalidResponse {
                message: e.to_string(),
            })?;

    Ok(serde_json::to_string_pretty(&sync_response.data).unwrap_or_default())
}
