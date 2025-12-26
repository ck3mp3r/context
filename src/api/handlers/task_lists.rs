//! TaskList management handlers.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utoipa::ToSchema;

use crate::api::AppState;
use crate::db::{Database, DbError, TaskList, TaskListRepository, TaskListStatus};

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

// =============================================================================
// Handlers
// =============================================================================

#[utoipa::path(
    get,
    path = "/task-lists",
    tag = "task-lists",
    responses(
        (status = 200, description = "List of task lists", body = Vec<TaskListResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn list_task_lists<D: Database>(
    State(state): State<AppState<D>>,
) -> Result<Json<Vec<TaskListResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let lists = state.db().task_lists().list().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(
        lists.into_iter().map(TaskListResponse::from).collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/task-lists/{id}",
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
    let list = state.db().task_lists().get(&id).map_err(|e| match e {
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
    path = "/task-lists",
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
        created_at: now.clone(),
        updated_at: now,
        archived_at: None,
    };

    state.db().task_lists().create(&list).map_err(|e| {
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
    path = "/task-lists/{id}",
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
    let mut list = state.db().task_lists().get(&id).map_err(|e| match e {
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

    state.db().task_lists().update(&list).map_err(|e| {
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
    path = "/task-lists/{id}",
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
    state.db().task_lists().delete(&id).map_err(|e| match e {
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
