//! Code analysis service — public API
//!
//! Exposes `analyze_repository` and `analyze_repository_with_progress`
//! which external modules (MCP tools, job handlers) call.

use crate::analysis::pipeline;
use crate::analysis::store::CodeGraph;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Store error: {0}")]
    Store(#[from] crate::analysis::store::StoreError),

    #[error("Pipeline error: {0}")]
    Pipeline(#[from] pipeline::PipelineError),
}

pub struct AnalysisResult {
    pub files_analyzed: usize,
    pub symbols_extracted: usize,
    pub relationships_created: usize,
}

pub async fn analyze_repository(
    repo_path: &Path,
    repo_id: &str,
    graph_path: &Path,
) -> Result<AnalysisResult, AnalysisError> {
    analyze_repository_with_progress(repo_path, repo_id, graph_path, |_, _| {}).await
}

pub async fn analyze_repository_with_progress<F>(
    repo_path: &Path,
    repo_id: &str,
    graph_path: &Path,
    progress_fn: F,
) -> Result<AnalysisResult, AnalysisError>
where
    F: Fn(usize, usize) + Send + Sync,
{
    let analysis_path = graph_path.join("analysis.nano");
    if analysis_path.exists() {
        tracing::info!("Removing existing graph at {:?}", analysis_path);
        std::fs::remove_dir_all(&analysis_path)?;
    }

    tracing::info!("Creating CodeGraph for repo_id: {}", repo_id);
    let mut graph = CodeGraph::new(graph_path, repo_id)?;

    let result = pipeline::run(repo_path, repo_id, &mut graph)?;

    progress_fn(result.files_analyzed, result.files_analyzed);

    tracing::info!("Committing all data to nanograph...");
    graph.commit()?;

    crate::analysis::queries::install_bundled_queries(graph_path)?;

    tracing::info!(
        "Analysis complete: {} files, {} symbols, {} relationships",
        result.files_analyzed,
        result.symbols_extracted,
        result.relationships_created
    );

    Ok(AnalysisResult {
        files_analyzed: result.files_analyzed,
        symbols_extracted: result.symbols_extracted,
        relationships_created: result.relationships_created,
    })
}
