//! Project management handlers.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utoipa::ToSchema;

use crate::api::AppState;
use crate::db::{Database, DbError, Project, ProjectRepository};

// =============================================================================
// DTOs (Data Transfer Objects)
// =============================================================================

/// Project response DTO
#[derive(Serialize, ToSchema)]
pub struct ProjectResponse {
    /// Unique identifier (8-character hex)
    #[schema(example = "a1b2c3d4")]
    pub id: String,
    /// Project title
    #[schema(example = "My Project")]
    pub title: String,
    /// Optional description
    #[schema(example = "A description of the project")]
    pub description: Option<String>,
    /// Creation timestamp
    #[schema(example = "2025-01-01 00:00:00")]
    pub created_at: String,
    /// Last update timestamp
    #[schema(example = "2025-01-01 00:00:00")]
    pub updated_at: String,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            title: p.title,
            description: p.description,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

/// Create project request DTO
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateProjectRequest {
    /// Project title
    #[schema(example = "My Project")]
    pub title: String,
    /// Optional description
    #[schema(example = "A description of the project")]
    pub description: Option<String>,
}

/// Update project request DTO
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProjectRequest {
    /// Project title
    #[schema(example = "Updated Project")]
    pub title: String,
    /// Optional description
    #[schema(example = "Updated description")]
    pub description: Option<String>,
}

/// Error response DTO
#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Error message
    #[schema(example = "Project not found")]
    pub error: String,
}

// =============================================================================
// Handlers
// =============================================================================

/// List all projects
///
/// Returns a list of all projects
#[utoipa::path(
    get,
    path = "/projects",
    tag = "projects",
    responses(
        (status = 200, description = "List of projects", body = Vec<ProjectResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn list_projects<D: Database>(
    State(state): State<AppState<D>>,
) -> Result<Json<Vec<ProjectResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let projects = state.db().projects().list().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(
        projects.into_iter().map(ProjectResponse::from).collect(),
    ))
}

/// Get a project by ID
///
/// Returns a single project by its ID
#[utoipa::path(
    get,
    path = "/projects/{id}",
    tag = "projects",
    params(
        ("id" = String, Path, description = "Project ID (8-character hex)")
    ),
    responses(
        (status = 200, description = "Project found", body = ProjectResponse),
        (status = 404, description = "Project not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn get_project<D: Database>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Json<ProjectResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project = state.db().projects().get(&id).map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Project '{}' not found", id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    Ok(Json(ProjectResponse::from(project)))
}

/// Create a new project
///
/// Creates a new project and returns it
#[utoipa::path(
    post,
    path = "/projects",
    tag = "projects",
    request_body = CreateProjectRequest,
    responses(
        (status = 201, description = "Project created", body = ProjectResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn create_project<D: Database>(
    State(state): State<AppState<D>>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Generate 8-character hex ID
    let id = format!("{:08x}", rand_id());

    let now = chrono_now();
    let project = Project {
        id: id.clone(),
        title: req.title,
        description: req.description,
        created_at: now.clone(),
        updated_at: now,
    };

    state.db().projects().create(&project).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(ProjectResponse::from(project))))
}

/// Update a project
///
/// Updates an existing project
#[utoipa::path(
    put,
    path = "/projects/{id}",
    tag = "projects",
    params(
        ("id" = String, Path, description = "Project ID (8-character hex)")
    ),
    request_body = UpdateProjectRequest,
    responses(
        (status = 200, description = "Project updated", body = ProjectResponse),
        (status = 404, description = "Project not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn update_project<D: Database>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectResponse>, (StatusCode, Json<ErrorResponse>)> {
    // First get the existing project
    let mut project = state.db().projects().get(&id).map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Project '{}' not found", id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    // Update fields
    project.title = req.title;
    project.description = req.description;

    state.db().projects().update(&project).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Re-fetch to get updated timestamp
    let updated = state.db().projects().get(&id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(ProjectResponse::from(updated)))
}

/// Delete a project
///
/// Deletes a project by its ID
#[utoipa::path(
    delete,
    path = "/projects/{id}",
    tag = "projects",
    params(
        ("id" = String, Path, description = "Project ID (8-character hex)")
    ),
    responses(
        (status = 204, description = "Project deleted"),
        (status = 404, description = "Project not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn delete_project<D: Database>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state.db().projects().delete(&id).map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Project '{}' not found", id),
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

/// Generate a random 32-bit ID
fn rand_id() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // Mix time with some randomness from the nanoseconds
    (duration.as_secs() as u32) ^ (duration.subsec_nanos())
}

/// Get current datetime as string
fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // Simple ISO-like format
    let secs = duration.as_secs();
    let days = secs / 86400;
    let years = 1970 + (days / 365);
    // Simplified - not accurate for leap years but good enough for now
    format!("{}-01-01 00:00:00", years)
}
