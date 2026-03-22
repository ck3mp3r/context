//! Job executor - spawns async tasks

use super::queue::{JobQueue, QueueError, Status};
use super::registry::JobRegistry;
use std::sync::Arc;

#[derive(Clone)]
pub struct JobExecutor {
    queue: Arc<JobQueue>,
    registry: Arc<JobRegistry>,
}

impl JobExecutor {
    pub fn new(queue: JobQueue, registry: JobRegistry) -> Self {
        Self {
            queue: Arc::new(queue),
            registry: Arc::new(registry),
        }
    }

    pub async fn execute_job(&self, job_id: &str) -> Result<(), QueueError> {
        let job = self.queue.get(job_id)?;
        self.queue.update_status(job_id, Status::Running)?;

        let queue = self.queue.clone();
        let registry = self.registry.clone();
        let job_id_owned = job_id.to_string();
        let job_type = job.job_type.clone();
        let params = job.params.clone();

        tokio::spawn(async move {
            // Get job instance from registry
            match registry.get(&job_type) {
                Ok(job_instance) => match job_instance.execute(params).await {
                    Ok(result) => {
                        let _ = queue.complete(&job_id_owned, result);
                    }
                    Err(e) => {
                        let _ = queue.fail(&job_id_owned, e.to_string());
                    }
                },
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
