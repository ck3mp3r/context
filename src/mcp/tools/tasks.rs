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

use crate::api::notifier::{ChangeNotifier, UpdateMessage};
use crate::db::{Database, PageSort, SortOrder, Task, TaskQuery, TaskRepository, TaskStatus};
use crate::mcp::tools::{apply_limit, map_db_error};

// =============================================================================
// Validation Helpers
// =============================================================================

/// Validates that priority is within the valid range (1-5).
fn validate_priority(priority: Option<i32>) -> Result<(), String> {
    if let Some(p) = priority
        && !(1..=5).contains(&p)
    {
        return Err("Priority must be between 1 and 5".to_string());
    }
    Ok(())
}

// =============================================================================
// Parameter Structs
// =============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListTasksParams {
    #[schemars(description = "TaskList ID to list tasks from")]
    pub list_id: String,
    #[schemars(
        description = "FTS5 search query (optional). If provided, performs full-text search. Searches title, description, tags, external_refs. Examples: 'rust backend' (simple), 'rust AND backend' (Boolean), '\"exact phrase\"' (phrase match), 'owner/repo#123' (GitHub issue)"
    )]
    pub query: Option<String>,
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
    #[schemars(
        description = "Priority: 1 (highest/urgent) to 5 (lowest/nice-to-have). Optional, defaults to 5 (P5) if not provided."
    )]
    pub priority: Option<i32>,
    #[schemars(
        description = "Parent task ID for subtasks. BEST PRACTICE: Only ONE level deep (subtasks should not have subtasks). Optional."
    )]
    pub parent_id: Option<String>,
    #[schemars(
        description = "Tags for categorization (e.g., 'bug', 'frontend', 'critical'). Optional."
    )]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "External references to link task to external systems. Examples: ['owner/repo#123', 'PROJ-456']. Optional."
    )]
    pub external_refs: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateTaskParams {
    #[schemars(description = "Task ID to update")]
    pub task_id: String,
    #[schemars(description = "Task title (optional)")]
    pub title: Option<String>,
    #[schemars(description = "Task description (optional)")]
    pub description: Option<String>,
    #[schemars(description = "Priority: 1 (highest) to 5 (lowest) (optional)")]
    pub priority: Option<i32>,
    #[schemars(description = "Tags (optional). Replaces all existing tags when provided.")]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "Parent task ID (optional). Set to change task hierarchy - convert to/from subtask. Use empty string \"\" or null to remove parent."
    )]
    #[serde(
        default,
        deserialize_with = "crate::serde_utils::double_option_string_or_empty"
    )]
    pub parent_id: Option<Option<String>>,
    #[schemars(
        description = "Move task to different list (optional). Use sparingly - tasks should stay in their original list."
    )]
    pub list_id: Option<String>,
    #[schemars(
        description = "External references (optional). Examples: ['owner/repo#123', 'PROJ-456']. Set to update or change external references."
    )]
    pub external_refs: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TransitionTaskParams {
    #[schemars(description = "List of task IDs to transition (one or more)")]
    pub task_ids: Vec<String>,
    #[schemars(
        description = "Target status: 'backlog', 'todo', 'in_progress', 'review', 'done', 'cancelled'"
    )]
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteTaskParams {
    #[schemars(description = "Task ID to delete")]
    pub task_id: String,
}

// =============================================================================
// Tool Implementation
// =============================================================================
// Task Tools
// =============================================================================

