//! MCP tools for Task management.

use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    schemars::JsonSchema,
    tool, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::db::{Database, PageSort, Task, TaskQuery, TaskRepository, TaskStatus};
use crate::mcp::tools::{apply_limit, map_db_error};

// =============================================================================
// Parameter Structs
// =============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListTasksParams {
    #[schemars(description = "TaskList ID to list tasks from")]
    pub list_id: String,
    #[schemars(
        description = "Filter by status (backlog, todo, in_progress, review, done, cancelled) - comma-separated"
    )]
    pub status: Option<Vec<String>>,
    #[schemars(description = "Filter by parent task ID (for subtasks)")]
    pub parent_id: Option<String>,
    #[schemars(description = "Filter by tags - comma-separated")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Maximum number of tasks to return (default: 10, max: 20)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetTaskParams {
    #[schemars(description = "Task ID")]
    pub task_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateTaskParams {
    #[schemars(description = "TaskList ID this task belongs to")]
    pub list_id: String,
    #[schemars(description = "Task content/description")]
    pub content: String,
    #[schemars(description = "Task status (defaults to backlog)")]
    pub status: Option<String>,
    #[schemars(description = "Priority level (1-5, where 1 is highest)")]
    pub priority: Option<i32>,
    #[schemars(description = "Parent task ID (for subtasks)")]
    pub parent_id: Option<String>,
    #[schemars(description = "Tags for organization (optional)")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateTaskParams {
    #[schemars(description = "Task ID to update")]
    pub task_id: String,
    #[schemars(description = "Task content/description (optional)")]
    pub content: Option<String>,
    #[schemars(description = "Task status (optional)")]
    pub status: Option<String>,
    #[schemars(description = "Priority level (1-5) (optional)")]
    pub priority: Option<i32>,
    #[schemars(description = "Tags for organization (optional)")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CompleteTaskParams {
    #[schemars(description = "Task ID to mark as complete")]
    pub task_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteTaskParams {
    #[schemars(description = "Task ID to delete")]
    pub task_id: String,
}

// =============================================================================
// Task Tools
// =============================================================================

#[derive(Clone)]
pub struct TaskTools<D: Database> {
    db: Arc<D>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> TaskTools<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    /// Get the tool router for this handler
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    #[tool(description = "List tasks for a task list with optional filtering")]
    pub async fn list_tasks(
        &self,
        params: Parameters<ListTasksParams>,
    ) -> Result<CallToolResult, McpError> {
        // Convert status Vec to comma-separated string if provided
        let status_str = params.0.status.as_ref().map(|statuses| statuses.join(","));

        // Build query
        let query = TaskQuery {
            page: PageSort {
                limit: Some(apply_limit(params.0.limit)),
                offset: None,
                sort_by: None,
                sort_order: None,
            },
            list_id: Some(params.0.list_id.clone()),
            status: status_str,
            parent_id: params.0.parent_id.clone(),
            tags: params.0.tags.clone(),
        };

        let result = self
            .db
            .tasks()
            .list(Some(&query))
            .await
            .map_err(|e| map_db_error(e))?;

        let response = json!({
            "items": result.items,
            "total": result.total,
            "limit": result.limit,
            "offset": result.offset,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap(),
        )]))
    }

    #[tool(description = "Get a task by ID")]
    pub async fn get_task(
        &self,
        params: Parameters<GetTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        let task = self.db.tasks().get(&params.0.task_id).await.map_err(|e| {
            McpError::resource_not_found(
                "task_not_found",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&task).unwrap(),
        )]))
    }

    #[tool(description = "Create a new task")]
    pub async fn create_task(
        &self,
        params: Parameters<CreateTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        // Parse status
        let status = if let Some(s) = &params.0.status {
            s.parse::<TaskStatus>().map_err(|e| {
                McpError::invalid_params("invalid_status", Some(serde_json::json!({"error": e})))
            })?
        } else {
            TaskStatus::Backlog
        };

        let task = Task {
            id: String::new(), // Will be generated by DB
            list_id: params.0.list_id.clone(),
            parent_id: params.0.parent_id.clone(),
            content: params.0.content.clone(),
            status,
            priority: params.0.priority,
            tags: params.0.tags.clone().unwrap_or_default(),
            created_at: String::new(), // Will be set by DB
            started_at: None,
            completed_at: None,
        };

        let created = self
            .db
            .tasks()
            .create(&task)
            .await
            .map_err(|e| map_db_error(e))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&created).unwrap(),
        )]))
    }

    #[tool(description = "Update an existing task")]
    pub async fn update_task(
        &self,
        params: Parameters<UpdateTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        // Get existing task
        let mut task = self.db.tasks().get(&params.0.task_id).await.map_err(|e| {
            McpError::resource_not_found(
                "task_not_found",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Update fields
        if let Some(content) = &params.0.content {
            task.content = content.clone();
        }
        if let Some(status_str) = &params.0.status {
            task.status = status_str.parse::<TaskStatus>().map_err(|e| {
                McpError::invalid_params("invalid_status", Some(serde_json::json!({"error": e})))
            })?;
        }
        if let Some(priority) = params.0.priority {
            task.priority = Some(priority);
        }
        if let Some(tags) = &params.0.tags {
            task.tags = tags.clone();
        }

        self.db
            .tasks()
            .update(&task)
            .await
            .map_err(|e| map_db_error(e))?;

        // Fetch updated task to get auto-set timestamps
        let updated = self
            .db
            .tasks()
            .get(&params.0.task_id)
            .await
            .map_err(|e| map_db_error(e))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&updated).unwrap(),
        )]))
    }

    #[tool(
        description = "Mark a task as complete (sets status to done and completed_at timestamp)"
    )]
    pub async fn complete_task(
        &self,
        params: Parameters<CompleteTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        // Get existing task
        let mut task = self.db.tasks().get(&params.0.task_id).await.map_err(|e| {
            McpError::resource_not_found(
                "task_not_found",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Set status to done
        task.status = TaskStatus::Done;

        self.db
            .tasks()
            .update(&task)
            .await
            .map_err(|e| map_db_error(e))?;

        // Fetch updated task to get auto-set completed_at
        let completed = self
            .db
            .tasks()
            .get(&params.0.task_id)
            .await
            .map_err(|e| map_db_error(e))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&completed).unwrap(),
        )]))
    }

    #[tool(description = "Delete a task")]
    pub async fn delete_task(
        &self,
        params: Parameters<DeleteTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .tasks()
            .delete(&params.0.task_id)
            .await
            .map_err(|e| {
                McpError::resource_not_found(
                    "task_not_found",
                    Some(serde_json::json!({"error": e.to_string()})),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Task {} deleted successfully",
            params.0.task_id
        ))]))
    }
}
