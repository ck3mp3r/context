//! Job handlers - concrete implementations

use super::job_trait::{Job, JobError, ProgressUpdate};
use crate::analysis::{get_analysis_path, service};
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

        // Run analysis with progress reporting
        // CodeGraph::new() handles init if needed
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

#[cfg(test)]
#[path = "handlers_test.rs"]
mod handlers_test;
