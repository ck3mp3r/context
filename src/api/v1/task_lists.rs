//! TaskList management handlers.

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
    Database, DbError, PageSort, SortOrder, TaskList, TaskListQuery, TaskListRepository,
    TaskListStatus,
};

use super::ErrorResponse;

// =============================================================================
// DTOs
// =============================================================================

#[derive(Serialize, ToSchema)]
pub struct TaskListResponse {
    #[schema(example = "a1b2c3d4")]
    pub id: String,
    #[schema(example = "Sprint 1")]
    pub name: String,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub external_ref: Option<String>,
    #[schema(example = "active")]
    pub status: String,
    /// Repository IDs linked to this task list
    pub repo_ids: Vec<String>,
    /// Project ID this task list belongs to (one project per task list)
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

impl From<TaskList> for TaskListResponse {
    fn from(t: TaskList) -> Self {
        Self {
            id: t.id,
            name: t.name,
            description: t.description,
            notes: t.notes,
            tags: t.tags,
            external_ref: t.external_ref,
            status: match t.status {
                TaskListStatus::Active => "active".to_string(),
                TaskListStatus::Archived => "archived".to_string(),
            },
            repo_ids: t.repo_ids,
            project_id: t.project_id,
            created_at: t.created_at,
            updated_at: t.updated_at,
            archived_at: t.archived_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTaskListRequest {
    #[schema(example = "Sprint 1")]
    pub name: String,
    pub description: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub external_ref: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTaskListRequest {
    #[schema(example = "Sprint 1")]
    pub name: String,
    pub description: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub external_ref: Option<String>,
    #[schema(example = "active")]
    pub status: Option<String>,
    /// Repository IDs to link to this task list
    #[serde(default)]
    pub repo_ids: Vec<String>,
    /// Project ID this task list belongs to (one project per task list)
    pub project_id: Option<String>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct PatchTaskListRequest {
    #[schema(example = "Sprint 1")]
    pub name: Option<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub tags: Option<Vec<String>>,
    pub external_ref: Option<String>,
    #[schema(example = "active")]
    pub status: Option<String>,
    /// Repository IDs to link to this task list
    pub repo_ids: Option<Vec<String>>,
    /// Project ID this task list belongs to (one project per task list)
    pub project_id: Option<String>,
}

impl PatchTaskListRequest {
    fn merge_into(self, target: &mut TaskList) {
        if let Some(name) = self.name {
            target.name = name;
        }
        if let Some(description) = self.description {
            target.description = Some(description);
        }
        if let Some(notes) = self.notes {
            target.notes = Some(notes);
        }
        if let Some(tags) = self.tags {
            target.tags = tags;
        }
        if let Some(external_ref) = self.external_ref {
            target.external_ref = Some(external_ref);
        }
        if let Some(status_str) = self.status
            && let Ok(status) = status_str.parse::<TaskListStatus>()
        {
            target.status = status;
        }
        if let Some(repo_ids) = self.repo_ids {
            target.repo_ids = repo_ids;
        }
        if let Some(project_id) = self.project_id {
            target.project_id = Some(project_id);
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListTaskListsQuery {
    /// Filter by tags (comma-separated)
    #[param(example = "work,urgent")]
    pub tags: Option<String>,
    /// Filter by status (active, archived)
    #[param(example = "active")]
    pub status: Option<String>,
    /// Maximum number of items to return
    #[param(example = 20)]
    pub limit: Option<usize>,
    /// Number of items to skip
    #[param(example = 0)]
    pub offset: Option<usize>,
    /// Field to sort by (name, status, created_at, updated_at)
    #[param(example = "created_at")]
    pub sort: Option<String>,
    /// Sort order (asc, desc)
    #[param(example = "desc")]
    pub order: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedTaskLists {
    pub items: Vec<TaskListResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

// =============================================================================
// Handlers
// =============================================================================

#[utoipa::path(
    get,
    path = "/v1/task-lists",
    tag = "task-lists",
    params(ListTaskListsQuery),
    responses(
        (status = 200, description = "Paginated list of task lists", body = PaginatedTaskLists),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn list_task_lists<D: Database>(
    State(state): State<AppState<D>>,
    Query(query): Query<ListTaskListsQuery>,
) -> Result<Json<PaginatedTaskLists>, (StatusCode, Json<ErrorResponse>)> {
    // Build database query with tag filtering at DB level
    let tags = query.tags.as_ref().map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    });

    let db_query = TaskListQuery {
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
        status: query.status.clone(),
        tags,
    };

    let result = state
        .db()
        .task_lists()
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

    let items: Vec<TaskListResponse> = result
        .items
        .into_iter()
        .map(TaskListResponse::from)
        .collect();

    Ok(Json(PaginatedTaskLists {
        items,
        total: result.total,
        limit: result.limit.unwrap_or(50),
        offset: result.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/v1/task-lists/{id}",
    tag = "task-lists",
    params(("id" = String, Path, description = "TaskList ID")),
    responses(
        (status = 200, description = "TaskList found", body = TaskListResponse),
        (status = 404, description = "TaskList not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn get_task_list<D: Database>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Json<TaskListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let list = state
        .db()
        .task_lists()
        .get(&id)
        .await
        .map_err(|e| match e {
            DbError::NotFound { .. } => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("TaskList '{}' not found", id),
                }),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ),
        })?;

    Ok(Json(TaskListResponse::from(list)))
}

#[utoipa::path(
    post,
    path = "/v1/task-lists",
    tag = "task-lists",
    request_body = CreateTaskListRequest,
    responses(
        (status = 201, description = "TaskList created", body = TaskListResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn create_task_list<D: Database>(
    State(state): State<AppState<D>>,
    Json(req): Json<CreateTaskListRequest>,
) -> Result<(StatusCode, Json<TaskListResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Create task list with placeholder values - repository will generate ID and timestamps
    let list = TaskList {
        id: String::new(), // Repository will generate this
        name: req.name,
        description: req.description,
        notes: req.notes,
        tags: req.tags,
        external_ref: req.external_ref,
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: None,
        created_at: String::new(), // Repository will generate this
        updated_at: String::new(), // Repository will generate this
        archived_at: None,
    };

    let created_list = state.db().task_lists().create(&list).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(TaskListResponse::from(created_list)),
    ))
}

#[utoipa::path(
    put,
    path = "/v1/task-lists/{id}",
    tag = "task-lists",
    params(("id" = String, Path, description = "TaskList ID")),
    request_body = UpdateTaskListRequest,
    responses(
        (status = 200, description = "TaskList updated", body = TaskListResponse),
        (status = 404, description = "TaskList not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn update_task_list<D: Database>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateTaskListRequest>,
) -> Result<Json<TaskListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut list = state
        .db()
        .task_lists()
        .get(&id)
        .await
        .map_err(|e| match e {
            DbError::NotFound { .. } => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("TaskList '{}' not found", id),
                }),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ),
        })?;

    list.name = req.name;
    list.description = req.description;
    list.notes = req.notes;
    list.tags = req.tags;
    list.external_ref = req.external_ref;
    list.repo_ids = req.repo_ids;
    list.project_id = req.project_id;

    if let Some(status) = req.status {
        list.status = match status.as_str() {
            "archived" => {
                list.archived_at = Some(current_timestamp());
                TaskListStatus::Archived
            }
            _ => TaskListStatus::Active,
        };
    }

    state.db().task_lists().update(&list).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(TaskListResponse::from(list)))
}

#[utoipa::path(
    patch,
    path = "/v1/task-lists/{id}",
    tag = "task-lists",
    params(("id" = String, Path, description = "TaskList ID")),
    request_body = PatchTaskListRequest,
    responses(
        (status = 200, description = "TaskList partially updated", body = TaskListResponse),
        (status = 404, description = "TaskList not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn patch_task_list<D: Database>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
    Json(req): Json<PatchTaskListRequest>,
) -> Result<Json<TaskListResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Fetch existing task list
    let mut list = state
        .db()
        .task_lists()
        .get(&id)
        .await
        .map_err(|e| match e {
            DbError::NotFound { .. } => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("TaskList '{}' not found", id),
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
    req.merge_into(&mut list);

    // Save (repository handles auto-timestamps for archived_at)
    state.db().task_lists().update(&list).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Re-fetch to get updated timestamps
    let updated = state.db().task_lists().get(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(TaskListResponse::from(updated)))
}

#[utoipa::path(
    delete,
    path = "/v1/task-lists/{id}",
    tag = "task-lists",
    params(("id" = String, Path, description = "TaskList ID")),
    responses(
        (status = 204, description = "TaskList deleted"),
        (status = 404, description = "TaskList not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn delete_task_list<D: Database>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .db()
        .task_lists()
        .delete(&id)
        .await
        .map_err(|e| match e {
            DbError::NotFound { .. } => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("TaskList '{}' not found", id),
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
