//! Job executor - spawns async tasks
//!
//! Uses tokio::spawn for non-blocking background execution.
//! Pure static dispatch with registry lookup.

use super::queue::{JobQueue, QueueError, Status};
use super::registry::JobRegistry;
use std::sync::Arc;

/// Job executor manages async job execution
pub struct JobExecutor {
    queue: Arc<JobQueue>,
    registry: Arc<JobRegistry>,
}

impl JobExecutor {
    /// Create a new job executor
    pub fn new(queue: JobQueue, registry: JobRegistry) -> Self {
        Self {
            queue: Arc::new(queue),
            registry: Arc::new(registry),
        }
    }

    /// Execute a job asynchronously
    pub async fn execute_job(&self, job_id: &str) -> Result<(), QueueError> {
        // 1. Load job from queue
        let job = self.queue.get(job_id)?;

        // 2. Update status to "running"
        self.queue.update_status(job_id, Status::Running)?;

        // 3. Spawn tokio task
        let queue = self.queue.clone();
        let registry = self.registry.clone();
        let job_id_owned = job_id.to_string();
        let job_type = job.job_type.clone();
        let params = job.params.clone();

        tokio::spawn(async move {
            // Execute via registry
            match registry.execute(&job_type, params).await {
                Ok(result) => {
                    let _ = queue.complete(&job_id_owned, result);
                }
                Err(e) => {
                    let _ = queue.fail(&job_id_owned, e.to_string());
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
#[path = "executor_test.rs"]
mod executor_test;
