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
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub external_refs: Vec<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateTaskRequest {
    pub(crate) title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) external_refs: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) external_refs: Option<Vec<String>>,
}

#[derive(Tabled)]
pub(crate) struct TaskDisplay {
    #[tabled(rename = "ID")]
    pub(crate) id: String,
    #[tabled(rename = "Title")]
    pub(crate) title: String,
    #[tabled(rename = "Status")]
    pub(crate) status: String,
    #[tabled(rename = "Priority")]
    pub(crate) priority: String,
}

impl From<&Task> for TaskDisplay {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.clone(),
            title: truncate_with_ellipsis(&task.title, 50),
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
    let mut request = api_client.get(&format!("/api/v1/task-lists/{}/tasks", list_id));

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

pub(crate) fn format_table(tasks: &[Task]) -> String {
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
        .post(&format!("/api/v1/tasks/{}/complete", task_id))
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
    let response = api_client
        .get(&format!("/api/v1/tasks/{}", id))
        .send()
        .await?;

    let task: Task = ApiClient::handle_response(response).await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&task)?),
        _ => {
            use tabled::builder::Builder;

            let mut builder = Builder::default();
            builder.push_record(["Field", "Value"]);
            builder.push_record(["ID", &task.id]);
            builder.push_record(["List ID", &task.list_id]);
            if let Some(parent_id) = &task.parent_id {
                builder.push_record(["Parent ID", parent_id]);
            }
            builder.push_record(["Title", &task.title]);
            if let Some(desc) = &task.description {
                builder.push_record(["Description", desc]);
            }
            builder.push_record(["Status", &task.status]);
            builder.push_record([
                "Priority",
                &task
                    .priority
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ]);
            builder.push_record(["Tags", &format_tags(task.tags.as_ref())]);
            if !task.external_refs.is_empty() {
                builder.push_record(["External Refs", &task.external_refs.join(", ")]);
            }
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
    title: &str,
    description: Option<&str>,
    priority: Option<i32>,
    tags: Option<&str>,
    external_refs: Option<&str>,
    parent_id: Option<&str>,
) -> CliResult<String> {
    let request_body = CreateTaskRequest {
        title: title.to_string(),
        description: description.map(|s| s.to_string()),
        parent_id: parent_id.map(|s| s.to_string()),
        priority,
        tags: parse_tags(tags),
        external_refs: parse_tags(external_refs),
    };

    let response = api_client
        .post(&format!("/api/v1/task-lists/{}/tasks", list_id))
        .json(&request_body)
        .send()
        .await?;

    let task: Task = ApiClient::handle_response(response).await?;
    Ok(format!("✓ Created task: {} ({})", task.title, task.id))
}

/// Parameters for updating a task
pub struct UpdateTaskParams<'a> {
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
    pub status: Option<&'a str>,
    pub priority: Option<i32>,
    pub parent_id: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub external_refs: Option<&'a str>,
}

/// Update a task
pub async fn update_task(
    api_client: &ApiClient,
    id: &str,
    params: UpdateTaskParams<'_>,
) -> CliResult<String> {
    let request_body = UpdateTaskRequest {
        title: params.title.map(|s| s.to_string()),
        description: params.description.map(|s| s.to_string()),
        status: params.status.map(|s| s.to_string()),
        priority: params.priority,
        parent_id: params.parent_id.map(|s| {
            if s.is_empty() {
                None // Empty string means remove parent
            } else {
                Some(s.to_string())
            }
        }),
        tags: parse_tags(params.tags),
        external_refs: parse_tags(params.external_refs),
    };

    let response = api_client
        .patch(&format!("/api/v1/tasks/{}", id))
        .json(&request_body)
        .send()
        .await?;

    let task: Task = ApiClient::handle_response(response).await?;
    Ok(format!("✓ Updated task: {} ({})", task.title, task.id))
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
        .delete(&format!("/api/v1/tasks/{}", id))
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
