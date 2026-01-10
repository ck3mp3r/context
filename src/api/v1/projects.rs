//! Project management handlers.

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
use crate::api::notifier::UpdateMessage;
use crate::db::{Database, DbError, PageSort, Project, ProjectQuery, ProjectRepository, SortOrder};

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
    /// Tags for categorization
    #[schema(example = json!(["rust", "backend"]))]
    pub tags: Vec<String>,
    /// External references (e.g., GitHub issues, Jira tickets)
    #[schema(example = json!(["owner/repo#123", "PROJ-456"]))]
    pub external_refs: Vec<String>,
    /// Linked repository IDs
    #[schema(example = json!(["repo0001", "repo0002"]))]
    pub repo_ids: Vec<String>,
    /// Linked task list IDs
    #[schema(example = json!(["list0001", "list0002"]))]
    pub task_list_ids: Vec<String>,
    /// Linked note IDs
    #[schema(example = json!(["note0001", "note0002"]))]
    pub note_ids: Vec<String>,
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
            tags: p.tags,
            external_refs: p.external_refs,
            repo_ids: p.repo_ids,
            task_list_ids: p.task_list_ids,
            note_ids: p.note_ids,
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
    /// Tags for categorization
    #[schema(example = json!(["rust", "backend"]))]
    #[serde(default)]
    pub tags: Vec<String>,
    /// External references (e.g., GitHub issues, Jira tickets)
    #[schema(example = json!(["owner/repo#123", "PROJ-456"]))]
    #[serde(default)]
    pub external_refs: Vec<String>,
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
    /// Tags for categorization
    #[schema(example = json!(["rust", "backend"]))]
    #[serde(default)]
    pub tags: Vec<String>,
    /// External references (e.g., GitHub issues, Jira tickets)
    #[schema(example = json!(["owner/repo#123", "PROJ-456"]))]
    #[serde(default)]
    pub external_refs: Vec<String>,
}

/// Patch project request DTO (partial update)
///
/// Note: Project relationships (repo_ids, task_list_ids, note_ids) are managed
/// from the other side - i.e., you link a Repo/TaskList/Note TO a Project,
/// not the other way around. These fields are read-only on Project responses.
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct PatchProjectRequest {
    /// Project title
    #[schema(example = "Updated Project")]
    pub title: Option<String>,
    /// Optional description  
    #[schema(example = "Updated description")]
    pub description: Option<String>,
    /// Tags for categorization
    #[schema(example = json!(["rust", "backend"]))]
    pub tags: Option<Vec<String>>,
    /// External references (e.g., GitHub issues, Jira tickets)
    #[schema(example = json!(["owner/repo#123", "PROJ-456"]))]
    pub external_refs: Option<Vec<String>>,
}

impl PatchProjectRequest {
    fn merge_into(self, target: &mut Project) {
        if let Some(title) = self.title {
            target.title = title;
        }
        if let Some(description) = self.description {
            target.description = Some(description);
        }
        if let Some(tags) = self.tags {
            target.tags = tags;
        }
        if let Some(external_refs) = self.external_refs {
            target.external_refs = external_refs;
        }
    }
}

