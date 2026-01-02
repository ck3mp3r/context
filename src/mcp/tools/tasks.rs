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
pub struct TransitionTaskParams {
    #[schemars(description = "Task ID to transition")]
    pub task_id: String,
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
// Helper Functions
// =============================================================================

/// Returns allowed target statuses for a given current status
fn allowed_transitions(current: &TaskStatus) -> Vec<TaskStatus> {
    match current {
        TaskStatus::Backlog => vec![
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::Cancelled,
        ],
        TaskStatus::Todo => vec![
            TaskStatus::Backlog,
            TaskStatus::InProgress,
            TaskStatus::Cancelled,
        ],
        TaskStatus::InProgress => vec![
            TaskStatus::Todo,
            TaskStatus::Review,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ],
        TaskStatus::Review => vec![
            TaskStatus::InProgress,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ],
        TaskStatus::Done => vec![
            TaskStatus::Backlog,
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::Review,
        ],
        TaskStatus::Cancelled => vec![
            TaskStatus::Backlog,
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::Review,
        ],
    }
}

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
        description = "Create a new task (always status='backlog'). WORKFLOW: 1) create_task (backlog), 2) transition_task to 'in_progress' (sets started_at), 3) transition_task to 'done' (sets completed_at). Use transition_task for ALL status changes. For subtasks use parent_id (max ONE level deep)."
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
            priority: params.0.priority.or(Some(5)), // Default to P5 (lowest priority)
            tags: params.0.tags.clone().unwrap_or_default(),
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
        description = "Transition task between statuses with validation. Enforces workflow rules and sets timestamps. Valid transitions: backlog→[todo,in_progress,cancelled], todo→[backlog,in_progress,cancelled], in_progress→[todo,review,done,cancelled], review→[in_progress,done,cancelled], done→[backlog,todo,in_progress,review], cancelled→[backlog,todo,in_progress,review]. Both done and cancelled can be reopened. Sets started_at when transitioning to in_progress, completed_at when transitioning to done, clears completed_at when transitioning from done."
    )]
    pub async fn transition_task(
        &self,
        params: Parameters<TransitionTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        // Get existing task
        let task = self.db.tasks().get(&params.0.task_id).await.map_err(|e| {
            McpError::resource_not_found(
                "task_not_found",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Parse target status
        let target_status = params.0.status.parse::<TaskStatus>().map_err(|e| {
            McpError::invalid_params(
                "invalid_status",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Check if transition is allowed
        let allowed = allowed_transitions(&task.status);
        if !allowed.contains(&target_status) {
            let current_str = task.status.to_string().to_lowercase();
            let target_str = target_status.to_string().to_lowercase();
            let allowed_strs: Vec<String> = allowed
                .iter()
                .map(|s| s.to_string().to_lowercase())
                .collect();

            let error_message = if allowed.is_empty() {
                format!(
                    "Cannot transition from '{}' - it is a final state. Current: {}, Attempted: {}",
                    current_str, current_str, target_str
                )
            } else {
                format!(
                    "Invalid transition from '{}' to '{}'. Allowed transitions: [{}]",
                    current_str,
                    target_str,
                    allowed_strs.join(", ")
                )
            };

            return Err(McpError::invalid_params(
                "invalid_transition",
                Some(serde_json::json!({
                    "error": error_message,
                    "current_status": current_str,
                    "attempted_status": target_str,
                    "allowed_statuses": allowed_strs,
                })),
            ));
        }

        // Perform transition
        let mut updated_task = task.clone();
        updated_task.status = target_status.clone();

        // Clear completed_at when transitioning FROM done to any other status
        if task.status == TaskStatus::Done && target_status != TaskStatus::Done {
            updated_task.completed_at = None;
        }

        // Set timestamps based on target status
        match target_status {
            TaskStatus::InProgress => {
                // Set started_at if not already set
                if updated_task.started_at.is_none() {
                    // Let database set it via trigger
                }
            }
            TaskStatus::Done => {
                // completed_at will be set by database trigger
            }
            _ => {
                // Other transitions don't modify timestamps
            }
        }

        self.db
            .tasks()
            .update(&updated_task)
            .await
            .map_err(map_db_error)?;

        // Fetch updated task to get auto-set timestamps
        let result_task = self
            .db
            .tasks()
            .get(&params.0.task_id)
            .await
            .map_err(map_db_error)?;

        self.notifier.notify(UpdateMessage::TaskUpdated {
            task_id: params.0.task_id.clone(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result_task).unwrap(),
        )]))
    }

    #[tool(
        description = "Update task content ONLY (title, description, priority, tags, parent_id, list_id). Does NOT change status - use transition_task for status changes. All fields optional."
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
