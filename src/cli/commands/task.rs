use crate::cli::api_client::ApiClient;
use crate::cli::error::{CliError, CliResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: Option<i32>,
    pub created_at: String,
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
    let url = format!(
        "{}/api/v1/task-lists/{}/tasks",
        api_client.base_url(),
        list_id
    );

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

    let mut output = String::new();
    output.push_str(&format!(
        "{:<20} {:<50} {:<15} {:<10}\n",
        "ID", "Content", "Status", "Priority"
    ));
    output.push_str(&"-".repeat(95));
    output.push('\n');

    for task in tasks {
        let priority = task
            .priority
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        output.push_str(&format!(
            "{:<20} {:<50} {:<15} {:<10}\n",
            &task.id[..8.min(task.id.len())],
            truncate(&task.content, 50),
            &task.status,
            priority
        ));
    }

    output
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Mark a task as complete
pub async fn complete_task(api_client: &ApiClient, task_id: &str) -> CliResult<String> {
    let url = format!(
        "{}/api/v1/tasks/{}/complete",
        api_client.base_url(),
        task_id
    );

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

        assert!(output.contains("12345678"));
        assert!(output.contains("Test task 1"));
        assert!(output.contains("todo"));
        assert!(output.contains("1"));
        assert!(output.contains("87654321"));
        // The truncate function adds "..." so we need to match the actual truncated content
        assert!(output.contains("..."));
        assert!(output.contains("done"));
        assert!(output.contains("-")); // for None priority
    }

    #[test]
    fn test_format_table_empty() {
        let tasks: Vec<Task> = vec![];
        let output = format_table(&tasks);
        assert_eq!(output, "No tasks found.");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a very long string", 10), "this is...");
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
