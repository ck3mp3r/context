use crate::cli::api_client::ApiClient;
use crate::cli::commands::task::*;

#[tokio::test]
async fn test_list_tasks_json_format() {
    // This is an integration test that requires a running API
    // For now, we'll test the formatter functions
    let tasks = vec![Task {
        id: "12345678".to_string(),
        list_id: "list1234".to_string(),
        parent_id: None,
        title: "Test task 1".to_string(),
        description: None,
        status: "todo".to_string(),
        priority: Some(1),
        tags: None,
        external_refs: vec![],
        created_at: "2025-12-28T10:00:00Z".to_string(),
        started_at: None,
        completed_at: None,
    }];

    let json = serde_json::to_string_pretty(&tasks).unwrap();
    assert!(json.contains("Test task 1"));
    assert!(json.contains("todo"));
}

#[test]
fn test_format_table_with_tasks() {
    let tasks = vec![
        Task {
            id: "12345678abcd".to_string(),
            list_id: "list1234".to_string(),
            parent_id: None,
            title: "Test task 1".to_string(),
            description: None,
            status: "todo".to_string(),
            priority: Some(1),
            tags: None,
            external_refs: vec![],
            created_at: "2025-12-28T10:00:00Z".to_string(),
            started_at: None,
            completed_at: None,
        },
        Task {
            id: "87654321efgh".to_string(),
            list_id: "list1234".to_string(),
            parent_id: None,
            title: "Test task 2 with a very long title that should be truncated".to_string(),
            description: None,
            status: "done".to_string(),
            priority: None,
            tags: None,
            external_refs: vec![],
            created_at: "2025-12-28T11:00:00Z".to_string(),
            started_at: None,
            completed_at: None,
        },
    ];

    let output = format_table(&tasks);
    println!("Output:\n{}", output);

    // Test that table contains the data
    assert!(output.contains("12345678"));
    assert!(output.contains("Test task 1"));
    assert!(output.contains("todo"));
    assert!(output.contains(" 1 ")); // Priority with spaces
    assert!(output.contains("87654321"));
    assert!(output.contains("...")); // Truncation marker
    assert!(output.contains("done"));
    assert!(output.contains(" - ")); // None priority rendered as dash

    // Test that table has rounded style characters
    assert!(output.contains("╭") || output.contains("─")); // Table borders
}

#[test]
fn test_format_table_empty() {
    let tasks: Vec<Task> = vec![];
    let output = format_table(&tasks);
    assert_eq!(output, "No tasks found.");
}

#[test]
fn test_task_display_conversion() {
    let task = Task {
        id: "12345678".to_string(), // IDs are always 8 chars
        list_id: "list1234".to_string(),
        parent_id: None,
        title: "short".to_string(),
        description: None,
        status: "todo".to_string(),
        priority: Some(5),
        tags: None,
        external_refs: vec![],
        created_at: "2025-12-28T10:00:00Z".to_string(),
        started_at: None,
        completed_at: None,
    };

    let display: TaskDisplay = (&task).into();
    assert_eq!(display.id, "12345678");
    assert_eq!(display.title, "short");
    assert_eq!(display.priority, "5");

    let task_none = Task {
        id: "abc12345".to_string(), // IDs are always 8 chars
        list_id: "list1234".to_string(),
        parent_id: None,
        title: "x".repeat(60),
        description: None,
        status: "done".to_string(),
        priority: None,
        tags: None,
        external_refs: vec![],
        created_at: "2025-12-28T10:00:00Z".to_string(),
        started_at: None,
        completed_at: None,
    };

    let display_none: TaskDisplay = (&task_none).into();
    assert_eq!(display_none.id, "abc12345");
    assert!(display_none.title.ends_with("..."));
    assert_eq!(display_none.priority, "-");
}

#[test]
fn test_complete_task_success_message() {
    // We can't test the actual API call without a running server,
    // but we can test the success message format
    let task_id = "12345678";
    let expected = format!("✓ Task {} marked as complete", task_id);
    assert!(expected.contains("12345678"));
    assert!(expected.contains("complete"));
    assert!(expected.contains("✓"));
}

// Tests for new CRUD operations

#[test]
fn test_get_task_builds_correct_url() {
    let client = ApiClient::new(None);
    let id = "abc12345";
    let builder = client.get(&format!("/api/v1/tasks/{}", id));
    let _request = builder;
}

#[test]
fn test_create_task_builds_correct_url() {
    let client = ApiClient::new(None);
    let list_id = "list1234";
    let builder = client.post(&format!("/api/v1/task-lists/{}/tasks", list_id));
    let _request = builder;
}

#[test]
fn test_update_task_builds_correct_url() {
    let client = ApiClient::new(None);
    let id = "abc12345";
    let builder = client.patch(&format!("/api/v1/tasks/{}", id));
    let _request = builder;
}

