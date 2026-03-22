//! Code analysis CLI commands

use clap::Args;
use serde_json::json;

use crate::cli::api_client::ApiClient;
use crate::cli::error::CliError;

#[derive(Debug, Args)]
pub struct AnalyzeArgs {
    /// Repository ID or path to analyze
    pub repo: String,
}

pub async fn analyze(client: &ApiClient, args: AnalyzeArgs) -> Result<String, CliError> {
    // Lookup repo by ID or path
    let repos: serde_json::Value =
        ApiClient::handle_response(client.get("/api/v1/repos").send().await?).await?;

    let repo = repos["items"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|r| r["id"] == args.repo || r["path"].as_str() == Some(&args.repo))
        })
        .ok_or_else(|| CliError::InvalidResponse {
            message: format!("Repository not found: {}", args.repo),
        })?;

    let repo_id = repo["id"]
        .as_str()
        .ok_or_else(|| CliError::InvalidResponse {
            message: "Invalid repo id".to_string(),
        })?;
    let repo_path = repo["path"]
        .as_str()
        .ok_or_else(|| CliError::InvalidResponse {
            message: "Invalid repo path".to_string(),
        })?;

    // Create analysis job
    let job: serde_json::Value = ApiClient::handle_response(
        client
            .post("/api/v1/jobs")
            .json(&json!({
                "job_type": "analyze_repository",
                "params": {
                    "repo_id": repo_id,
                    "path": repo_path,
                }
            }))
            .send()
            .await?,
    )
    .await?;

    let job_id = job["job_id"]
        .as_str()
        .ok_or_else(|| CliError::InvalidResponse {
            message: "Invalid job_id".to_string(),
        })?;

    Ok(format!(
        "✓ Analysis started: {}\n  Use 'c5t job get {}' to check progress",
        job_id, job_id
    ))
}
