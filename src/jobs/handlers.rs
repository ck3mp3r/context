//! Job handlers - concrete implementations

use super::job_trait::{Job, JobError, ProgressUpdate};
use crate::analysis::service;
use crate::sync::get_data_dir;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

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
        info!("Starting analyze_repository job");
        let params: AnalyzeParams = serde_json::from_value(params)?;
        info!(
            "Job params: repo_id={}, path={}",
            params.repo_id, params.path
        );

        // Get analysis path (same logic as MCP tool)
        let graph_path = get_analysis_path(&params.repo_id);
        info!("Analysis path: {:?}", graph_path);

        // Ensure analysis directory exists
        let analysis_path = graph_path.join("analysis.nano");
        if !analysis_path.exists() {
            info!("Initializing nanograph database at {:?}", analysis_path);
            std::fs::create_dir_all(&analysis_path).map_err(|e| {
                error!("Failed to create analysis directory: {}", e);
                JobError::AnalysisError(format!("Failed to create analysis directory: {}", e))
            })?;

            // Write schema
            let schema_path = analysis_path.join("schema.pg");
            debug!("Writing schema to {:?}", schema_path);
            std::fs::write(&schema_path, include_str!("../analysis/schema.pg")).map_err(|e| {
                error!("Failed to write schema: {}", e);
                JobError::AnalysisError(format!("Failed to write schema: {}", e))
            })?;

            // Initialize nanograph
            info!("Running nanograph init command");
            let init_output = std::process::Command::new("nanograph")
                .arg("init")
                .arg("--db")
                .arg(&analysis_path)
                .arg("--schema")
                .arg(&schema_path)
                .output()
                .map_err(|e| {
                    error!("Failed to run nanograph init: {}", e);
                    JobError::AnalysisError(format!("Failed to run nanograph init: {}", e))
                })?;

            if !init_output.status.success() {
                let stderr = String::from_utf8_lossy(&init_output.stderr);
                error!("nanograph init failed: {}", stderr);
                return Err(JobError::AnalysisError(format!(
                    "nanograph init failed: {}",
                    stderr
                )));
            }
            info!("Nanograph initialization complete");
        } else {
            info!("Nanograph database already exists at {:?}", analysis_path);
        }

        // Run analysis with progress reporting
        info!("Starting repository analysis");
        let analysis_result = service::analyze_repository_with_progress(
            &PathBuf::from(&params.path),
            &params.repo_id,
            &graph_path,
            |current, total| {
                debug!("Progress update: {}/{}", current, total);
                if let Some(ref tx) = progress_tx {
                    // Use try_send to avoid blocking the async runtime
                    // If the buffer is full, we skip this update (non-critical)
                    match tx.try_send(ProgressUpdate { current, total }) {
                        Ok(_) => debug!("Progress sent successfully: {}/{}", current, total),
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            debug!(
                                "Progress channel full, skipping update {}/{}",
                                current, total
                            )
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => {
                            error!("Progress channel closed")
                        }
                    }
                }
            },
        )
        .await
        .map_err(|e| {
            error!("Analysis failed: {}", e);
            JobError::AnalysisError(e.to_string())
        })?;

        info!(
            "Analysis complete: {} files, {} symbols, {} relationships",
            analysis_result.files_analyzed,
            analysis_result.symbols_extracted,
            analysis_result.relationships_created
        );

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