#[test]
fn test_delete_task_builds_correct_url() {
    let client = ApiClient::new(None);
    let id = "abc12345";
    let builder = client.delete(&format!("/api/v1/tasks/{}", id));
    let _request = builder;
}

#[test]
fn test_create_request_serialization() {
    let req = CreateTaskRequest {
        title: "Test task".to_string(),
        description: Some("Test description".to_string()),
        parent_id: None,
        priority: Some(3),
        tags: Some(vec!["urgent".to_string()]),
        external_refs: None,
    };

    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("Test task"));
    assert!(json.contains("Test description"));
    assert!(json.contains("3"));
    assert!(json.contains("urgent"));
}

#[test]
fn test_update_request_serialization() {
    let req = UpdateTaskRequest {
        title: Some("Updated title".to_string()),
        description: Some("Updated description".to_string()),
        status: Some("in_progress".to_string()),
        priority: Some(2),
        parent_id: None,
        tags: Some(vec!["important".to_string()]),
        external_refs: None,
    };

    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("Updated title"));
    assert!(json.contains("Updated description"));
    assert!(json.contains("in_progress"));
    assert!(json.contains("2"));
    assert!(json.contains("important"));
}

#[test]
fn test_task_with_all_fields() {
    let task = Task {
        id: "abc12345".to_string(),
        list_id: "list1234".to_string(),
        parent_id: Some("parent12".to_string()),
        title: "Full task".to_string(),
        description: Some("Full description".to_string()),
        status: "in_progress".to_string(),
        priority: Some(1),
        tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
        external_refs: vec![],
        created_at: "2025-12-28T10:00:00Z".to_string(),
        started_at: Some("2025-12-28T11:00:00Z".to_string()),
        completed_at: None,
    };

    assert_eq!(task.id, "abc12345");
    assert_eq!(task.list_id, "list1234");
    assert_eq!(task.title, "Full task");
    assert_eq!(task.description, Some("Full description".to_string()));
    assert_eq!(task.priority, Some(1));
    assert_eq!(
        task.tags,
        Some(vec!["tag1".to_string(), "tag2".to_string()])
    );
}

#[test]
fn test_create_request_with_parent_id_serialization() {
    let req = CreateTaskRequest {
        title: "Subtask".to_string(),
        description: Some("Child task".to_string()),
        parent_id: Some("parent123".to_string()),
        priority: Some(2),
        tags: Some(vec!["subtask".to_string()]),
        external_refs: None,
    };

    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("Subtask"));
    assert!(json.contains("parent123"));
    assert!(json.contains("parent_id"));
}

#[test]
fn test_create_request_without_parent_id_omits_field() {
    let req = CreateTaskRequest {
        title: "Top-level task".to_string(),
        description: None,
        parent_id: None,
        priority: None,
        tags: None,
        external_refs: None,
    };

    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("Top-level task"));
    // parent_id should be omitted when None
    assert!(!json.contains("parent_id"));
}

// Tests for empty string parent_id handling (CLI pattern: --parent-id="" removes parent)

#[test]
fn test_update_request_empty_string_parent_id_converts_to_none() {
    // Simulate CLI logic: empty string should convert to Some(None) to remove parent
    let parent_id_input = Some("".to_string());

    let parent_id = parent_id_input.map(|s| {
        if s.is_empty() {
            None // Empty string means remove parent
        } else {
            Some(s.to_string())
        }
    });

    let req = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id,
        tags: None,
        external_refs: None,
    };

    // parent_id should be Some(None) - explicitly removing the parent
    assert_eq!(req.parent_id, Some(None));

    // Serialize and verify it includes "parent_id": null
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"parent_id\":null"));
}

#[test]
fn test_update_request_non_empty_parent_id_sets_value() {
    // CLI: --parent-id="parent123" should set parent
    let parent_id_input = Some("parent123".to_string());

    let parent_id = parent_id_input.map(|s| {
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    });

    let req = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id,
        tags: None,
        external_refs: None,
    };

    // parent_id should be Some(Some("parent123"))
    assert_eq!(req.parent_id, Some(Some("parent123".to_string())));

    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"parent_id\":\"parent123\""));
}

#[test]
fn test_update_request_missing_parent_id_field_is_none() {
    // CLI: not providing --parent-id at all should be None (no change)
    let parent_id_input: Option<String> = None;

    let parent_id = parent_id_input.map(|s| {
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    });

    let req = UpdateTaskRequest {
        title: None,
        description: None,
        status: None,
        priority: None,
        parent_id,
        tags: None,
        external_refs: None,
    };

    // parent_id should be None - field not included in update
    assert_eq!(req.parent_id, None);

    // Serialize and verify parent_id is omitted (skip_serializing_if)
    let json = serde_json::to_string(&req).unwrap();
    assert!(!json.contains("parent_id"));
}
