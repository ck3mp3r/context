//! Sync command implementations.

use crate::cli::api_client::ApiClient;
use crate::cli::error::{CliError, CliResult};
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled, settings::Style};

/// Request to initialize sync
#[derive(Debug, Serialize)]
struct InitSyncRequest {
    remote_url: Option<String>,
}

/// Request to export sync
#[derive(Debug, Serialize)]
struct ExportSyncRequest {
    message: Option<String>,
    remote: bool,
}

/// Request to import sync
#[derive(Debug, Serialize)]
struct ImportSyncRequest {
    remote: bool,
}

/// Response from sync operations
#[derive(Debug, Deserialize)]
struct SyncResponse {
    message: String,
    data: Option<serde_json::Value>,
}

/// Initialize sync repository
pub async fn init(api_client: &ApiClient, remote_url: Option<String>) -> CliResult<String> {
    let req = InitSyncRequest { remote_url };

    let response = api_client
        .post("/api/v1/sync/init")
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

    // Check if this was a new creation (201) or already initialized (200)
    let icon = if status_code == 201 { "✓" } else { "ℹ" };
    output.push_str(&format!("{} {}\n\n", icon, sync_response.message));

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

#[derive(Tabled)]
struct SyncCountRow {
    #[tabled(rename = "Item")]
    item: String,
    #[tabled(rename = "Count")]
    count: String,
}

/// Export database to sync
pub async fn export(
    api_client: &ApiClient,
    message: Option<String>,
    remote: bool,
) -> CliResult<String> {
    let req = ExportSyncRequest { message, remote };

    let response = api_client
        .post("/api/v1/sync/export")
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

    if let Some(data) = &sync_response.data
        && let Some(exported) = data.get("exported")
    {
        let rows = vec![
            SyncCountRow {
                item: "Repos".to_string(),
                count: exported
                    .get("repos")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Projects".to_string(),
                count: exported
                    .get("projects")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Task Lists".to_string(),
                count: exported
                    .get("task_lists")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Tasks".to_string(),
                count: exported
                    .get("tasks")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Notes".to_string(),
                count: exported
                    .get("notes")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Skills".to_string(),
                count: exported
                    .get("skills")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Total".to_string(),
                count: exported
                    .get("total")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
        ];

        let mut table = Table::new(rows);
        table.with(Style::rounded());
        output.push_str(&table.to_string());
    }

    Ok(output)
}

/// Import from sync to database
pub async fn import(api_client: &ApiClient, remote: bool) -> CliResult<String> {
    let req = ImportSyncRequest { remote };

    let response = api_client
        .post("/api/v1/sync/import")
        .json(&req)
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

    if let Some(data) = &sync_response.data
        && let Some(imported) = data.get("imported")
    {
        let rows = vec![
            SyncCountRow {
                item: "Repos".to_string(),
                count: imported
                    .get("repos")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Projects".to_string(),
                count: imported
                    .get("projects")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Task Lists".to_string(),
                count: imported
                    .get("task_lists")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Tasks".to_string(),
                count: imported
                    .get("tasks")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Notes".to_string(),
                count: imported
                    .get("notes")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Skills".to_string(),
                count: imported
                    .get("skills")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
            SyncCountRow {
                item: "Total".to_string(),
                count: imported
                    .get("total")
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("0".to_string()),
            },
        ];

        let mut table = Table::new(rows);
        table.with(Style::rounded());
        output.push_str(&table.to_string());
    }

    Ok(output)
}

/// Get sync status
pub async fn status(api_client: &ApiClient) -> CliResult<String> {
    let response = api_client
        .get("/api/v1/sync/status")
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

#[derive(Tabled)]
struct SyncStatusRow {
    #[tabled(rename = "Item")]
    item: String,
    #[tabled(rename = "Database")]
    database: String,
    #[tabled(rename = "Sync Files")]
    sync_files: String,
}

fn format_sync_status(response: &SyncResponse) -> String {
    let mut output = String::new();

    // Parse the data field
    if let Some(data) = &response.data {
        let initialized = data
            .get("initialized")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !initialized {
            return "Sync not initialized. Run: c5t sync init\n".to_string();
        }

        // Remote URL and Git Status
        if let Some(remote) = data.get("remote_url").and_then(|v| v.as_str()) {
            output.push_str(&format!("Remote: {}\n", remote));
        }

        if let Some(git) = data.get("git")
            && let Some(clean) = git.get("clean").and_then(|v| v.as_bool())
        {
            output.push_str(&format!(
                "Status: {}\n\n",
                if clean {
                    "✓ Clean"
                } else {
                    "✗ Uncommitted changes"
                }
            ));
        }

        // Build table data
        let db = data.get("database");
        let sync = data.get("sync_files");

        let rows = vec![
            SyncStatusRow {
                item: "Repos".to_string(),
                database: db
                    .and_then(|d| d.get("repos"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
                sync_files: sync
                    .and_then(|s| s.get("repos"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
            },
            SyncStatusRow {
                item: "Projects".to_string(),
                database: db
                    .and_then(|d| d.get("projects"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
                sync_files: sync
                    .and_then(|s| s.get("projects"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
            },
            SyncStatusRow {
                item: "Task Lists".to_string(),
                database: db
                    .and_then(|d| d.get("task_lists"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
                sync_files: sync
                    .and_then(|s| s.get("task_lists"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
            },
            SyncStatusRow {
                item: "Tasks".to_string(),
                database: db
                    .and_then(|d| d.get("tasks"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
                sync_files: sync
                    .and_then(|s| s.get("tasks"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
            },
            SyncStatusRow {
                item: "Notes".to_string(),
                database: db
                    .and_then(|d| d.get("notes"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
                sync_files: sync
                    .and_then(|s| s.get("notes"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
            },
            SyncStatusRow {
                item: "Skills".to_string(),
                database: db
                    .and_then(|d| d.get("skills"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
                sync_files: sync
                    .and_then(|s| s.get("skills"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
            },
            SyncStatusRow {
                item: "Total".to_string(),
                database: db
                    .and_then(|d| d.get("total"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
                sync_files: sync
                    .and_then(|s| s.get("total"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or("-".to_string()),
            },
        ];

        let mut table = Table::new(rows);
        table.with(Style::rounded());
        output.push_str(&table.to_string());
    }

    output
}
