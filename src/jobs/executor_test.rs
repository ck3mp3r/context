#![cfg(test)]

use super::JobExecutor;
use crate::jobs::{JobHandler, JobParams, JobQueue, Status};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_executor_spawns_job_and_updates_status() {
    let queue = JobQueue::new();
    let executor = JobExecutor::new(queue.clone());

    // Create a job
    let job_id = "test-job-1".to_string();
    queue
        .create(
            job_id.clone(),
            JobHandler::AnalyzeRepository,
            JobParams::AnalyzeRepository {
                repo_id: "repo123".to_string(),
                path: "/test/path".to_string(),
            },
        )
        .unwrap();

    // Job should start as queued
    let job = queue.get(&job_id).unwrap();
    assert_eq!(job.status, Status::Queued);

    // Execute the job
    executor.execute_job(&job_id).await.unwrap();

    // Give it a moment to spawn
    sleep(Duration::from_millis(50)).await;

    // Job should have transitioned to running or completed
    let job = queue.get(&job_id).unwrap();
    assert!(matches!(job.status, Status::Running | Status::Completed));
}

#[tokio::test]
async fn test_executor_completes_job_successfully() {
    let queue = JobQueue::new();
    let executor = JobExecutor::new(queue.clone());

    let job_id = "test-job-2".to_string();
    queue
        .create(
            job_id.clone(),
            JobHandler::AnalyzeRepository,
            JobParams::AnalyzeRepository {
                repo_id: "repo456".to_string(),
                path: "/test/path2".to_string(),
            },
        )
        .unwrap();

    executor.execute_job(&job_id).await.unwrap();

    // Wait for completion
    sleep(Duration::from_millis(100)).await;

    let job = queue.get(&job_id).unwrap();
    assert_eq!(job.status, Status::Completed);
    assert!(job.result.is_some());
}

#[tokio::test]
async fn test_executor_handles_nonexistent_job() {
    let queue = JobQueue::new();
    let executor = JobExecutor::new(queue.clone());

    let result = executor.execute_job("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_executor_runs_multiple_jobs_concurrently() {
    let queue = JobQueue::new();
    let executor = JobExecutor::new(queue.clone());

    // Create 5 jobs
    let job_ids: Vec<String> = (0..5).map(|i| format!("job-{}", i)).collect();

    for job_id in &job_ids {
        queue
            .create(
                job_id.clone(),
                JobHandler::AnalyzeRepository,
                JobParams::AnalyzeRepository {
                    repo_id: format!("repo-{}", job_id),
                    path: "/test/path".to_string(),
                },
            )
            .unwrap();
    }

    // Execute all jobs
    for job_id in &job_ids {
        executor.execute_job(job_id).await.unwrap();
    }

    // Wait for all to complete
    sleep(Duration::from_millis(200)).await;

    // All should be completed
    for job_id in &job_ids {
        let job = queue.get(job_id).unwrap();
        assert_eq!(job.status, Status::Completed);
    }
}

#[tokio::test]
async fn test_executor_is_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<JobExecutor>();
    assert_sync::<JobExecutor>();
}
