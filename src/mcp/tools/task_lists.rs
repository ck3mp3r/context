//! MCP tools for TaskList management.

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
use crate::db::{
    Database, PageSort, SortOrder, TaskList, TaskListQuery, TaskListRepository, TaskListStatus,
    TaskRepository,
};
use crate::mcp::tools::{apply_limit, map_db_error};

// =============================================================================
// Parameter Structs
// =============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListTaskListsParams {
    #[schemars(description = "Filter by tags (comma-separated)")]
    pub tags: Option<String>,
    #[schemars(description = "Filter by status (active, archived)")]
    pub status: Option<String>,
    #[schemars(description = "Filter by project ID")]
    pub project_id: Option<String>,
    #[schemars(description = "Maximum number of items to return (default: 10, max: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of items to skip")]
    pub offset: Option<usize>,
    #[schemars(description = "Field to sort by (title, status, created_at, updated_at)")]
    pub sort: Option<String>,
    #[schemars(description = "Sort order (asc, desc)")]
    pub order: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetTaskListParams {
    #[schemars(description = "TaskList ID")]
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateTaskListParams {
    #[schemars(
        description = "Task list title (e.g., 'Phase 2 Implementation', 'Bug Fixes Sprint 12')"
    )]
    pub title: String,
    #[schemars(description = "Brief description of this workstream (optional)")]
    pub description: Option<String>,
    #[schemars(description = "Additional notes or context (optional)")]
    pub notes: Option<String>,
    #[schemars(
        description = "Tags for categorization (e.g., 'sprint-12', 'critical', 'backend') (optional)"
    )]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "External reference like Jira/GitHub issue (e.g., 'PROJ-123') (optional)"
    )]
    pub external_ref: Option<String>,
    #[schemars(description = "Repository IDs related to this workstream (optional)")]
    pub repo_ids: Option<Vec<String>>,
    #[schemars(
        description = "Project ID this list belongs to (REQUIRED - ask user if unclear). Use list_projects to find available projects."
    )]
    pub project_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateTaskListParams {
    #[schemars(description = "TaskList ID")]
    pub id: String,
    #[schemars(description = "TaskList title")]
    pub title: String,
    #[schemars(description = "TaskList description (optional)")]
    pub description: Option<String>,
    #[schemars(description = "Notes for this task list (optional)")]
    pub notes: Option<String>,
    #[schemars(description = "Tags for organization (optional)")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "External reference (optional)")]
    pub external_ref: Option<String>,
    #[schemars(description = "Status (active, archived) (optional)")]
    pub status: Option<String>,
    #[schemars(
        description = "Repository IDs to link (optional). Associate with relevant repos for context."
    )]
    pub repo_ids: Option<Vec<String>>,
    #[schemars(
        description = "Project ID this task list belongs to (optional). Use sparingly - task lists should stay in their original project."
    )]
    pub project_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteTaskListParams {
    #[schemars(description = "TaskList ID")]
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetTaskListStatsParams {
    #[schemars(description = "TaskList ID")]
    pub id: String,
}

// =============================================================================
// TaskList Tools
// =============================================================================

#[derive(Clone)]
pub struct TaskListTools<D: Database> {
    db: Arc<D>,
    notifier: ChangeNotifier,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> TaskListTools<D> {
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
        description = "List task lists with filtering by project, status, or tags. Use this to find existing lists before creating new ones."
    )]
    pub async fn list_task_lists(
        &self,
        params: Parameters<ListTaskListsParams>,
    ) -> Result<CallToolResult, McpError> {
        // Parse tags
        let tags = params.0.tags.as_ref().map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        });

        // Build query
        let query = TaskListQuery {
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
            status: params.0.status.clone(),
            tags,
            project_id: params.0.project_id.clone(),
        };

        let result = self
            .db
            .task_lists()
            .list(Some(&query))
            .await
            .map_err(map_db_error)?;

        let response = json!({
            "items": result.items,
            "total": result.total,
            "limit": result.limit.unwrap_or(50),
            "offset": result.offset,
        });

        let content = serde_json::to_string_pretty(&response).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    #[tool(
        description = "Get a task list by ID with full details including metadata and relationships."
    )]
    pub async fn get_task_list(
        &self,
        params: Parameters<GetTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        let list = self
            .db
            .task_lists()
            .get(&params.0.id)
            .await
            .map_err(map_db_error)?;
        let content = serde_json::to_string_pretty(&list).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    #[tool(
        description = "Create a new task list. IMPORTANT: Only create when starting a NEW workstream/project/feature. Most work should be added as tasks to existing lists. MUST specify project_id - ask user which project if unclear. Task lists group related work, not individual tasks."
    )]
    pub async fn create_task_list(
        &self,
        params: Parameters<CreateTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        let list = TaskList {
            id: String::new(), // Repository generates
            title: params.0.title,
            description: params.0.description,
            notes: params.0.notes,
            tags: params.0.tags.unwrap_or_default(),
            external_ref: params.0.external_ref,
            status: TaskListStatus::Active,
            repo_ids: params.0.repo_ids.unwrap_or_default(),
            project_id: params.0.project_id,
            created_at: String::new(), // Repository generates
            updated_at: String::new(), // Repository generates
            archived_at: None,
        };

        let created = self
            .db
            .task_lists()
            .create(&list)
            .await
            .map_err(map_db_error)?;

        self.notifier.notify(UpdateMessage::TaskListCreated {
            task_list_id: created.id.clone(),
        });

        let content = serde_json::to_string_pretty(&created).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    #[tool(
        description = "Update task list metadata (name, description, status, etc). Use update_task for task-level changes. Archive completed lists with status='archived' instead of deleting."
    )]
    pub async fn update_task_list(
        &self,
        params: Parameters<UpdateTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        // Fetch existing
        let mut list = self
            .db
            .task_lists()
            .get(&params.0.id)
            .await
            .map_err(map_db_error)?;

        // Update fields - only update if provided to preserve existing values
        list.title = params.0.title;

        if let Some(description) = params.0.description {
            list.description = Some(description);
        }

        if let Some(notes) = params.0.notes {
            list.notes = Some(notes);
        }

        if let Some(tags) = params.0.tags {
            list.tags = tags;
        }

        if let Some(external_ref) = params.0.external_ref {
            list.external_ref = Some(external_ref);
        }

        if let Some(repo_ids) = params.0.repo_ids {
            list.repo_ids = repo_ids;
        }

        if let Some(project_id) = params.0.project_id {
            list.project_id = project_id;
        }

        if let Some(status_str) = params.0.status {
            list.status = match status_str.as_str() {
                "archived" => TaskListStatus::Archived,
                _ => TaskListStatus::Active,
            };
        }

        // Update returns (), must fetch again
        self.db
            .task_lists()
            .update(&list)
            .await
            .map_err(map_db_error)?;
        let updated = self
            .db
            .task_lists()
            .get(&params.0.id)
            .await
            .map_err(map_db_error)?;

        self.notifier.notify(UpdateMessage::TaskListUpdated {
            task_list_id: params.0.id.clone(),
        });

        let content = serde_json::to_string_pretty(&updated).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    #[tool(
        description = "Delete a task list permanently. RARELY needed - use update_task_list with status='archived' instead to preserve history."
    )]
    pub async fn delete_task_list(
        &self,
        params: Parameters<DeleteTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .task_lists()
            .delete(&params.0.id)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "database_error",
                    Some(serde_json::json!({"error": e.to_string()})),
                )
            })?;

        self.notifier.notify(UpdateMessage::TaskListDeleted {
            task_list_id: params.0.id.clone(),
        });

        let response = json!({
            "success": true,
            "id": params.0.id,
        });

        let content = serde_json::to_string_pretty(&response).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    #[tool(
        description = "Get task statistics for a task list (counts by status: backlog, todo, in_progress, review, done, cancelled). Use to track progress."
    )]
    pub async fn get_task_list_stats(
        &self,
        params: Parameters<GetTaskListStatsParams>,
    ) -> Result<CallToolResult, McpError> {
        let stats = self
            .db
            .tasks()
            .get_stats_for_list(&params.0.id)
            .await
            .map_err(map_db_error)?;
        let content = serde_json::to_string_pretty(&stats).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        Ok(CallToolResult::success(vec![Content::text(content)]))
    }
}
