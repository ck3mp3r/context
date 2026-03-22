#![cfg(test)]

use super::*;

#[tokio::test]
async fn test_execute_analyze_repository() {
    let registry = JobRegistry::new();

    let params = serde_json::json!({
        "repo_id": "test123",
        "path": "/test/path"
    });

    let result = registry.execute("analyze_repository", params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_execute_unknown_type() {
    let registry = JobRegistry::new();
    let result = registry.execute("unknown", serde_json::json!({})).await;
    assert!(result.is_err());
}

#[test]
fn test_registry_is_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<JobRegistry>();
    assert_sync::<JobRegistry>();
}
