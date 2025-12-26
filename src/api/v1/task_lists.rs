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
use crate::db::{
    Database, DbError, ListQuery, SortOrder, TaskList, TaskListRepository, TaskListStatus,
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

    let db_query = ListQuery {
        limit: query.limit,
        offset: query.offset,
        sort_by: query.sort.clone(),
        sort_order: match query.order.as_deref() {
            Some("desc") => Some(SortOrder::Desc),
            Some("asc") => Some(SortOrder::Asc),
            _ => None,
        },
        tags,
        status: query.status.clone(),
        ..Default::default()
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
    let id = format!("{:08x}", rand_id());
    let now = chrono_now();

    let list = TaskList {
        id: id.clone(),
        name: req.name,
        description: req.description,
        notes: req.notes,
        tags: req.tags,
        external_ref: req.external_ref,
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: now.clone(),
        updated_at: now,
        archived_at: None,
    };

    state.db().task_lists().create(&list).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(TaskListResponse::from(list))))
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

    if let Some(status) = req.status {
        list.status = match status.as_str() {
            "archived" => {
                list.archived_at = Some(chrono_now());
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

fn rand_id() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (duration.as_secs() as u32) ^ (duration.subsec_nanos())
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = secs / 86400;
    let years = 1970 + (days / 365);
    format!("{}-01-01 00:00:00", years)
}