#[derive(Clone)]
pub struct TaskTools<D: Database> {
    db: Arc<D>,
    notifier: ChangeNotifier,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> TaskTools<D> {
    pub fn new(db: Arc<D>, notifier: ChangeNotifier) -> Self {
        Self {
            db,
            notifier,
            tool_router: Self::tool_router(),
        }
    }

    /// Get the tool router for this handler
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    #[tool(
        description = "List tasks in a task list with optional full-text search. Provide 'query' parameter to search, omit to list all. Filter by status, parent_id (for subtasks), or tags. Sort by title, status, priority, created_at, updated_at, or completed_at (default: created_at). Use order='asc' or 'desc' (default: asc). Use this to see current work before adding new tasks."
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

        // If query is provided, perform FTS search
        let result = if let Some(q) = &params.0.query {
            self.db.tasks().search(q, Some(&query)).await
        } else {
            self.db.tasks().list(Some(&query)).await
        }
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
        description = "Create a new task (always status='backlog'). WORKFLOW: 1) create_task (backlog), 2) transition_task to 'in_progress' (sets started_at), 3) transition_task to 'done' (sets completed_at). Use transition_task for ALL status changes. For subtasks use parent_id (max ONE level deep)."
    )]
    pub async fn create_task(
        &self,
        params: Parameters<CreateTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate priority before applying default
        validate_priority(params.0.priority).map_err(|e| {
            McpError::invalid_params("validation_error", Some(serde_json::json!({"message": e})))
        })?;

        let task = Task {
            id: String::new(), // Will be generated by DB
            list_id: params.0.list_id.clone(),
            parent_id: params.0.parent_id.clone(),
            title: params.0.title.clone(),
            description: params.0.description.clone(),
            status: TaskStatus::Backlog, // Always create as backlog
            priority: params.0.priority.or(Some(5)), // Default to P5 (lowest priority)
            tags: params.0.tags.clone().unwrap_or_default(),
            external_refs: params.0.external_refs.clone().unwrap_or_default(),
            created_at: None, // Will be set by DB
            started_at: None,
            completed_at: None,
            updated_at: None, // Will be set by DB
        };

        let created = self.db.tasks().create(&task).await.map_err(map_db_error)?;

        self.notifier.notify(UpdateMessage::TaskCreated {
            task_id: created.id.clone(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&created).unwrap(),
        )]))
    }

    #[tool(
        description = "Transition task between statuses. All specified tasks must have the same current status. Transitions: backlog→[todo,in_progress,cancelled], todo→[backlog,in_progress,cancelled], in_progress→[todo,review,done,cancelled], review→[in_progress,done,cancelled], done/cancelled→[backlog,todo,in_progress,review]."
    )]
    pub async fn transition_task(
        &self,
        params: Parameters<TransitionTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        // Parse target status
        let target_status = params.0.status.parse::<TaskStatus>().map_err(|e| {
            McpError::invalid_params(
                "invalid_status",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Call database transition_tasks method
        self.db
            .tasks()
            .transition_tasks(&params.0.task_ids, target_status)
            .await
            .map_err(map_db_error)?;

        // Send notification for each transitioned task
        for task_id in &params.0.task_ids {
            self.notifier.notify(UpdateMessage::TaskUpdated {
                task_id: task_id.clone(),
            });
        }

        // Return success message
        let count = params.0.task_ids.len();
        let message = if count == 1 {
            format!("Successfully transitioned 1 task to {}", params.0.status)
        } else {
            format!(
                "Successfully transitioned {} tasks to {}",
                count, params.0.status
            )
        };

        Ok(CallToolResult::success(vec![Content::text(message)]))
    }

    #[tool(
        description = "Update task content ONLY (title, description, priority, tags, parent_id, list_id). Does NOT change status - use transition_task for status changes. All fields optional."
    )]
    pub async fn update_task(
        &self,
        params: Parameters<UpdateTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate priority if provided
        validate_priority(params.0.priority).map_err(|e| {
            McpError::invalid_params("validation_error", Some(serde_json::json!({"message": e})))
        })?;

        // Get existing task
        let mut task = self.db.tasks().get(&params.0.task_id).await.map_err(|e| {
            McpError::resource_not_found(
                "task_not_found",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Update fields (content only - use transition_task for status changes)
        if let Some(title) = &params.0.title {
            task.title = title.clone();
        }
        if let Some(description) = &params.0.description {
            task.description = Some(description.clone());
        }
        if let Some(priority) = params.0.priority {
            task.priority = Some(priority);
        }
        if let Some(tags) = &params.0.tags {
            task.tags = tags.clone();
        }
        if let Some(parent_id) = &params.0.parent_id {
            task.parent_id = parent_id.clone();
        }
        if let Some(list_id) = &params.0.list_id {
            task.list_id = list_id.clone();
        }
        if let Some(external_refs) = &params.0.external_refs {
            task.external_refs = external_refs.clone();
        }

        task.updated_at = None;

        self.db.tasks().update(&task).await.map_err(map_db_error)?;

        // Fetch updated task to get auto-set timestamps
        let updated = self
            .db
            .tasks()
            .get(&params.0.task_id)
            .await
            .map_err(map_db_error)?;

        self.notifier.notify(UpdateMessage::TaskUpdated {
            task_id: params.0.task_id.clone(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&updated).unwrap(),
        )]))
    }

    #[tool(
        description = "Delete a task permanently. Consider using transition_task with status='cancelled' instead to preserve history."
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

        self.notifier.notify(UpdateMessage::TaskDeleted {
            task_id: params.0.task_id.clone(),
        });

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Task {} deleted successfully",
            params.0.task_id
        ))]))
    }
}
