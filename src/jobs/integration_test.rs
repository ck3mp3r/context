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
use axum::http::StatusCode;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;

/// Spawn a test HTTP server with real job infrastructure
async fn spawn_test_server() -> (String, tokio::task::JoinHandle<()>) {
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
    let app = routes::create_router(state, false);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (url, handle)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_complete_job_queue_flow() {
    let (url, _handle) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // Step 1: Create a test repository in the database
    let create_repo_response = client
        .post(format!("{}/api/v1/repos", url))
        .json(&json!({
            "remote": "https://github.com/test/repo",
            "path": "/tmp/test-repo",
            "tags": ["test"],
            "project_ids": []
        }))
        .send()
        .await
        .expect("Failed to create repo");

    assert_eq!(create_repo_response.status(), StatusCode::CREATED);
    let repo_data: serde_json::Value = create_repo_response.json().await.unwrap();
    let _repo_id = repo_data["id"].as_str().unwrap();

    // Step 2: Create analysis job via API
    let create_job_response = client
        .post(format!("{}/api/v1/jobs", url))
        .json(&json!({
            "job_type": "test_mock",
            "params": {
                "duration_ms": 100,
                "should_fail": false
            }
        }))
        .send()
        .await
        .expect("Failed to create job");

    assert_eq!(create_job_response.status(), StatusCode::CREATED);
    let job_data: serde_json::Value = create_job_response.json().await.unwrap();
    let job_id = job_data["job_id"].as_str().unwrap();

    // Job should be queued or already running (executor is fast!)
    let initial_status = job_data["status"].as_str().unwrap();
    assert!(
        initial_status == "queued" || initial_status == "running",
        "Initial status should be queued or running, got: {}",
        initial_status
    );
    assert_eq!(job_data["job_type"], "test_mock");

    // Step 3: Poll for job completion
    let mut attempts = 0;
    let max_attempts = 50; // 5 seconds with 100ms sleep
    let mut final_status = None;

    while attempts < max_attempts {
        let status_response = client
            .get(format!("{}/api/v1/jobs/{}", url, job_id))
            .send()
            .await
            .expect("Failed to get job status");

        assert_eq!(status_response.status(), StatusCode::OK);
        let status_data: serde_json::Value = status_response.json().await.unwrap();

        match status_data["status"].as_str() {
            Some("completed") => {
                final_status = Some(status_data);
                break;
            }
            Some("failed") => {
                panic!("Job failed: {:?}", status_data["error"]);
            }
            Some("running") | Some("queued") => {
                // Continue polling
            }
            status => {
                panic!("Unexpected status: {:?}", status);
            }
        }

        attempts += 1;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Step 4: Verify final status
    let final_data = final_status.expect("Job did not complete within timeout");
    assert_eq!(final_data["status"], "completed");
    assert_eq!(final_data["job_type"], "test_mock");

    // Verify result structure
    let result = &final_data["result"];
    assert!(result.is_object(), "Result should be an object");
    assert_eq!(result["success"], true);

    // Step 5: Verify job appears in list
    let list_response = client
        .get(format!("{}/api/v1/jobs", url))
        .send()
        .await
        .expect("Failed to list jobs");

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_data: serde_json::Value = list_response.json().await.unwrap();

    let jobs = list_data["items"].as_array().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["job_id"], job_id);
    assert_eq!(jobs[0]["status"], "completed");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_job_queue_handles_failure() {
    let (url, _handle) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // Create a job that will fail
    let create_job_response = client
        .post(format!("{}/api/v1/jobs", url))
        .json(&json!({
            "job_type": "test_mock",
            "params": {
                "duration_ms": 50,
                "should_fail": true
            }
        }))
        .send()
        .await
        .expect("Failed to create job");

    assert_eq!(create_job_response.status(), StatusCode::CREATED);
    let job_data: serde_json::Value = create_job_response.json().await.unwrap();
    let job_id = job_data["job_id"].as_str().unwrap();

    // Poll for job failure
    let mut attempts = 0;
    let max_attempts = 50;

    while attempts < max_attempts {
        let status_response = client
            .get(format!("{}/api/v1/jobs/{}", url, job_id))
            .send()
            .await
            .expect("Failed to get job status");

        let status_data: serde_json::Value = status_response.json().await.unwrap();

        match status_data["status"].as_str() {
            Some("failed") => {
                // Success - job failed as expected
                assert!(
                    status_data["error"].as_str().is_some(),
                    "Failed job should have error message"
                );
                return;
            }
            Some("completed") => {
                panic!("Job should have failed but completed successfully");
            }
            Some("running") | Some("queued") => {
                // Continue polling
            }
            status => {
                panic!("Unexpected status: {:?}", status);
            }
        }

        attempts += 1;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    panic!("Job did not fail within timeout");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_multiple_concurrent_jobs() {
    let (url, _handle) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // Create 3 jobs
    let mut job_ids = Vec::new();
    for i in 0..3 {
        let response = client
            .post(format!("{}/api/v1/jobs", url))
            .json(&json!({
                "job_type": "test_mock",
                "params": {
                    "duration_ms": 50 + (i * 10),
                    "should_fail": false
                }
            }))
            .send()
            .await
            .expect("Failed to create job");

        let data: serde_json::Value = response.json().await.unwrap();
        job_ids.push(data["job_id"].as_str().unwrap().to_string());
    }

    assert_eq!(job_ids.len(), 3);

    // Wait for all jobs to complete
    let mut attempts = 0;
    let max_attempts = 100;

    while attempts < max_attempts {
        let list_response = client
            .get(format!("{}/api/v1/jobs?status=completed", url))
            .send()
            .await
            .expect("Failed to list jobs");

        let list_data: serde_json::Value = list_response.json().await.unwrap();
        let completed_jobs = list_data["items"].as_array().unwrap();

        if completed_jobs.len() == 3 {
            // All jobs completed
            return;
        }

        attempts += 1;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    panic!("Not all jobs completed within timeout");
}
