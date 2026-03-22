//! Job management CLI commands

use clap::{Args, Subcommand};

use crate::cli::api_client::ApiClient;
use crate::cli::error::CliError;

#[derive(Debug, Args)]
pub struct JobArgs {
    #[command(subcommand)]
    pub command: JobCommand,
}

#[derive(Debug, Subcommand)]
pub enum JobCommand {
    /// List all jobs
    List {
        /// Filter by status (queued, running, completed, failed, cancelled)
        #[arg(long)]
        status: Option<String>,

        /// Filter by job type
        #[arg(long)]
        job_type: Option<String>,
    },
    /// Get job status by ID
    Get {
        /// Job ID
        job_id: String,
    },
    /// Cancel a running job
    Cancel {
        /// Job ID
        job_id: String,
    },
}

pub async fn handle_job(client: &ApiClient, args: JobArgs) -> Result<String, CliError> {
    match args.command {
        JobCommand::List { status, job_type } => list_jobs(client, status, job_type).await,
        JobCommand::Get { job_id } => get_job(client, &job_id).await,
        JobCommand::Cancel { job_id } => cancel_job(client, &job_id).await,
    }
}

async fn list_jobs(
    client: &ApiClient,
    status: Option<String>,
    job_type: Option<String>,
) -> Result<String, CliError> {
    let mut url = "/api/v1/jobs?".to_string();
    if let Some(s) = status {
        url.push_str(&format!("status={}", s));
    }
    if let Some(jt) = job_type {
        if url.ends_with('?') {
            url.push_str(&format!("job_type={}", jt));
        } else {
            url.push_str(&format!("&job_type={}", jt));
        }
    }

    let response: serde_json::Value =
        ApiClient::handle_response(client.get(&url).send().await?).await?;

    let items = response["items"].as_array().ok_or_else(|| {
        CliError::InvalidResponse {
            message: "Invalid response format".to_string(),
        }
    })?;

    if items.is_empty() {
        return Ok("No jobs found".to_string());
    }

    let mut output = String::new();
    for job in items {
        output.push_str(&format!(
            "{} | {} | {} | {}\n",
            job["job_id"].as_str().unwrap_or("?"),
            job["status"].as_str().unwrap_or("?"),
            job["job_type"].as_str().unwrap_or("?"),
            format_progress(job.get("progress"))
        ));
    }

    Ok(output)
}

async fn get_job(client: &ApiClient, job_id: &str) -> Result<String, CliError> {
    let response: serde_json::Value = ApiClient::handle_response(
        client
            .get(&format!("/api/v1/jobs/{}", job_id))
            .send()
            .await?,
    )
    .await?;

    let mut output = format!(
        "Job ID: {}\nStatus: {}\nType: {}\n",
        response["job_id"].as_str().unwrap_or("?"),
        response["status"].as_str().unwrap_or("?"),
        response["job_type"].as_str().unwrap_or("?")
    );

    if let Some(progress) = response.get("progress") {
        output.push_str(&format!("Progress: {}\n", format_progress(Some(progress))));
    }

    if let Some(result) = response.get("result") {
        output.push_str(&format!("Result:\n{}\n", serde_json::to_string_pretty(result).unwrap()));
    }

    if let Some(error) = response.get("error") {
        output.push_str(&format!("Error: {}\n", error.as_str().unwrap_or("?")));
    }

    Ok(output)
}

async fn cancel_job(client: &ApiClient, job_id: &str) -> Result<String, CliError> {
    let _response: serde_json::Value = ApiClient::handle_response(
        client
            .delete(&format!("/api/v1/jobs/{}", job_id))
            .send()
            .await?,
    )
    .await?;

    Ok(format!("✓ Job {} cancelled", job_id))
}

fn format_progress(progress: Option<&serde_json::Value>) -> String {
    match progress {
        Some(p) if !p.is_null() => {
            format!(
                "{}/{}",
                p["current"].as_u64().unwrap_or(0),
                p["total"].as_u64().unwrap_or(0)
            )
        }
        _ => "-".to_string(),
    }
}
