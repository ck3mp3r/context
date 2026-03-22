//! Job handler implementation using enums for static dispatch
//!
//! This design avoids trait objects and dynamic dispatch by using enums
//! to represent both job types and their parameters/results.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Job handler enum - each variant represents a different job type
///
/// This uses static dispatch instead of trait objects for better performance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobHandler {
    AnalyzeRepository,
    // Future job types can be added here:
    // ExportData,
    // BackupDatabase,
}

impl JobHandler {
    /// Get job type as string identifier
    pub fn as_str(&self) -> &'static str {
        match self {
            JobHandler::AnalyzeRepository => "analyze_repository",
        }
    }

    /// Execute the job with given parameters
    ///
    /// This dispatches to the appropriate handler based on the enum variant
    pub async fn execute(&self, params: JobParams) -> Result<JobResult, JobError> {
        match self {
            JobHandler::AnalyzeRepository => {
                let JobParams::AnalyzeRepository { repo_id, path: _ } = params;
                // Stub implementation - will be replaced with actual analysis
                Ok(JobResult::AnalyzeRepository {
                    repo_id,
                    status: "stubbed".to_string(),
                })
            }
        }
    }
}

impl FromStr for JobHandler {
    type Err = JobError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "analyze_repository" => Ok(JobHandler::AnalyzeRepository),
            _ => Err(JobError::UnknownJobType(s.to_string())),
        }
    }
}

impl fmt::Display for JobHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Job parameters enum - each variant corresponds to a JobHandler
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobParams {
    AnalyzeRepository { repo_id: String, path: String },
    // Future params can be added here
}

impl JobParams {
    /// Get the job type this params variant corresponds to
    pub fn job_type(&self) -> &'static str {
        match self {
            JobParams::AnalyzeRepository { .. } => "analyze_repository",
        }
    }
}

/// Job result enum - each variant corresponds to a JobHandler
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobResult {
    AnalyzeRepository { repo_id: String, status: String },
    // Future results can be added here
}

/// Job execution errors
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Parameters mismatch: expected {expected}, got {got}")]
    ParamsMismatch {
        expected: &'static str,
        got: &'static str,
    },

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Unknown job type: {0}")]
    UnknownJobType(String),
}

#[cfg(test)]
#[path = "handler_test.rs"]
mod handler_test;
