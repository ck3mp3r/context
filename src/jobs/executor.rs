//! Job executor - spawns async tasks for job execution
//!
//! Uses tokio::spawn for non-blocking background execution.
//! Jobs are executed based on their JobHandler enum variant.

use super::queue::{JobQueue, QueueError, Status};
use std::sync::Arc;

/// Job executor manages async job execution
#[derive(Clone)]
pub struct JobExecutor {
    queue: Arc<JobQueue>,
}

impl JobExecutor {
    /// Create a new job executor
    pub fn new(queue: JobQueue) -> Self {
        Self {
            queue: Arc::new(queue),
        }
    }

    /// Execute a job asynchronously
    ///
    /// This spawns a tokio task and returns immediately.
    /// The job status is tracked in the queue.
    pub async fn execute_job(&self, job_id: &str) -> Result<(), QueueError> {
        // 1. Load job from queue
        let job = self.queue.get(job_id)?;

        // 2. Update status to "running"
        self.queue.update_status(job_id, Status::Running)?;

        // 3. Spawn tokio task for async execution
        let queue = self.queue.clone();
        let job_id_owned = job_id.to_string();
        let handler = job.job_type;
        let params = job.params;

        tokio::spawn(async move {
            // Execute the job
            match handler.execute(params).await {
                Ok(result) => {
                    // Serialize result to JSON
                    if let Ok(json_result) = serde_json::to_value(&result) {
                        let _ = queue.complete(&job_id_owned, json_result);
                    } else {
                        let _ = queue.fail(&job_id_owned, "Failed to serialize result".to_string());
                    }
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
