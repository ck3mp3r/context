//! Job executor - spawns async tasks

use super::job_trait::ProgressUpdate;
use super::queue::{JobQueue, QueueError, Status};
use super::registry::JobRegistry;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, debug, error};

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
        info!("Executing job: {}", job_id);
        let job = self.queue.get(job_id)?;
        info!("Job type: {}", job.job_type);
        self.queue.update_status(job_id, Status::Running)?;

        let queue = self.queue.clone();
        let registry = self.registry.clone();
        let job_id_owned = job_id.to_string();
        let job_type = job.job_type.clone();
        let params = job.params.clone();

        tokio::spawn(async move {
            info!("Spawned job task for: {}", job_id_owned);
            // Create progress channel
            let (progress_tx, mut progress_rx) = mpsc::channel::<ProgressUpdate>(100);
            debug!("Created progress channel with buffer size 100");
            let queue_clone = queue.clone();
            let job_id_clone = job_id_owned.clone();

            // Spawn listener for progress updates
            tokio::spawn(async move {
                info!("Progress listener started for job: {}", job_id_clone);
                while let Some(update) = progress_rx.recv().await {
                    debug!("Progress listener received: {}/{}", update.current, update.total);
                    match queue_clone.update_progress(&job_id_clone, update.current, update.total) {
                        Ok(_) => debug!("Progress updated in queue: {}/{}", update.current, update.total),
                        Err(e) => error!("Failed to update progress in queue: {}", e),
                    }
                }
                info!("Progress listener stopped for job: {}", job_id_clone);
            });

            // Get job instance from registry
            info!("Getting job instance from registry: {}", job_type);
            match registry.get(&job_type) {
                Ok(job_instance) => {
                    info!("Executing job instance: {}", job_id_owned);
                    match job_instance.execute(params, Some(progress_tx)).await {
                        Ok(result) => {
                            info!("Job completed successfully: {}", job_id_owned);
                            match queue.complete(&job_id_owned, result) {
                                Ok(_) => info!("Job marked as completed: {}", job_id_owned),
                                Err(e) => error!("Failed to mark job as completed: {}", e),
                            }
                        }
                        Err(e) => {
                            error!("Job execution failed: {} - {}", job_id_owned, e);
                            match queue.fail(&job_id_owned, e.to_string()) {
                                Ok(_) => info!("Job marked as failed: {}", job_id_owned),
                                Err(e) => error!("Failed to mark job as failed: {}", e),
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get job instance from registry: {}", e);
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
