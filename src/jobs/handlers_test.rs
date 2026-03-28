#![cfg(test)]

use super::*;
use serde_json::json;

#[tokio::test]
async fn test_invalid_params() {
    let job = AnalyzeRepositoryJob;
    let result = job.execute(json!({"wrong": "params"}), None).await;
    assert!(result.is_err());
}

#[test]
fn test_job_type() {
    assert_eq!(AnalyzeRepositoryJob::job_type(), "analyze_repository");
}
