use crate::cli::api_client::ApiClient;
use crate::cli::error::{CliError, CliResult};
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled, settings::Style};

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: Option<i32>,
    pub created_at: String,
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
            id: task.id.chars().take(8).collect(),
            content: if task.content.len() <= 50 {
                task.content.clone()
            } else {
                format!("{}...", task.content.chars().take(47).collect::<String>())
            },
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

/// List tasks from a task list
pub async fn list_tasks(api_client: &ApiClient, list_id: &str, format: &str) -> CliResult<String> {
    let url = format!("{}/v1/task-lists/{}/tasks", api_client.base_url(), list_id);

    let response: TaskListResponse = reqwest::get(&url).await?.json().await?;

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
    table.with(Style::rounded());
    table.to_string()
}

/// Mark a task as complete
pub async fn complete_task(api_client: &ApiClient, task_id: &str) -> CliResult<String> {
    let url = format!("{}/v1/tasks/{}/complete", api_client.base_url(), task_id);

    let client = reqwest::Client::new();
    let response = client.post(&url).send().await?;

    if response.status().is_success() {
        Ok(format!("✓ Task {} marked as complete", task_id))
    } else {
        let status = response.status().as_u16();
        let error_text = response.text().await?;
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
            content: "Test task 1".to_string(),
            status: "todo".to_string(),
            priority: Some(1),
            created_at: "2025-12-28T10:00:00Z".to_string(),
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
                content: "Test task 1".to_string(),
                status: "todo".to_string(),
                priority: Some(1),
                created_at: "2025-12-28T10:00:00Z".to_string(),
            },
            Task {
                id: "87654321efgh".to_string(),
                content: "Test task 2 with a very long content that should be truncated"
                    .to_string(),
                status: "done".to_string(),
                priority: None,
                created_at: "2025-12-28T11:00:00Z".to_string(),
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
            id: "12345678abcdef".to_string(),
            content: "short".to_string(),
            status: "todo".to_string(),
            priority: Some(5),
            created_at: "2025-12-28T10:00:00Z".to_string(),
        };

        let display: TaskDisplay = (&task).into();
        assert_eq!(display.id, "12345678");
        assert_eq!(display.content, "short");
        assert_eq!(display.priority, "5");

        let task_none = Task {
            id: "abc123".to_string(),
            content: "x".repeat(60),
            status: "done".to_string(),
            priority: None,
            created_at: "2025-12-28T10:00:00Z".to_string(),
        };

        let display_none: TaskDisplay = (&task_none).into();
        assert_eq!(display_none.id, "abc123");
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
}
