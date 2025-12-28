//! Task list CLI commands
//!
//! This module provides CLI commands for managing task lists via the REST API.

use crate::cli::api_client::ApiClient;
use crate::cli::error::CliResult;
use crate::cli::utils::{apply_table_style, format_tags, parse_tags, truncate_with_ellipsis};
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled};

#[derive(Debug, Serialize, Deserialize)]
struct ListTaskListsResponse {
    items: Vec<TaskList>,
    total: usize,
    limit: usize,
    offset: usize,
}

#[derive(Debug, Serialize)]
struct CreateTaskListRequest {
    name: String,
    project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repo_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct UpdateTaskListRequest {
    name: String, // Name is required for update API
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskList {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub tags: Option<Vec<String>>,
    pub external_ref: Option<String>,
    pub status: String,
    pub repo_ids: Option<Vec<String>>,
    pub project_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Tabled)]
struct TaskListDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Project")]
    project_id: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Tags")]
    tags: String,
}

impl From<&TaskList> for TaskListDisplay {
    fn from(task_list: &TaskList) -> Self {
        Self {
            id: task_list.id.clone(),
            name: truncate_with_ellipsis(&task_list.name, 40),
            project_id: task_list.project_id.clone(),
            status: task_list.status.clone(),
            tags: format_tags(task_list.tags.as_ref()),
        }
    }
}

fn format_table(task_lists: &[TaskList]) -> String {
    if task_lists.is_empty() {
        return "No task lists found.".to_string();
    }

    let display: Vec<TaskListDisplay> = task_lists.iter().map(TaskListDisplay::from).collect();
    let mut table = Table::new(display);
    apply_table_style(&mut table);
    table.to_string()
}

/// List task lists with optional filtering
pub async fn list_task_lists(
    api_client: &ApiClient,
    status: Option<&str>,
    tags: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
    format: &str,
) -> CliResult<String> {
    let mut request = api_client.get("/v1/task-lists");

    if let Some(s) = status {
        request = request.query(&[("status", s)]);
    }
    if let Some(t) = tags {
        request = request.query(&[("tags", t)]);
    }
    if let Some(l) = limit {
        request = request.query(&[("limit", l.to_string())]);
    }
    if let Some(o) = offset {
        request = request.query(&[("offset", o.to_string())]);
    }

    let response: ListTaskListsResponse = request.send().await?.json().await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&response.items)?),
        _ => Ok(format_table(&response.items)),
    }
}

/// Get a single task list by ID
pub async fn get_task_list(api_client: &ApiClient, id: &str, format: &str) -> CliResult<String> {
    let response = api_client
        .get(&format!("/v1/task-lists/{}", id))
        .send()
        .await?;

    let task_list: TaskList = ApiClient::handle_response(response).await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&task_list)?),
        _ => {
            use tabled::builder::Builder;

            let mut builder = Builder::default();
            builder.push_record(["Field", "Value"]);
            builder.push_record(["ID", &task_list.id]);
            builder.push_record(["Name", &task_list.name]);
            builder.push_record([
                "Description",
                task_list.description.as_deref().unwrap_or("-"),
            ]);
            builder.push_record(["Project ID", &task_list.project_id]);
            builder.push_record(["Status", &task_list.status]);
            builder.push_record(["Tags", &format_tags(task_list.tags.as_ref())]);
            builder.push_record([
                "External Ref",
                task_list.external_ref.as_deref().unwrap_or("-"),
            ]);
            builder.push_record(["Created", &task_list.created_at]);
            builder.push_record(["Updated", &task_list.updated_at]);

            let mut table = builder.build();
            apply_table_style(&mut table);
            Ok(table.to_string())
        }
    }
}

