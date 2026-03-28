#![cfg(test)]

use super::JobExecutor;
use crate::jobs::{JobQueue, JobRegistry, Status};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_executor_spawns_job() {
    let queue = JobQueue::new();
    let registry = JobRegistry::new();
    let executor = JobExecutor::new(queue.clone(), registry);

    let job_id = "test-1".to_string();
    queue
        .create(
            job_id.clone(),
            "test_mock".to_string(),
            json!({"test": "data"}),
        )
        .unwrap();

    executor.execute_job(&job_id).await.unwrap();
    sleep(Duration::from_millis(50)).await;

    let job = queue.get(&job_id).unwrap();
    assert!(matches!(job.status, Status::Running | Status::Completed));
}

#[tokio::test]
async fn test_executor_completes_job() {
    let queue = JobQueue::new();
    let registry = JobRegistry::new();
    let executor = JobExecutor::new(queue.clone(), registry);

    let job_id = "test-2".to_string();
    queue
        .create(
            job_id.clone(),
            "test_mock".to_string(),
            json!({"test": "data"}),
        )
        .unwrap();

    executor.execute_job(&job_id).await.unwrap();
    sleep(Duration::from_millis(100)).await;

    let job = queue.get(&job_id).unwrap();
    assert_eq!(job.status, Status::Completed);
}

#[tokio::test]
async fn test_executor_handles_nonexistent() {
    let queue = JobQueue::new();
    let registry = JobRegistry::new();
    let executor = JobExecutor::new(queue, registry);

    let result = executor.execute_job("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_executor_runs_multiple() {
    let queue = JobQueue::new();
    let registry = JobRegistry::new();
    let executor = JobExecutor::new(queue.clone(), registry);

    for i in 0..5 {
        let job_id = format!("job-{}", i);
        queue
            .create(job_id.clone(), "test_mock".to_string(), json!({"index": i}))
            .unwrap();
        executor.execute_job(&job_id).await.unwrap();
    }

    sleep(Duration::from_millis(200)).await;

    for i in 0..5 {
        let job = queue.get(&format!("job-{}", i)).unwrap();
        assert_eq!(job.status, Status::Completed);
    }
}
