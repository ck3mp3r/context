#![cfg(test)]

use super::*;
use serde_json::json;

#[tokio::test]
async fn test_analyze_job_stub() {
    let params = json!({
        "repo_id": "test123",
        "path": "/test/path"
    });

    let result = AnalyzeRepositoryJob::execute(params).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output["repo_id"], "test123");
    assert_eq!(output["files_analyzed"], 0);
}

#[tokio::test]
async fn test_invalid_params_returns_error() {
    let invalid_params = json!({"wrong": "params"});

    let result = AnalyzeRepositoryJob::execute(invalid_params).await;
    assert!(result.is_err());
}
