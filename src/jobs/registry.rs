//! Job registry - returns job instances using generics
//!
//! Pure static dispatch - uses match to return concrete job types

use super::handlers::AnalyzeRepositoryJob;
use super::job_trait::{Job, JobError};

pub struct JobRegistry;

impl JobRegistry {
    pub fn new() -> Self {
        Self
    }

    /// Get a job instance by type - returns concrete type via enum
    pub fn get(&self, job_type: &str) -> Result<JobInstance, JobError> {
        match job_type {
            "analyze_repository" => Ok(JobInstance::AnalyzeRepository(AnalyzeRepositoryJob)),
            #[cfg(test)]
            "test_mock" => Ok(JobInstance::TestMock(TestMockJob)),
            _ => Err(JobError::ExecutionFailed(format!(
                "Unknown job type: {}",
                job_type
            ))),
        }
    }
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Job instance enum - each variant holds a concrete job type
///
/// This allows returning different job types while maintaining static dispatch
pub enum JobInstance {
    AnalyzeRepository(AnalyzeRepositoryJob),
    #[cfg(test)]
    TestMock(TestMockJob),
}

impl JobInstance {
    /// Execute the job - dispatches to concrete type
    pub async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value, JobError> {
        match self {
            JobInstance::AnalyzeRepository(job) => job.execute(params).await,
            #[cfg(test)]
            JobInstance::TestMock(job) => job.execute(params).await,
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct TestMockJob;

#[cfg(test)]
impl Job for TestMockJob {
    fn job_type() -> &'static str {
        "test_mock"
    }

    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value, JobError> {
        // Sleep for duration_ms if specified
        if let Some(duration_ms) = params.get("duration_ms").and_then(|v| v.as_u64()) {
            tokio::time::sleep(tokio::time::Duration::from_millis(duration_ms)).await;
        }

        // Fail if should_fail is true
        if let Some(true) = params.get("should_fail").and_then(|v| v.as_bool()) {
            return Err(JobError::ExecutionFailed(
                "TestMockJob was configured to fail".to_string(),
            ));
        }

        Ok(serde_json::json!({
            "success": true,
            "params": params
        }))
    }
}

#[cfg(test)]
#[path = "registry_test.rs"]
mod registry_test;
