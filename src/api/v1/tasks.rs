//! Task management handlers.

use crate::sync::GitOps;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

use crate::api::AppState;
use crate::db::utils::current_timestamp;
use crate::db::{
    Database, DbError, PageSort, SortOrder, Task, TaskQuery, TaskRepository, TaskStatus,
};

use super::ErrorResponse;

// =============================================================================
// DTOs
// =============================================================================

#[derive(Serialize, ToSchema)]
pub struct TaskResponse {
    #[schema(example = "a1b2c3d4")]
    pub id: String,
    pub list_id: String,
    pub parent_id: Option<String>,
    #[schema(example = "Complete the feature")]
    pub content: String,
    #[schema(example = "in_progress")]
    pub status: String,
    pub priority: Option<i32>,
    #[schema(example = json!(["urgent", "bug-fix"]))]
    pub tags: Vec<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl From<Task> for TaskResponse {
    fn from(t: Task) -> Self {
        Self {
            id: t.id,
            list_id: t.list_id,
            parent_id: t.parent_id,
            content: t.content,
            status: match t.status {
                TaskStatus::Backlog => "backlog",
                TaskStatus::Todo => "todo",
                TaskStatus::InProgress => "in_progress",
                TaskStatus::Review => "review",
                TaskStatus::Done => "done",
                TaskStatus::Cancelled => "cancelled",
            }
            .to_string(),
            priority: t.priority,
            tags: t.tags,
            created_at: t.created_at,
            started_at: t.started_at,
            completed_at: t.completed_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTaskRequest {
    #[schema(example = "Complete the feature")]
    pub content: String,
    pub parent_id: Option<String>,
    #[schema(example = "backlog")]
    pub status: Option<String>,
    pub priority: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTaskRequest {
    #[schema(example = "Updated content")]
    pub content: String,
    #[schema(example = "done")]
    pub status: Option<String>,
    pub priority: Option<i32>,
    #[schema(example = json!(["urgent", "bug-fix"]))]
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Patch task request DTO (partial update)
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct PatchTaskRequest {
    /// Task content/description
    #[schema(example = "Updated content")]
    pub content: Option<String>,
    /// Task status (auto-manages started_at and completed_at timestamps)
    #[schema(example = "done")]
    pub status: Option<String>,
    /// Priority level
    pub priority: Option<i32>,
    /// Parent task ID (for subtasks)
    pub parent_id: Option<String>,
    /// Tags for categorization
    #[schema(example = json!(["urgent", "bug-fix"]))]
    pub tags: Option<Vec<String>>,
    /// Move task to different list
    #[schema(example = "abc123de")]
    pub list_id: Option<String>,
}

impl PatchTaskRequest {
    fn merge_into(self, target: &mut Task) {
        if let Some(content) = self.content {
            target.content = content;
        }
        if let Some(status_str) = self.status
            && let Ok(status) = status_str.parse()
        {
            target.status = status;
        }
        if let Some(priority) = self.priority {
            target.priority = Some(priority);
        }
        if let Some(parent_id) = self.parent_id {
            target.parent_id = Some(parent_id);
        }
        if let Some(tags) = self.tags {
            target.tags = tags;
        }
        if let Some(list_id) = self.list_id {
            target.list_id = list_id;
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListTasksQuery {
    /// Filter by status (backlog, todo, in_progress, review, done, cancelled)
    #[param(example = "in_progress")]
    pub status: Option<String>,
    /// Filter by parent task ID (for subtasks)
    #[param(example = "a1b2c3d4")]
    pub parent_id: Option<String>,
    /// Maximum number of items to return
    #[param(example = 20)]
    pub limit: Option<usize>,
    /// Number of items to skip
    #[param(example = 0)]
    pub offset: Option<usize>,
    /// Field to sort by (content, status, priority, created_at)
    #[param(example = "created_at")]
    pub sort: Option<String>,
    /// Sort order (asc, desc)
    #[param(example = "desc")]
    pub order: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedTasks {
    pub items: Vec<TaskResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

// =============================================================================
// Handlers
// =============================================================================

#[utoipa::path(
    get,
    path = "/v1/task-lists/{list_id}/tasks",
    tag = "tasks",
    params(
        ("list_id" = String, Path, description = "TaskList ID"),
        ListTasksQuery
    ),
    responses(
        (status = 200, description = "Paginated list of tasks", body = PaginatedTasks),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn list_tasks<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(list_id): Path<String>,
    Query(query): Query<ListTasksQuery>,
) -> Result<Json<PaginatedTasks>, (StatusCode, Json<ErrorResponse>)> {
    // Build database query
    let db_query = TaskQuery {
        page: PageSort {
            limit: query.limit,
            offset: query.offset,
            sort_by: query.sort.clone(),
            sort_order: match query.order.as_deref() {
                Some("desc") => Some(SortOrder::Desc),
                Some("asc") => Some(SortOrder::Asc),
                _ => None,
            },
        },
        list_id: Some(list_id),
        parent_id: query.parent_id.clone(),
        status: query.status.clone(),
        tags: None,
    };

    let result = state
        .db()
        .tasks()
        .list(Some(&db_query))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let items: Vec<TaskResponse> = result.items.into_iter().map(TaskResponse::from).collect();

    Ok(Json(PaginatedTasks {
        items,
        total: result.total,
        limit: result.limit.unwrap_or(50),
        offset: result.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/v1/tasks/{id}",
    tag = "tasks",
    params(("id" = String, Path, description = "Task ID")),
    responses(
        (status = 200, description = "Task found", body = TaskResponse),
        (status = 404, description = "Task not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn get_task<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<Json<TaskResponse>, (StatusCode, Json<ErrorResponse>)> {
    let task = state.db().tasks().get(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Task '{}' not found", id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    Ok(Json(TaskResponse::from(task)))
}

#[utoipa::path(
    post,
    path = "/v1/task-lists/{list_id}/tasks",
    tag = "tasks",
    params(("list_id" = String, Path, description = "TaskList ID")),
    request_body = CreateTaskRequest,
    responses(
        (status = 201, description = "Task created", body = TaskResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn create_task<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(list_id): Path<String>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, Json<ErrorResponse>)> {
    let status = req
        .status
        .as_deref()
        .map(parse_status)
        .unwrap_or(TaskStatus::Backlog);

    let started_at = if matches!(status, TaskStatus::InProgress) {
        Some(current_timestamp())
    } else {
        None
    };

    // Create task with placeholder values - repository will generate ID and timestamp
    let task = Task {
        id: String::new(), // Repository will generate this
        list_id,
        parent_id: req.parent_id,
        content: req.content,
        status,
        priority: req.priority,
        tags: vec![],
        created_at: String::new(), // Repository will generate this
        started_at,
        completed_at: None,
    };

    let created_task = state.db().tasks().create(&task).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(TaskResponse::from(created_task))))
}

#[utoipa::path(
    put,
    path = "/v1/tasks/{id}",
    tag = "tasks",
    params(("id" = String, Path, description = "Task ID")),
    request_body = UpdateTaskRequest,
    responses(
        (status = 200, description = "Task updated", body = TaskResponse),
        (status = 404, description = "Task not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn update_task<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateTaskRequest>,
) -> Result<Json<TaskResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut task = state.db().tasks().get(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Task '{}' not found", id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    task.content = req.content;
    task.priority = req.priority;
    task.tags = req.tags;

    if let Some(status_str) = req.status {
        let new_status = parse_status(&status_str);

        // Track timestamps on status transitions
        if matches!(new_status, TaskStatus::InProgress) && task.started_at.is_none() {
            task.started_at = Some(current_timestamp());
        }
        if matches!(new_status, TaskStatus::Done | TaskStatus::Cancelled)
            && task.completed_at.is_none()
        {
            task.completed_at = Some(current_timestamp());
        }

        task.status = new_status;
    }

    state.db().tasks().update(&task).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(TaskResponse::from(task)))
}

/// Partially update a task
///
/// Updates only the fields provided in the request (PATCH semantics).
/// Auto-manages started_at and completed_at timestamps based on status transitions.
#[utoipa::path(
    patch,
    path = "/v1/tasks/{id}",
    tag = "tasks",
    params(("id" = String, Path, description = "Task ID")),
    request_body = PatchTaskRequest,
    responses(
        (status = 200, description = "Task updated", body = TaskResponse),
        (status = 404, description = "Task not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn patch_task<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
    Json(req): Json<PatchTaskRequest>,
) -> Result<Json<TaskResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Fetch existing task
    let mut task = state.db().tasks().get(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Task '{}' not found", id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    // Merge PATCH changes
    req.merge_into(&mut task);

    // Save (repository auto-manages started_at/completed_at based on status)
    state.db().tasks().update(&task).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Re-fetch to get auto-set timestamps
    let updated = state.db().tasks().get(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(TaskResponse::from(updated)))
}

#[utoipa::path(
    delete,
    path = "/v1/tasks/{id}",
    tag = "tasks",
    params(("id" = String, Path, description = "Task ID")),
    responses(
        (status = 204, description = "Task deleted"),
        (status = 404, description = "Task not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn delete_task<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state.db().tasks().delete(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Task '{}' not found", id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// Helpers
// =============================================================================

fn parse_status(s: &str) -> TaskStatus {
    match s {
        "todo" => TaskStatus::Todo,
        "in_progress" => TaskStatus::InProgress,
        "review" => TaskStatus::Review,
        "done" => TaskStatus::Done,
        "cancelled" => TaskStatus::Cancelled,
        _ => TaskStatus::Backlog,
    }
}
