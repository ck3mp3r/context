//! Job registry - compile-time dispatch
//!
//! Pure match statement - NO dyn, NO Box, NO traits

use serde_json::Value;

pub struct JobRegistry;

impl JobRegistry {
    pub fn new() -> Self {
        Self
    }

    /// Execute job by type - pure static dispatch via match
    pub async fn execute(
        &self,
        job_type: &str,
        params: Value,
    ) -> Result<Value, super::job::JobError> {
        match job_type {
            "analyze_repository" => super::job::AnalyzeRepositoryJob::execute(params).await,
            _ => Err(super::job::JobError::ExecutionFailed(format!(
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

#[cfg(test)]
#[path = "registry_test.rs"]
mod registry_test;
