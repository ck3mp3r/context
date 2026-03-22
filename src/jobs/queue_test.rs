//! Tests for in-memory job queue
//!
//! TDD: RED phase - write failing tests first

use super::*;
use serde_json::json;

#[test]
fn test_create_job_with_queued_status() {
    let queue = JobQueue::new();

    let job = queue
        .create(
            "job-123".to_string(),
            "test_job".to_string(),
            json!({"key": "value"}),
        )
        .unwrap();

    assert_eq!(job.job_id, "job-123");
    assert_eq!(job.job_type, "test_job");
    assert_eq!(job.status, Status::Queued);
    assert!(job.result.is_none());
    assert!(job.error.is_none());
    assert!(job.started_at.is_none());
    assert!(job.completed_at.is_none());
}

#[test]
fn test_get_job_by_id() {
    let queue = JobQueue::new();
    queue
        .create("job-456".to_string(), "test".to_string(), json!({}))
        .unwrap();

    let job = queue.get("job-456").unwrap();
    assert_eq!(job.job_id, "job-456");
}

#[test]
fn test_get_nonexistent_job_returns_error() {
    let queue = JobQueue::new();

    let result = queue.get("nonexistent");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), QueueError::NotFound(_)));
}

#[test]
fn test_update_status_queued_to_running() {
    let queue = JobQueue::new();
    queue
        .create("job-789".to_string(), "test".to_string(), json!({}))
        .unwrap();

    queue.update_status("job-789", Status::Running).unwrap();

    let job = queue.get("job-789").unwrap();
    assert_eq!(job.status, Status::Running);
    assert!(job.started_at.is_some());
}

#[test]
fn test_update_status_running_to_completed() {
    let queue = JobQueue::new();
    queue
        .create("job-abc".to_string(), "test".to_string(), json!({}))
        .unwrap();
    queue.update_status("job-abc", Status::Running).unwrap();

    queue.update_status("job-abc", Status::Completed).unwrap();

    let job = queue.get("job-abc").unwrap();
    assert_eq!(job.status, Status::Completed);
    assert!(job.completed_at.is_some());
}

#[test]
fn test_invalid_status_transition_returns_error() {
    let queue = JobQueue::new();
    queue
        .create("job-bad".to_string(), "test".to_string(), json!({}))
        .unwrap();

    // Can't go from queued directly to completed (must go through running)
    let result = queue.update_status("job-bad", Status::Completed);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        QueueError::InvalidTransition { .. }
    ));
}

#[test]
fn test_update_progress() {
    let queue = JobQueue::new();
    queue
        .create("job-prog".to_string(), "test".to_string(), json!({}))
        .unwrap();

    queue.update_progress("job-prog", 50, 100).unwrap();

    let job = queue.get("job-prog").unwrap();
    assert_eq!(job.progress, Some((50, 100)));
}

#[test]
fn test_complete_job_with_result() {
    let queue = JobQueue::new();
    queue
        .create("job-complete".to_string(), "test".to_string(), json!({}))
        .unwrap();
    queue
        .update_status("job-complete", Status::Running)
        .unwrap();

    let result = json!({"success": true, "count": 42});
    queue.complete("job-complete", result.clone()).unwrap();

    let job = queue.get("job-complete").unwrap();
    assert_eq!(job.status, Status::Completed);
    assert_eq!(job.result, Some(result));
    assert!(job.completed_at.is_some());
}

#[test]
fn test_fail_job_with_error() {
    let queue = JobQueue::new();
    queue
        .create("job-fail".to_string(), "test".to_string(), json!({}))
        .unwrap();
    queue.update_status("job-fail", Status::Running).unwrap();

    queue
        .fail("job-fail", "Something went wrong".to_string())
        .unwrap();

    let job = queue.get("job-fail").unwrap();
    assert_eq!(job.status, Status::Failed);
    assert_eq!(job.error, Some("Something went wrong".to_string()));
    assert!(job.completed_at.is_some());
}

#[test]
fn test_list_jobs_by_status() {
    let queue = JobQueue::new();
    queue
        .create("job-1".to_string(), "test".to_string(), json!({}))
        .unwrap();
    queue
        .create("job-2".to_string(), "test".to_string(), json!({}))
        .unwrap();
    queue
        .create("job-3".to_string(), "test".to_string(), json!({}))
        .unwrap();

    queue.update_status("job-2", Status::Running).unwrap();

    let queued_jobs = queue.list_by_status(Status::Queued);
    assert_eq!(queued_jobs.len(), 2);

    let running_jobs = queue.list_by_status(Status::Running);
    assert_eq!(running_jobs.len(), 1);
    assert_eq!(running_jobs[0].job_id, "job-2");
}

#[test]
fn test_thread_safe_concurrent_access() {
    use std::thread;

    let queue = JobQueue::new();
    queue
        .create("job-thread".to_string(), "test".to_string(), json!({}))
        .unwrap();

    let queue_clone1 = queue.clone();
    let queue_clone2 = queue.clone();

    let handle1 = thread::spawn(move || {
        queue_clone1.update_progress("job-thread", 50, 100).unwrap();
    });

    let handle2 = thread::spawn(move || {
        let _ = queue_clone2.get("job-thread");
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    // Should not panic or deadlock
    let job = queue.get("job-thread").unwrap();
    assert_eq!(job.progress, Some((50, 100)));
}
