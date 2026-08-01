//! Task list CLI commands
//!
//! This module provides CLI commands for managing task lists via the REST API.

use crate::cli::api_client::ApiClient;
use crate::cli::commands::PageParams;
use crate::cli::error::CliResult;
use crate::cli::utils::{apply_table_style, format_tags, truncate_with_ellipsis};
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
pub struct CreateTaskListRequest {
    pub title: String,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct UpdateTaskListRequest {
    pub title: String, // Title is required for update API
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskList {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub tags: Option<Vec<String>>,
    pub external_refs: Vec<String>,
    pub status: String,
    pub repo_ids: Option<Vec<String>>,
    pub project_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Tabled)]
pub(crate) struct TaskListDisplay {
    #[tabled(rename = "ID")]
    pub(crate) id: String,
    #[tabled(rename = "Title")]
    pub(crate) title: String,
    #[tabled(rename = "Project")]
    pub(crate) project_id: String,
    #[tabled(rename = "Status")]
    pub(crate) status: String,
    #[tabled(rename = "Tags")]
    pub(crate) tags: String,
}

impl From<&TaskList> for TaskListDisplay {
    fn from(task_list: &TaskList) -> Self {
        Self {
            id: task_list.id.clone(),
            title: truncate_with_ellipsis(&task_list.title, 40),
            project_id: task_list.project_id.clone(),
            status: task_list.status.clone(),
            tags: format_tags(task_list.tags.as_ref()),
        }
    }
}

pub(crate) fn format_table(task_lists: &[TaskList]) -> String {
    if task_lists.is_empty() {
        return "No task lists found.".to_string();
    }

    let display: Vec<TaskListDisplay> = task_lists.iter().map(TaskListDisplay::from).collect();
    let mut table = Table::new(display);
    apply_table_style(&mut table);
    table.to_string()
}

/// List task lists with optional filtering
#[allow(clippy::too_many_arguments)]
pub async fn list_task_lists(
    api_client: &ApiClient,
    query: Option<&str>,
    project_id: Option<&str>,
    status: Option<&str>,
    tags: Option<&str>,
    page: PageParams<'_>,
    format: &str,
) -> CliResult<String> {
    let mut request = api_client.get("/api/v1/task-lists");

    if let Some(q) = query {
        request = request.query(&[("q", q)]);
    }
    if let Some(p) = project_id {
        request = request.query(&[("project_id", p)]);
    }
    if let Some(s) = status {
        request = request.query(&[("status", s)]);
    }
    if let Some(t) = tags {
        request = request.query(&[("tags", t)]);
    }
    if let Some(l) = page.limit {
        request = request.query(&[("limit", l.to_string())]);
    }
    if let Some(o) = page.offset {
        request = request.query(&[("offset", o.to_string())]);
    }
    if let Some(s) = page.sort {
        request = request.query(&[("sort", s)]);
    }
    if let Some(ord) = page.order {
        request = request.query(&[("order", ord)]);
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
        .get(&format!("/api/v1/task-lists/{}", id))
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
            builder.push_record(["Title", &task_list.title]);
            builder.push_record([
                "Description",
                task_list.description.as_deref().unwrap_or("-"),
            ]);
            builder.push_record(["Project ID", &task_list.project_id]);
            builder.push_record(["Status", &task_list.status]);
            builder.push_record(["Tags", &format_tags(task_list.tags.as_ref())]);
            let external_refs_str = if task_list.external_refs.is_empty() {
                "-".to_string()
            } else {
                task_list.external_refs.join(", ")
            };
            builder.push_record(["External Refs", &external_refs_str]);
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
    request: CreateTaskListRequest,
) -> CliResult<String> {
    let response = api_client
        .post("/api/v1/task-lists")
        .json(&request)
        .send()
        .await?;

    let task_list: TaskList = ApiClient::handle_response(response).await?;
    Ok(format!(
        "✓ Created task list: {} ({})",
        task_list.title, task_list.id
    ))
}

/// Update a task list
pub async fn update_task_list(
    api_client: &ApiClient,
    id: &str,
    mut request: UpdateTaskListRequest,
) -> CliResult<String> {
    // For update, if title is empty, we need to fetch the current title (API requires title field)
    if request.title.is_empty() {
        let response = api_client
            .get(&format!("/api/v1/task-lists/{}", id))
            .send()
            .await?;
        let task_list: TaskList = ApiClient::handle_response(response).await?;
        request.title = task_list.title;
    }

    let response = api_client
        .patch(&format!("/api/v1/task-lists/{}", id))
        .json(&request)
        .send()
        .await?;

    let task_list: TaskList = ApiClient::handle_response(response).await?;
    Ok(format!(
        "✓ Updated task list: {} ({})",
        task_list.title, task_list.id
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
        .delete(&format!("/api/v1/task-lists/{}", id))
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

/// Get task statistics for a task list
pub async fn get_task_list_stats(
    api_client: &ApiClient,
    id: &str,
    format: &str,
) -> CliResult<String> {
    let response = api_client
        .get(&format!("/api/v1/task-lists/{}/stats", id))
        .send()
        .await?;

    let stats: serde_json::Value = ApiClient::handle_response(response).await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&stats)?),
        _ => {
            use tabled::builder::Builder;

            let mut builder = Builder::default();
            builder.push_record(["Metric", "Count"]);
            builder.push_record(["Total", &stats["total"].to_string()]);
            builder.push_record(["Backlog", &stats["backlog"].to_string()]);
            builder.push_record(["Todo", &stats["todo"].to_string()]);
            builder.push_record(["In Progress", &stats["in_progress"].to_string()]);
            builder.push_record(["Review", &stats["review"].to_string()]);
            builder.push_record(["Done", &stats["done"].to_string()]);
            builder.push_record(["Cancelled", &stats["cancelled"].to_string()]);

            let mut table = builder.build();
            crate::cli::utils::apply_table_style(&mut table);
            Ok(table.to_string())
        }
    }
}