/// Error response DTO
#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Error message
    #[schema(example = "Project not found")]
    pub error: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListProjectsQuery {
    /// Maximum number of items to return
    #[param(example = 20)]
    pub limit: Option<usize>,
    /// Number of items to skip
    #[param(example = 0)]
    pub offset: Option<usize>,
    /// Field to sort by (title, created_at, updated_at)
    #[param(example = "created_at")]
    pub sort: Option<String>,
    /// Sort order (asc, desc)
    #[param(example = "desc")]
    pub order: Option<String>,
    /// Filter by tags (comma-separated)
    #[param(example = "rust,backend")]
    pub tags: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedProjects {
    pub items: Vec<ProjectResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

// =============================================================================
// Handlers
// =============================================================================

/// List all projects
///
/// Returns a paginated list of projects with optional sorting
#[utoipa::path(
    get,
    path = "/api/v1/projects",
    tag = "projects",
    params(ListProjectsQuery),
    responses(
        (status = 200, description = "Paginated list of projects", body = PaginatedProjects),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn list_projects<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Query(query): Query<ListProjectsQuery>,
) -> Result<Json<PaginatedProjects>, (StatusCode, Json<ErrorResponse>)> {
    // Parse tags from comma-separated string
    let tags = query
        .tags
        .as_ref()
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    // Build database query
    let db_query = ProjectQuery {
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
        tags,
    };

    let result = state
        .db()
        .projects()
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

    let items: Vec<ProjectResponse> = result
        .items
        .into_iter()
        .map(ProjectResponse::from)
        .collect();

    Ok(Json(PaginatedProjects {
        items,
        total: result.total,
        limit: result.limit.unwrap_or(50),
        offset: result.offset,
    }))
}

/// Get a project by ID
///
/// Returns a single project by its ID
#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}",
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
pub async fn get_project<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<Json<ProjectResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project = state.db().projects().get(&id).await.map_err(|e| match e {
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
    path = "/api/v1/projects",
    tag = "projects",
    request_body = CreateProjectRequest,
    responses(
        (status = 201, description = "Project created", body = ProjectResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn create_project<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Create project with placeholder values - repository will generate ID and timestamps
    let project = Project {
        id: String::new(), // Repository will generate this
        title: req.title,
        description: req.description,
        tags: req.tags,
        external_refs: req.external_refs,
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: String::new(), // Repository will generate this
        updated_at: String::new(), // Repository will generate this
    };

    let created_project = state.db().projects().create(&project).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Broadcast notification
    state.notifier().notify(UpdateMessage::ProjectCreated {
        project_id: created_project.id.clone(),
    });

    Ok((
        StatusCode::CREATED,
        Json(ProjectResponse::from(created_project)),
    ))
}

/// Update a project
///
/// Updates an existing project
#[utoipa::path(
    put,
    path = "/api/v1/projects/{id}",
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
pub async fn update_project<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectResponse>, (StatusCode, Json<ErrorResponse>)> {
    // First get the existing project
    let mut project = state.db().projects().get(&id).await.map_err(|e| match e {
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
    project.tags = req.tags;
    project.external_refs = req.external_refs;

    state.db().projects().update(&project).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Broadcast notification
    state.notifier().notify(UpdateMessage::ProjectUpdated {
        project_id: id.clone(),
    });

    // Re-fetch to get updated timestamp
    let updated = state.db().projects().get(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(ProjectResponse::from(updated)))
}

/// Partially update a project
///
/// Updates only the fields provided in the request (PATCH semantics)
#[utoipa::path(
    patch,
    path = "/api/v1/projects/{id}",
    tag = "projects",
    params(
        ("id" = String, Path, description = "Project ID (8-character hex)")
    ),
    request_body = PatchProjectRequest,
    responses(
        (status = 200, description = "Project updated", body = ProjectResponse),
        (status = 404, description = "Project not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn patch_project<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
    Json(req): Json<PatchProjectRequest>,
) -> Result<Json<ProjectResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Fetch existing project
    let mut project = state.db().projects().get(&id).await.map_err(|e| match e {
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

    // Merge PATCH changes
    req.merge_into(&mut project);

    // Save
    state.db().projects().update(&project).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Broadcast notification
    state.notifier().notify(UpdateMessage::ProjectUpdated {
        project_id: id.clone(),
    });

    // Re-fetch to get updated timestamp
    let updated = state.db().projects().get(&id).await.map_err(|e| {
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
    path = "/api/v1/projects/{id}",
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
pub async fn delete_project<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .db()
        .projects()
        .delete(&id)
        .await
        .map_err(|e| match e {
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

    // Broadcast notification
    state.notifier().notify(UpdateMessage::ProjectDeleted {
        project_id: id.clone(),
    });

    Ok(StatusCode::NO_CONTENT)
}
