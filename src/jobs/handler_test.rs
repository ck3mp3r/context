#![cfg(test)]

use super::{JobHandler, JobParams, JobResult};
use std::str::FromStr;

#[tokio::test]
async fn test_job_handler_display() {
    let handler = JobHandler::AnalyzeRepository;
    assert_eq!(handler.to_string(), "analyze_repository");
}

#[tokio::test]
async fn test_job_handler_from_str() {
    let handler = JobHandler::from_str("analyze_repository");
    assert!(handler.is_ok());
    assert_eq!(handler.unwrap(), JobHandler::AnalyzeRepository);

    let unknown = JobHandler::from_str("unknown");
    assert!(unknown.is_err());
}

#[tokio::test]
async fn test_analyze_repository_execute_stub() {
    let handler = JobHandler::AnalyzeRepository;
    let params = JobParams::AnalyzeRepository {
        repo_id: "test123".to_string(),
        path: "/test/path".to_string(),
    };

    let result = handler.execute(params).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    match output {
        JobResult::AnalyzeRepository { repo_id, status } => {
            assert_eq!(repo_id, "test123");
            assert_eq!(status, "stubbed");
        }
    }
}

#[tokio::test]
async fn test_job_handler_is_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<JobHandler>();
    assert_sync::<JobHandler>();
}

#[test]
fn test_job_params_serialization() {
    let params = JobParams::AnalyzeRepository {
        repo_id: "abc123".to_string(),
        path: "/some/path".to_string(),
    };

    let json_value = serde_json::to_value(&params).unwrap();
    assert_eq!(json_value["repo_id"], "abc123");
    assert_eq!(json_value["path"], "/some/path");
}

#[test]
fn test_job_result_serialization() {
    let result = JobResult::AnalyzeRepository {
        repo_id: "xyz789".to_string(),
        status: "completed".to_string(),
    };

    let json_value = serde_json::to_value(&result).unwrap();
    assert_eq!(json_value["repo_id"], "xyz789");
    assert_eq!(json_value["status"], "completed");
}

#[tokio::test]
async fn test_all_handlers_registered() {
    // Ensure all job types can be constructed from strings
    let types = vec!["analyze_repository"];

    for job_type in types {
        let handler = JobHandler::from_str(job_type);
        assert!(handler.is_ok(), "Job type '{}' not registered", job_type);
    }
}