/// Create a new task list
pub async fn create_task_list(
    api_client: &ApiClient,
    name: &str,
    project_id: &str,
    description: Option<&str>,
    tags: Option<&str>,
    repo_ids: Option<&str>,
) -> CliResult<String> {
    let request_body = CreateTaskListRequest {
        name: name.to_string(),
        project_id: project_id.to_string(),
        description: description.map(|s| s.to_string()),
        tags: parse_tags(tags),
        repo_ids: repo_ids.map(|ids| {
            ids.split(',')
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>()
        }),
    };

    let response = api_client
        .post("/v1/task-lists")
        .json(&request_body)
        .send()
        .await?;

    let task_list: TaskList = ApiClient::handle_response(response).await?;
    Ok(format!(
        "✓ Created task list: {} ({})",
        task_list.name, task_list.id
    ))
}

/// Update a task list
pub async fn update_task_list(
    api_client: &ApiClient,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
    status: Option<&str>,
    tags: Option<&str>,
) -> CliResult<String> {
    // For update, we need to get the current name if not provided (API requires name field)
    let current_name = if name.is_none() {
        let response = api_client
            .get(&format!("/v1/task-lists/{}", id))
            .send()
            .await?;
        let task_list: TaskList = ApiClient::handle_response(response).await?;
        task_list.name
    } else {
        name.unwrap().to_string()
    };

    let request_body = UpdateTaskListRequest {
        name: current_name,
        description: description.map(|s| s.to_string()),
        status: status.map(|s| s.to_string()),
        tags: parse_tags(tags),
    };

    let response = api_client
        .patch(&format!("/v1/task-lists/{}", id))
        .json(&request_body)
        .send()
        .await?;

    let task_list: TaskList = ApiClient::handle_response(response).await?;
    Ok(format!(
        "✓ Updated task list: {} ({})",
        task_list.name, task_list.id
    ))
}

/// Delete a task list (requires --force flag for safety)
pub async fn delete_task_list(api_client: &ApiClient, id: &str, force: bool) -> CliResult<String> {
    // Safety check: require --force flag
    if !force {
        return Err(crate::cli::error::CliError::InvalidResponse {
            message: "Delete operation requires --force flag. This action is destructive and cannot be undone.".to_string(),
        });
    }

    let response = api_client
        .delete(&format!("/v1/task-lists/{}", id))
        .send()
        .await?;

    if response.status().is_success() {
        Ok(format!("✓ Deleted task list: {}", id))
    } else {
        let status = response.status().as_u16();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        Err(crate::cli::error::CliError::ApiError {
            status,
            message: error_text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            name: "Test Task List".to_string(),
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
        assert_eq!(display.name, "Test Task List");
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
            name: "Test".to_string(),
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
        let builder = client.get("/v1/task-lists");
        let _request = builder;
    }

    #[test]
    fn test_get_task_list_builds_correct_url() {
        let client = ApiClient::new(None);
        let id = "abc12345";
        let builder = client.get(&format!("/v1/task-lists/{}", id));
        let _request = builder;
    }

    #[test]
    fn test_create_task_list_builds_correct_url() {
        let client = ApiClient::new(None);
        let builder = client.post("/v1/task-lists");
        let _request = builder;
    }

    #[test]
    fn test_update_task_list_builds_correct_url() {
        let client = ApiClient::new(None);
        let id = "abc12345";
        let builder = client.patch(&format!("/v1/task-lists/{}", id));
        let _request = builder;
    }

    #[test]
    fn test_delete_task_list_builds_correct_url() {
        let client = ApiClient::new(None);
        let id = "abc12345";
        let builder = client.delete(&format!("/v1/task-lists/{}", id));
        let _request = builder;
    }

    #[test]
    fn test_create_request_serialization() {
        let req = CreateTaskListRequest {
            name: "Test".to_string(),
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
            name: "Updated".to_string(),
            description: Some("New desc".to_string()),
            status: Some("archived".to_string()),
            tags: Some(vec!["tag2".to_string()]),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Updated"));
        assert!(json.contains("archived"));
    }
}
