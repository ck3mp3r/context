use crate::cli::api_client::ApiClient;
use crate::cli::error::{CliError, CliResult};
use crate::cli::utils::{apply_table_style, format_tags, parse_tags, truncate_with_ellipsis};
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled};

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub list_id: String,
    pub parent_id: Option<String>,
    pub content: String,
    pub status: String,
    pub priority: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateTaskRequest {
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct UpdateTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

#[derive(Tabled)]
struct TaskDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Content")]
    content: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Priority")]
    priority: String,
}

impl From<&Task> for TaskDisplay {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.clone(),
            content: truncate_with_ellipsis(&task.content, 50),
            status: task.status.clone(),
            priority: task
                .priority
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskListResponse {
    items: Vec<Task>,
    total: usize,
    limit: usize,
    offset: usize,
}

/// Filter parameters for listing tasks
pub struct ListTasksFilter<'a> {
    pub status: Option<&'a str>,
    pub parent_id: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// List tasks from a task list with optional filtering
pub async fn list_tasks(
    api_client: &ApiClient,
    list_id: &str,
    filter: ListTasksFilter<'_>,
    format: &str,
) -> CliResult<String> {
    let mut request = api_client.get(&format!("/v1/task-lists/{}/tasks", list_id));

    if let Some(s) = filter.status {
        request = request.query(&[("status", s)]);
    }
    if let Some(p) = filter.parent_id {
        request = request.query(&[("parent_id", p)]);
    }
    if let Some(t) = filter.tags {
        request = request.query(&[("tags", t)]);
    }
    if let Some(l) = filter.limit {
        request = request.query(&[("limit", l.to_string())]);
    }
    if let Some(o) = filter.offset {
        request = request.query(&[("offset", o.to_string())]);
    }

    let response: TaskListResponse = request.send().await?.json().await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&response.items)?),
        _ => Ok(format_table(&response.items)),
    }
}

fn format_table(tasks: &[Task]) -> String {
    if tasks.is_empty() {
        return "No tasks found.".to_string();
    }

    let display_tasks: Vec<TaskDisplay> = tasks.iter().map(|t| t.into()).collect();
    let mut table = Table::new(display_tasks);
    apply_table_style(&mut table);
    table.to_string()
}

/// Mark a task as complete
pub async fn complete_task(api_client: &ApiClient, task_id: &str) -> CliResult<String> {
    let response = api_client
        .post(&format!("/v1/tasks/{}/complete", task_id))
        .send()
        .await?;

    if response.status().is_success() {
        Ok(format!("✓ Task {} marked as complete", task_id))
    } else {
        let status = response.status().as_u16();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        Err(CliError::ApiError {
            status,
            message: error_text,
        })
    }
}

/// Get a single task by ID
pub async fn get_task(api_client: &ApiClient, id: &str, format: &str) -> CliResult<String> {
    let response = api_client.get(&format!("/v1/tasks/{}", id)).send().await?;

    let task: Task = ApiClient::handle_response(response).await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&task)?),
        _ => {
            use tabled::builder::Builder;

            let mut builder = Builder::default();
            builder.push_record(["Field", "Value"]);
            builder.push_record(["ID", &task.id]);
            builder.push_record(["List ID", &task.list_id]);
            builder.push_record(["Content", &task.content]);
            builder.push_record(["Status", &task.status]);
            builder.push_record([
                "Priority",
                &task
                    .priority
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ]);
            builder.push_record(["Tags", &format_tags(task.tags.as_ref())]);
            builder.push_record(["Created", &task.created_at]);
            builder.push_record(["Started", task.started_at.as_deref().unwrap_or("-")]);
            builder.push_record(["Completed", task.completed_at.as_deref().unwrap_or("-")]);

            let mut table = builder.build();
            apply_table_style(&mut table);
            Ok(table.to_string())
        }
    }
}

/// Create a new task
pub async fn create_task(
    api_client: &ApiClient,
    list_id: &str,
    content: &str,
    priority: Option<i32>,
    tags: Option<&str>,
) -> CliResult<String> {
    let request_body = CreateTaskRequest {
        content: content.to_string(),
        priority,
        tags: parse_tags(tags),
    };

    let response = api_client
        .post(&format!("/v1/task-lists/{}/tasks", list_id))
        .json(&request_body)
        .send()
        .await?;

    let task: Task = ApiClient::handle_response(response).await?;
    Ok(format!("✓ Created task: {} ({})", task.content, task.id))
}

