//! Code analysis CLI commands

use clap::Args;
use serde_json::json;
use std::time::Duration;

use crate::cli::api_client::ApiClient;
use crate::cli::error::CliError;

#[derive(Debug, Args)]
pub struct AnalyzeArgs {
    /// Repository ID or path to analyze
    pub repo: String,

    /// Poll interval in seconds (default: 2)
    #[arg(long, default_value = "2")]
    pub poll_interval: u64,
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

    println!("Analysis started: {}", job_id);

    // Poll for completion
    loop {
        let status: serde_json::Value = ApiClient::handle_response(
            client
                .get(&format!("/api/v1/jobs/{}", job_id))
                .send()
                .await?,
        )
        .await?;

        match status["status"].as_str() {
            Some("completed") => {
                let result = &status["result"];
                return Ok(format!(
                    "Analysis complete!\n  Files: {}\n  Symbols: {}\n  Relationships: {}",
                    result["files_analyzed"],
                    result["symbols_extracted"],
                    result["relationships_created"]
                ));
            }
            Some("failed") => {
                let error_msg = status["error"].as_str().unwrap_or("Unknown error");
                return Err(CliError::InvalidResponse {
                    message: format!("Analysis failed: {}", error_msg),
                });
            }
            Some("running") => {
                if let Some(progress) = status.get("progress") {
                    println!("Progress: {}/{}", progress["current"], progress["total"]);
                }
            }
            Some("cancelled") => {
                return Err(CliError::InvalidResponse {
                    message: "Analysis was cancelled".to_string(),
                });
            }
            _ => {}
        }

        tokio::time::sleep(Duration::from_secs(args.poll_interval)).await;
    }
}
