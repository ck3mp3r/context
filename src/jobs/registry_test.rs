#![cfg(test)]

use super::*;

#[test]
fn test_registry_get_job() {
    let registry = JobRegistry::new();
    let instance = registry.get("test_mock");
    assert!(instance.is_ok());
}

#[test]
fn test_registry_unknown_type() {
    let registry = JobRegistry::new();
    let result = registry.get("unknown");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_job_instance_execute() {
    let registry = JobRegistry::new();
    let instance = registry.get("test_mock").unwrap();

    let result = instance
        .execute(serde_json::json!({
            "test": "data"
        }))
        .await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output["success"], true);
    assert_eq!(output["params"]["test"], "data");
}