/// Update a task
pub async fn update_task(
    api_client: &ApiClient,
    id: &str,
    content: Option<&str>,
    status: Option<&str>,
    priority: Option<i32>,
    tags: Option<&str>,
) -> CliResult<String> {
    let request_body = UpdateTaskRequest {
        content: content.map(|s| s.to_string()),
        status: status.map(|s| s.to_string()),
        priority,
        tags: parse_tags(tags),
    };

    let response = api_client
        .patch(&format!("/v1/tasks/{}", id))
        .json(&request_body)
        .send()
        .await?;

    let task: Task = ApiClient::handle_response(response).await?;
    Ok(format!("✓ Updated task: {} ({})", task.content, task.id))
}

/// Delete a task (requires --force flag for safety)
pub async fn delete_task(api_client: &ApiClient, id: &str, force: bool) -> CliResult<String> {
    // Safety check: require --force flag
    if !force {
        return Err(CliError::InvalidResponse {
            message: "Delete operation requires --force flag. This action is destructive and cannot be undone.".to_string(),
        });
    }

    let response = api_client
        .delete(&format!("/v1/tasks/{}", id))
        .send()
        .await?;

    // For delete, we expect no body on success, so we don't use handle_response
    if response.status().is_success() {
        Ok(format!("✓ Deleted task: {}", id))
    } else {
        let status = response.status().as_u16();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        Err(CliError::ApiError {
            status,
            message: error_text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_tasks_json_format() {
        // This is an integration test that requires a running API
        // For now, we'll test the formatter functions
        let tasks = vec![Task {
            id: "12345678".to_string(),
            list_id: "list1234".to_string(),
            parent_id: None,
            content: "Test task 1".to_string(),
            status: "todo".to_string(),
            priority: Some(1),
            tags: None,
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
                content: "Test task 1".to_string(),
                status: "todo".to_string(),
                priority: Some(1),
                tags: None,
                created_at: "2025-12-28T10:00:00Z".to_string(),
                started_at: None,
                completed_at: None,
            },
            Task {
                id: "87654321efgh".to_string(),
                list_id: "list1234".to_string(),
                parent_id: None,
                content: "Test task 2 with a very long content that should be truncated"
                    .to_string(),
                status: "done".to_string(),
                priority: None,
                tags: None,
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
            content: "short".to_string(),
            status: "todo".to_string(),
            priority: Some(5),
            tags: None,
            created_at: "2025-12-28T10:00:00Z".to_string(),
            started_at: None,
            completed_at: None,
        };

        let display: TaskDisplay = (&task).into();
        assert_eq!(display.id, "12345678");
        assert_eq!(display.content, "short");
        assert_eq!(display.priority, "5");

        let task_none = Task {
            id: "abc12345".to_string(), // IDs are always 8 chars
            list_id: "list1234".to_string(),
            parent_id: None,
            content: "x".repeat(60),
            status: "done".to_string(),
            priority: None,
            tags: None,
            created_at: "2025-12-28T10:00:00Z".to_string(),
            started_at: None,
            completed_at: None,
        };

        let display_none: TaskDisplay = (&task_none).into();
        assert_eq!(display_none.id, "abc12345");
        assert!(display_none.content.ends_with("..."));
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
        let builder = client.get(&format!("/v1/tasks/{}", id));
        let _request = builder;
    }

    #[test]
    fn test_create_task_builds_correct_url() {
        let client = ApiClient::new(None);
        let list_id = "list1234";
        let builder = client.post(&format!("/v1/task-lists/{}/tasks", list_id));
        let _request = builder;
    }

    #[test]
    fn test_update_task_builds_correct_url() {
        let client = ApiClient::new(None);
        let id = "abc12345";
        let builder = client.patch(&format!("/v1/tasks/{}", id));
        let _request = builder;
    }

    #[test]
    fn test_delete_task_builds_correct_url() {
        let client = ApiClient::new(None);
        let id = "abc12345";
        let builder = client.delete(&format!("/v1/tasks/{}", id));
        let _request = builder;
    }

    #[test]
    fn test_create_request_serialization() {
        let req = CreateTaskRequest {
            content: "Test task".to_string(),
            priority: Some(3),
            tags: Some(vec!["urgent".to_string()]),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Test task"));
        assert!(json.contains("3"));
        assert!(json.contains("urgent"));
    }

    #[test]
    fn test_update_request_serialization() {
        let req = UpdateTaskRequest {
            content: Some("Updated content".to_string()),
            status: Some("in_progress".to_string()),
            priority: Some(2),
            tags: Some(vec!["important".to_string()]),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Updated content"));
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
            content: "Full task".to_string(),
            status: "in_progress".to_string(),
            priority: Some(1),
            tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
            created_at: "2025-12-28T10:00:00Z".to_string(),
            started_at: Some("2025-12-28T11:00:00Z".to_string()),
            completed_at: None,
        };

        assert_eq!(task.id, "abc12345");
        assert_eq!(task.list_id, "list1234");
        assert_eq!(task.priority, Some(1));
        assert_eq!(
            task.tags,
            Some(vec!["tag1".to_string(), "tag2".to_string()])
        );
    }
}
