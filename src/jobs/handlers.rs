//! Job handlers - concrete implementations

use super::job_trait::{Job, JobError, ProgressUpdate};
use crate::analysis::service;
use crate::sync::get_data_dir;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Analyze repository job
#[derive(Default)]
pub struct AnalyzeRepositoryJob;

#[derive(Debug, Deserialize)]
struct AnalyzeParams {
    repo_id: String,
    #[allow(dead_code)]
    path: String,
}

#[derive(Debug, Serialize)]
struct AnalyzeResult {
    repo_id: String,
    files_analyzed: usize,
    symbols_extracted: usize,
    relationships_created: usize,
}

impl Job for AnalyzeRepositoryJob {
    fn job_type() -> &'static str {
        "analyze_repository"
    }

    async fn execute(
        &self,
        params: Value,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<Value, JobError> {
        let params: AnalyzeParams = serde_json::from_value(params)?;

        // Get analysis path (same logic as MCP tool)
        let graph_path = get_analysis_path(&params.repo_id);

        // Ensure analysis directory exists
        let analysis_path = graph_path.join("analysis.nano");
        if !analysis_path.exists() {
            std::fs::create_dir_all(&analysis_path).map_err(|e| {
                JobError::AnalysisError(format!("Failed to create analysis directory: {}", e))
            })?;

            // Write schema
            let schema_path = analysis_path.join("schema.pg");
            std::fs::write(&schema_path, include_str!("../analysis/schema.pg"))
                .map_err(|e| JobError::AnalysisError(format!("Failed to write schema: {}", e)))?;

            // Initialize nanograph
            let init_output = std::process::Command::new("nanograph")
                .arg("init")
                .arg("--db")
                .arg(&analysis_path)
                .arg("--schema")
                .arg(&schema_path)
                .output()
                .map_err(|e| {
                    JobError::AnalysisError(format!("Failed to run nanograph init: {}", e))
                })?;

            if !init_output.status.success() {
                let stderr = String::from_utf8_lossy(&init_output.stderr);
                return Err(JobError::AnalysisError(format!(
                    "nanograph init failed: {}",
                    stderr
                )));
            }
        }

        // Run analysis with progress reporting
        let analysis_result = service::analyze_repository_with_progress(
            &PathBuf::from(&params.path),
            &params.repo_id,
            &graph_path,
            |current, total| {
                if let Some(ref tx) = progress_tx {
                    // Use blocking send to ensure no progress updates are lost
                    // This will wait if the channel buffer is full
                    let _ = tx.blocking_send(ProgressUpdate { current, total });
                }
            },
        )
        .await
        .map_err(|e| JobError::AnalysisError(e.to_string()))?;

        // Return result
        let result = AnalyzeResult {
            repo_id: params.repo_id,
            files_analyzed: analysis_result.files_analyzed,
            symbols_extracted: analysis_result.symbols_extracted,
            relationships_created: analysis_result.relationships_created,
        };

        Ok(serde_json::to_value(result)?)
    }
}

/// Get the analysis directory path for a repository
///
/// Uses the XDG-compliant data directory from sync::paths
fn get_analysis_path(repo_id: &str) -> PathBuf {
    get_data_dir().join("repos").join(repo_id)
}

#[cfg(test)]
#[path = "handlers_test.rs"]
mod handlers_test;
