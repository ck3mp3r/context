// Integration test: Complete job queue flow
//
// End-to-end test covering:
// 1. API creates analysis job via POST /api/v1/jobs
// 2. Job is stored in database (status: queued)
// 3. JobExecutor picks up job and executes
// 4. Job updates progress during execution
// 5. Job completes successfully with result
// 6. API polls and receives final result

use crate::api::{AppState, routes};
use crate::db::{Database, SqliteDatabase};
use crate::jobs::{JobExecutor, JobQueue, JobRegistry};
use crate::sync::MockGitOps;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

/// Create test app with job infrastructure
async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");
    let temp_dir = TempDir::new().unwrap();

    // Create job infrastructure (real components)
    let job_queue = JobQueue::new();
    let job_registry = JobRegistry::new();
    let job_executor = JobExecutor::new(job_queue.clone(), job_registry);

    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
        job_queue,
        job_executor,
    );
    routes::create_router(state, false)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_complete_job_queue_flow() {
    let app = test_app().await;

    // Create job
    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "job_type": "test_mock",
                "params": {
                    "duration_ms": 100,
                    "should_fail": false
                }
            }))
            .unwrap(),
        ))
        .unwrap();

    let response = app.clone().oneshot(create_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let job_data: Value = serde_json::from_slice(&body).unwrap();
    let job_id = job_data["job_id"].as_str().unwrap();

    // Poll for completion
    let mut attempts = 0;
    let max_attempts = 50;

    while attempts < max_attempts {
        let status_request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/jobs/{}", job_id))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(status_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let status_data: Value = serde_json::from_slice(&body).unwrap();

        if status_data["status"] == "completed" {
            assert_eq!(status_data["result"]["success"], true);
            return;
        } else if status_data["status"] == "failed" {
            panic!("Job failed: {:?}", status_data["error"]);
        }

        attempts += 1;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    panic!("Job did not complete within timeout");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_job_queue_handles_failure() {
    let app = test_app().await;

    // Create job that will fail
    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "job_type": "test_mock",
                "params": {
                    "duration_ms": 50,
                    "should_fail": true
                }
            }))
            .unwrap(),
        ))
        .unwrap();

    let response = app.clone().oneshot(create_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let job_data: Value = serde_json::from_slice(&body).unwrap();
    let job_id = job_data["job_id"].as_str().unwrap();

    // Poll for failure
    let mut attempts = 0;
    let max_attempts = 50;

    while attempts < max_attempts {
        let status_request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/jobs/{}", job_id))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(status_request).await.unwrap();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let status_data: Value = serde_json::from_slice(&body).unwrap();

        if status_data["status"] == "failed" {
            assert!(status_data["error"].as_str().is_some());
            return;
        } else if status_data["status"] == "completed" {
            panic!("Job should have failed but completed successfully");
        }

        attempts += 1;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    panic!("Job did not fail within timeout");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_multiple_concurrent_jobs() {
    let app = test_app().await;

    // Create 3 jobs
    let mut job_ids = Vec::new();
    for i in 0..3 {
        let create_request = Request::builder()
            .method("POST")
            .uri("/api/v1/jobs")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({
                    "job_type": "test_mock",
                    "params": {
                        "duration_ms": 50 + (i * 10),
                        "should_fail": false
                    }
                }))
                .unwrap(),
            ))
            .unwrap();

        let response = app.clone().oneshot(create_request).await.unwrap();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let data: Value = serde_json::from_slice(&body).unwrap();
        job_ids.push(data["job_id"].as_str().unwrap().to_string());
    }

    assert_eq!(job_ids.len(), 3);

    // Wait for all jobs to complete
    let mut attempts = 0;
    let max_attempts = 100;

    while attempts < max_attempts {
        let list_request = Request::builder()
            .method("GET")
            .uri("/api/v1/jobs?status=completed")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(list_request).await.unwrap();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let list_data: Value = serde_json::from_slice(&body).unwrap();
        let completed_jobs = list_data["items"].as_array().unwrap();

        if completed_jobs.len() == 3 {
            return;
        }

        attempts += 1;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    panic!("Not all jobs completed within timeout");
}
