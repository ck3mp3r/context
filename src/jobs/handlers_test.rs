#![cfg(test)]

use super::*;
use serde_json::json;

#[tokio::test]
async fn test_analyze_job() {
    let job = AnalyzeRepositoryJob;
    let params = json!({
        "repo_id": "test123",
        "path": "/test/path"
    });

    let result = job.execute(params).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output["repo_id"], "test123");
    assert_eq!(output["files_analyzed"], 0);
}

#[tokio::test]
async fn test_invalid_params() {
    let job = AnalyzeRepositoryJob;
    let result = job.execute(json!({"wrong": "params"})).await;
    assert!(result.is_err());
}

#[test]
fn test_job_type() {
    assert_eq!(AnalyzeRepositoryJob::job_type(), "analyze_repository");
}
