//! In-memory job queue implementation
//!
//! Manages job lifecycle and state using HashMap + RwLock for thread-safe access.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("Job not found: {0}")]
    NotFound(String),

    #[error("Invalid status transition: {from} -> {to}")]
    InvalidTransition { from: Status, to: Status },

    #[error("Lock error")]
    LockError,
}

/// Job status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Queued => write!(f, "queued"),
            Status::Running => write!(f, "running"),
            Status::Completed => write!(f, "completed"),
            Status::Failed => write!(f, "failed"),
            Status::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Job status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatus {
    pub job_id: String,
    pub job_type: String,
    pub params: Value,
    pub status: Status,
    pub progress: Option<(usize, usize)>, // (current, total)
    pub result: Option<Value>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// In-memory job queue
#[derive(Clone)]
pub struct JobQueue {
    jobs: Arc<RwLock<HashMap<String, JobStatus>>>,
}

impl JobQueue {
    /// Create a new empty job queue
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new job with status "queued"
    pub fn create(
        &self,
        job_id: String,
        job_type: String,
        params: Value,
    ) -> Result<JobStatus, QueueError> {
        let mut jobs = self.jobs.write().map_err(|_| QueueError::LockError)?;

        let job_status = JobStatus {
            job_id: job_id.clone(),
            job_type,
            params,
            status: Status::Queued,
            progress: None,
            result: None,
            error: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        };

        jobs.insert(job_id, job_status.clone());
        Ok(job_status)
    }

    /// Get a job by ID
    pub fn get(&self, job_id: &str) -> Result<JobStatus, QueueError> {
        let jobs = self.jobs.read().map_err(|_| QueueError::LockError)?;
        jobs.get(job_id)
            .cloned()
            .ok_or_else(|| QueueError::NotFound(job_id.to_string()))
    }

    /// Update job status
    pub fn update_status(&self, job_id: &str, new_status: Status) -> Result<(), QueueError> {
        let mut jobs = self.jobs.write().map_err(|_| QueueError::LockError)?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| QueueError::NotFound(job_id.to_string()))?;

        // Validate status transition
        match (&job.status, &new_status) {
            (Status::Queued, Status::Running) => {
                job.status = new_status;
                job.started_at = Some(Utc::now());
            }
            (Status::Running, Status::Completed) => {
                job.status = new_status;
                job.completed_at = Some(Utc::now());
            }
            (Status::Running, Status::Failed) => {
                job.status = new_status;
                job.completed_at = Some(Utc::now());
            }
            (Status::Queued, Status::Cancelled) | (Status::Running, Status::Cancelled) => {
                job.status = new_status;
                job.completed_at = Some(Utc::now());
            }
            _ => {
                return Err(QueueError::InvalidTransition {
                    from: job.status,
                    to: new_status,
                });
            }
        }

        Ok(())
    }

    /// Update job progress
    pub fn update_progress(
        &self,
        job_id: &str,
        current: usize,
        total: usize,
    ) -> Result<(), QueueError> {
        let mut jobs = self.jobs.write().map_err(|_| QueueError::LockError)?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| QueueError::NotFound(job_id.to_string()))?;
        job.progress = Some((current, total));
        Ok(())
    }

    /// Mark job as completed with result
    pub fn complete(&self, job_id: &str, result: Value) -> Result<(), QueueError> {
        let mut jobs = self.jobs.write().map_err(|_| QueueError::LockError)?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| QueueError::NotFound(job_id.to_string()))?;

        job.status = Status::Completed;
        job.result = Some(result);
        job.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Mark job as failed with error message
    pub fn fail(&self, job_id: &str, error: String) -> Result<(), QueueError> {
        let mut jobs = self.jobs.write().map_err(|_| QueueError::LockError)?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| QueueError::NotFound(job_id.to_string()))?;

        job.status = Status::Failed;
        job.error = Some(error);
        job.completed_at = Some(Utc::now());
        Ok(())
    }

    /// List jobs by status
    pub fn list_by_status(&self, status: Status) -> Vec<JobStatus> {
        let jobs = self.jobs.read().unwrap();
        jobs.values()
            .filter(|job| job.status == status)
            .cloned()
            .collect()
    }

    /// List all jobs with optional filtering by status and job_type
    pub fn list(&self, status: Option<&str>, job_type: Option<&str>) -> Vec<JobStatus> {
        let jobs = self.jobs.read().unwrap();
        jobs.values()
            .filter(|job| {
                let status_match = status.map_or(true, |s| job.status.to_string() == s);
                let type_match = job_type.map_or(true, |t| job.job_type == t);
                status_match && type_match
            })
            .cloned()
            .collect()
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "queue_test.rs"]
mod queue_test;
