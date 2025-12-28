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

    let mut output = String::new();
    output.push_str(&format!("✓ {}\n\n", sync_response.message));

    if let Some(data) = &sync_response.data {
        if let Some(sync_dir) = data.get("sync_dir").and_then(|v| v.as_str()) {
            output.push_str(&format!("Sync directory: {}\n", sync_dir));
        }
        if let Some(remote) = data.get("remote_url").and_then(|v| v.as_str()) {
            output.push_str(&format!("Remote URL:     {}\n", remote));
        }
    }

    Ok(output)
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

    let mut output = String::new();
    output.push_str(&format!("✓ {}\n\n", sync_response.message));

    if let Some(data) = &sync_response.data {
        if let Some(exported) = data.get("exported") {
            output.push_str("Exported:\n");
            output.push_str(&format!(
                "  Repos:      {}\n",
                exported.get("repos").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Projects:   {}\n",
                exported
                    .get("projects")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Task Lists: {}\n",
                exported
                    .get("task_lists")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Tasks:      {}\n",
                exported.get("tasks").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Notes:      {}\n",
                exported.get("notes").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Total:      {}\n",
                exported.get("total").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
        }
    }

    Ok(output)
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

    let mut output = String::new();
    output.push_str(&format!("✓ {}\n\n", sync_response.message));

    if let Some(data) = &sync_response.data {
        if let Some(imported) = data.get("imported") {
            output.push_str("Imported:\n");
            output.push_str(&format!(
                "  Repos:      {}\n",
                imported.get("repos").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Projects:   {}\n",
                imported
                    .get("projects")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Task Lists: {}\n",
                imported
                    .get("task_lists")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Tasks:      {}\n",
                imported.get("tasks").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Notes:      {}\n",
                imported.get("notes").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Total:      {}\n",
                imported.get("total").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
        }
    }

    Ok(output)
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

    Ok(format_sync_status(&sync_response))
}

fn format_sync_status(response: &SyncResponse) -> String {
    let mut output = String::new();

    // Status message
    output.push_str(&format!("Status: {}\n", response.status));
    output.push_str(&format!("{}\n\n", response.message));

    // Parse the data field
    if let Some(data) = &response.data {
        let initialized = data
            .get("initialized")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !initialized {
            output.push_str("Sync not initialized. Run: c5t sync init\n");
            return output;
        }

        // Remote URL
        if let Some(remote) = data.get("remote_url").and_then(|v| v.as_str()) {
            output.push_str(&format!("Remote: {}\n", remote));
        } else {
            output.push_str("Remote: (not configured)\n");
        }

        // Git status
        if let Some(git) = data.get("git") {
            if let Some(clean) = git.get("clean").and_then(|v| v.as_bool()) {
                output.push_str(&format!(
                    "Git Status: {}\n",
                    if clean {
                        "Clean"
                    } else {
                        "Uncommitted changes"
                    }
                ));
                if !clean {
                    if let Some(status) = git.get("status").and_then(|v| v.as_str()) {
                        output.push_str(&format!("  {}\n", status.trim()));
                    }
                }
            }
        }

        output.push('\n');

        // Database counts
        if let Some(db) = data.get("database") {
            output.push_str("Database:\n");
            output.push_str(&format!(
                "  Repos:      {}\n",
                db.get("repos").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Projects:   {}\n",
                db.get("projects").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Task Lists: {}\n",
                db.get("task_lists").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Tasks:      {}\n",
                db.get("tasks").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Notes:      {}\n",
                db.get("notes").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Total:      {}\n",
                db.get("total").and_then(|v| v.as_u64()).unwrap_or(0)
            ));
            output.push('\n');
        }

        // Sync files counts
        if let Some(sync_files) = data.get("sync_files") {
            output.push_str("Sync Files:\n");
            output.push_str(&format!(
                "  Repos:      {}\n",
                sync_files
                    .get("repos")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Projects:   {}\n",
                sync_files
                    .get("projects")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Task Lists: {}\n",
                sync_files
                    .get("task_lists")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Tasks:      {}\n",
                sync_files
                    .get("tasks")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Notes:      {}\n",
                sync_files
                    .get("notes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ));
            output.push_str(&format!(
                "  Total:      {}\n",
                sync_files
                    .get("total")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ));
        }
    }

    output
}
