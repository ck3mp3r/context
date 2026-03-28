//! Tests for in-memory job queue

use super::*;
use serde_json::json;

#[test]
fn test_create_job_with_queued_status() {
    let queue = JobQueue::new();
    let job = queue
        .create(
            "job-123".to_string(),
            "analyze_repository".to_string(),
            json!({"repo_id": "repo123", "path": "/test"}),
        )
        .unwrap();

    assert_eq!(job.job_id, "job-123");
    assert_eq!(job.job_type, "analyze_repository");
    assert_eq!(job.status, Status::Queued);
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
}

#[test]
fn test_invalid_status_transition_returns_error() {
    let queue = JobQueue::new();
    queue
        .create("job-bad".to_string(), "test".to_string(), json!({}))
        .unwrap();

    let result = queue.update_status("job-bad", Status::Completed);
    assert!(result.is_err());
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

    let result = json!({"success": true});
    queue.complete("job-complete", result.clone()).unwrap();

    let job = queue.get("job-complete").unwrap();
    assert_eq!(job.status, Status::Completed);
    assert_eq!(job.result, Some(result));
}

#[test]
fn test_fail_job_with_error() {
    let queue = JobQueue::new();
    queue
        .create("job-fail".to_string(), "test".to_string(), json!({}))
        .unwrap();
    queue.update_status("job-fail", Status::Running).unwrap();

    queue.fail("job-fail", "Error".to_string()).unwrap();

    let job = queue.get("job-fail").unwrap();
    assert_eq!(job.status, Status::Failed);
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

    let queued = queue.list_by_status(Status::Queued);
    assert_eq!(queued.len(), 2);

    let running = queue.list_by_status(Status::Running);
    assert_eq!(running.len(), 1);
}

#[test]
fn test_thread_safe_concurrent_access() {
    use std::thread;

    let queue = JobQueue::new();
    queue
        .create("job-thread".to_string(), "test".to_string(), json!({}))
        .unwrap();

    let q1 = queue.clone();
    let q2 = queue.clone();

    let h1 = thread::spawn(move || {
        q1.update_progress("job-thread", 50, 100).unwrap();
    });

    let h2 = thread::spawn(move || {
        let _ = q2.get("job-thread");
    });

    h1.join().unwrap();
    h2.join().unwrap();

    let job = queue.get("job-thread").unwrap();
    assert_eq!(job.progress, Some((50, 100)));
}
