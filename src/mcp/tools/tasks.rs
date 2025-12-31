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

use crate::db::{Database, PageSort, SortOrder, Task, TaskQuery, TaskRepository, TaskStatus};
use crate::mcp::tools::{apply_limit, map_db_error};

// =============================================================================
// Parameter Structs
// =============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListTasksParams {
    #[schemars(description = "TaskList ID to list tasks from")]
    pub list_id: String,
    #[schemars(
        description = "Filter by status: ['backlog'], ['todo'], ['in_progress'], ['review'], ['done'], ['cancelled']. Can combine: ['todo', 'in_progress'] for active tasks."
    )]
    pub status: Option<Vec<String>>,
    #[schemars(
        description = "Filter by parent task ID to list subtasks of a specific parent. Omit to return all tasks (both parents and subtasks). Use type='task' to filter only top-level tasks."
    )]
    pub parent_id: Option<String>,
    #[schemars(description = "Filter by tags to find tasks with specific labels.")]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "Filter by task type: 'task' (top-level only) or 'subtask' (only subtasks). Omit to return both tasks and subtasks (default). Examples: type='task' lists only parents (parent_id IS NULL), type='subtask' lists only subtasks (parent_id IS NOT NULL), type='subtask' with parent_id='xyz' lists subtasks of specific parent."
    )]
    #[serde(rename = "type")]
    pub task_type: Option<String>,
    #[schemars(description = "Maximum number of tasks to return (default: 10, max: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of items to skip (for pagination)")]
    pub offset: Option<usize>,
    #[schemars(
        description = "Field to sort by (title, status, priority, created_at, updated_at, completed_at)"
    )]
    pub sort: Option<String>,
    #[schemars(description = "Sort order (asc, desc)")]
    pub order: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetTaskParams {
    #[schemars(description = "Task ID")]
    pub task_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateTaskParams {
    #[schemars(
        description = "Task list ID this task belongs to. Use list_task_lists to find existing lists."
    )]
    pub list_id: String,
    #[schemars(description = "Task title (short summary)")]
    pub title: String,
    #[schemars(description = "Task description (detailed info, optional)")]
    pub description: Option<String>,
    #[schemars(description = "Priority: 1 (highest/urgent) to 5 (lowest/nice-to-have). Optional.")]
    pub priority: Option<i32>,
    #[schemars(
        description = "Parent task ID for subtasks. BEST PRACTICE: Only ONE level deep (subtasks should not have subtasks). Optional."
    )]
    pub parent_id: Option<String>,
    #[schemars(
        description = "Tags for categorization (e.g., 'bug', 'frontend', 'critical'). Optional."
    )]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateTaskParams {
    #[schemars(description = "Task ID to update")]
    pub task_id: String,
    #[schemars(description = "Task title (optional)")]
    pub title: Option<String>,
    #[schemars(description = "Task description (optional)")]
    pub description: Option<String>,
    #[schemars(
        description = "Status: 'backlog', 'todo', 'in_progress' (sets started_at), 'review', 'done' (sets completed_at), 'cancelled' (optional)"
    )]
    pub status: Option<String>,
    #[schemars(description = "Priority: 1 (highest) to 5 (lowest) (optional)")]
    pub priority: Option<i32>,
    #[schemars(description = "Tags (optional). Replaces all existing tags when provided.")]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "Parent task ID (optional). Set to change task hierarchy - convert to/from subtask. Set to empty string to remove parent."
    )]
    pub parent_id: Option<String>,
    #[schemars(
        description = "Move task to different list (optional). Use sparingly - tasks should stay in their original list."
    )]
    pub list_id: Option<String>,
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

    #[tool(
        description = "List tasks in a task list. Filter by status, parent_id (for subtasks), or tags. Sort by title, status, priority, created_at, updated_at, or completed_at (default: created_at). Use order='asc' or 'desc' (default: asc). Use this to see current work before adding new tasks."
    )]
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
                offset: params.0.offset,
                sort_by: params.0.sort.clone(),
                sort_order: match params.0.order.as_deref() {
                    Some("desc") => Some(SortOrder::Desc),
                    Some("asc") => Some(SortOrder::Asc),
                    _ => None,
                },
            },
            list_id: Some(params.0.list_id.clone()),
            status: status_str,
            parent_id: params.0.parent_id.clone(),
            tags: params.0.tags.clone(),
            task_type: params.0.task_type.clone(),
        };

        let result = self
            .db
            .tasks()
            .list(Some(&query))
            .await
            .map_err(map_db_error)?;

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

    #[tool(
        description = "Get a task by ID with full details including status, timestamps, and relationships."
    )]
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

    #[tool(
        description = "Create a new task (always status='backlog'). To change status: update_task. To complete: complete_task. For subtasks use parent_id (max ONE level deep)."
    )]
    pub async fn create_task(
        &self,
        params: Parameters<CreateTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        let task = Task {
            id: String::new(), // Will be generated by DB
            list_id: params.0.list_id.clone(),
            parent_id: params.0.parent_id.clone(),
            title: params.0.title.clone(),
            description: params.0.description.clone(),
            status: TaskStatus::Backlog, // Always create as backlog
            priority: params.0.priority,
            tags: params.0.tags.clone().unwrap_or_default(),
            created_at: None, // Will be set by DB
            started_at: None,
            completed_at: None,
            updated_at: None, // Will be set by DB
        };

        let created = self.db.tasks().create(&task).await.map_err(map_db_error)?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&created).unwrap(),
        )]))
    }

    #[tool(
        description = "Update task content, status, priority, parent_id (reparent task), tags, or move to different list. Status changes to 'in_progress' set started_at, changes to 'done' set completed_at. All fields optional."
    )]
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
        if let Some(title) = &params.0.title {
            task.title = title.clone();
        }
        if let Some(description) = &params.0.description {
            task.description = Some(description.clone());
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
        if let Some(parent_id) = &params.0.parent_id {
            // Empty string means remove parent (convert subtask to standalone)
            if parent_id.is_empty() {
                task.parent_id = None;
            } else {
                task.parent_id = Some(parent_id.clone());
            }
        }
        if let Some(list_id) = &params.0.list_id {
            task.list_id = list_id.clone();
        }

        self.db.tasks().update(&task).await.map_err(map_db_error)?;

        // Fetch updated task to get auto-set timestamps
        let updated = self
            .db
            .tasks()
            .get(&params.0.task_id)
            .await
            .map_err(map_db_error)?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&updated).unwrap(),
        )]))
    }

    #[tool(
        description = "Mark task as complete. Shortcut for update_task with status='done'. Sets completed_at timestamp automatically."
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

        self.db.tasks().update(&task).await.map_err(map_db_error)?;

        // Fetch updated task to get auto-set completed_at
        let completed = self
            .db
            .tasks()
            .get(&params.0.task_id)
            .await
            .map_err(map_db_error)?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&completed).unwrap(),
        )]))
    }

    #[tool(
        description = "Delete a task permanently. Consider setting status='cancelled' with update_task instead to preserve history."
    )]
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
