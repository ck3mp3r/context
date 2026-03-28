//! Job trait - pure static dispatch with generics
//!
//! NO dyn, NO Box - only concrete types used via generics

use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc;

/// Progress update from a job
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub current: usize,
    pub total: usize,
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid parameters: {0}")]
    InvalidParams(#[from] serde_json::Error),

    #[error("Analysis error: {0}")]
    AnalysisError(String),
}

/// Job trait - implement this for each job type
///
/// Used with generics for pure static dispatch
pub trait Job: Send + Sync + Sized + Default {
    fn job_type() -> &'static str;

    fn execute(
        &self,
        params: Value,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> impl std::future::Future<Output = Result<Value, JobError>> + Send;
}
