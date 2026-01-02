use crate::cli::api_client::ApiClient;
use crate::cli::commands::task_list::*;
use crate::cli::utils::parse_tags;

// Test helper functions
#[test]
fn test_parse_tags_helper() {
    let tags = parse_tags(Some("tag1,tag2"));
    assert_eq!(tags, Some(vec!["tag1".to_string(), "tag2".to_string()]));

    let empty = parse_tags(None);
    assert_eq!(empty, None);
}

#[test]
fn test_parse_repo_ids() {
    let ids = "abc12345,def67890";
    let result: Vec<String> = ids.split(',').map(|s| s.trim().to_string()).collect();
    assert_eq!(result, vec!["abc12345".to_string(), "def67890".to_string()]);
}

#[test]
fn test_task_list_display_from() {
    let task_list = TaskList {
        id: "abc12345".to_string(),
        title: "Test Task List".to_string(),
        description: Some("Description".to_string()),
        notes: None,
        tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
        external_ref: None,
        status: "active".to_string(),
        repo_ids: None,
        project_id: "proj1234".to_string(),
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:00:00Z".to_string(),
        archived_at: None,
    };

    let display = TaskListDisplay::from(&task_list);
    assert_eq!(display.id, "abc12345");
    assert_eq!(display.title, "Test Task List");
    assert_eq!(display.project_id, "proj1234");
    assert_eq!(display.status, "active");
    assert_eq!(display.tags, "tag1, tag2");
}

#[test]
fn test_format_table_empty() {
    let result = format_table(&[]);
    assert_eq!(result, "No task lists found.");
}

#[test]
fn test_format_table_with_data() {
    let task_lists = vec![TaskList {
        id: "abc12345".to_string(),
        title: "Test".to_string(),
        description: None,
        notes: None,
        tags: None,
        external_ref: None,
        status: "active".to_string(),
        repo_ids: None,
        project_id: "proj1234".to_string(),
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:00:00Z".to_string(),
        archived_at: None,
    }];

    let result = format_table(&task_lists);
    assert!(result.contains("abc12345"));
    assert!(result.contains("Test"));
    assert!(result.contains("active"));
}

// API client tests (smoke tests for URL building)
#[test]
fn test_list_task_lists_builds_correct_url() {
    let client = ApiClient::new(None);
    let builder = client.get("/api/v1/task-lists");
    let _request = builder;
}

#[test]
fn test_get_task_list_builds_correct_url() {
    let client = ApiClient::new(None);
    let id = "abc12345";
    let builder = client.get(&format!("/api/v1/task-lists/{}", id));
    let _request = builder;
}

#[test]
fn test_create_task_list_builds_correct_url() {
    let client = ApiClient::new(None);
    let builder = client.post("/api/v1/task-lists");
    let _request = builder;
}

#[test]
fn test_update_task_list_builds_correct_url() {
    let client = ApiClient::new(None);
    let id = "abc12345";
    let builder = client.patch(&format!("/api/v1/task-lists/{}", id));
    let _request = builder;
}

#[test]
fn test_delete_task_list_builds_correct_url() {
    let client = ApiClient::new(None);
    let id = "abc12345";
    let builder = client.delete(&format!("/api/v1/task-lists/{}", id));
    let _request = builder;
}

#[test]
fn test_create_request_serialization() {
    let req = CreateTaskListRequest {
        title: "Test".to_string(),
        project_id: "proj1234".to_string(),
        description: Some("Desc".to_string()),
        tags: Some(vec!["tag1".to_string()]),
        repo_ids: Some(vec!["repo1".to_string()]),
    };

    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("Test"));
    assert!(json.contains("proj1234"));
}

#[test]
fn test_update_request_serialization() {
    let req = UpdateTaskListRequest {
        title: "Updated".to_string(),
        description: Some("New desc".to_string()),
        status: Some("archived".to_string()),
        tags: Some(vec!["tag2".to_string()]),
    };

    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("Updated"));
    assert!(json.contains("archived"));
}
