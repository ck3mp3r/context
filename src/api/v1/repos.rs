//! Repo management handlers.

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
use crate::db::{Database, DbError, PageSort, Repo, RepoQuery, RepoRepository, SortOrder};

use super::ErrorResponse;

// =============================================================================
// DTOs (Data Transfer Objects)
// =============================================================================

/// Repo response DTO
#[derive(Serialize, ToSchema)]
pub struct RepoResponse {
    /// Unique identifier (8-character hex)
    #[schema(example = "a1b2c3d4")]
    pub id: String,
    /// Remote URL (e.g., "github:user/project")
    #[schema(example = "github:user/project")]
    pub remote: String,
    /// Local filesystem path
    #[schema(example = "/home/user/project")]
    pub path: Option<String>,
    /// Tags for categorization
    #[schema(example = json!(["work", "active"]))]
    pub tags: Vec<String>,
    /// Linked project IDs (M:N relationship via project_repo)
    #[schema(example = json!(["proj123a", "proj456b"]))]
    pub project_ids: Vec<String>,
    /// Creation timestamp
    #[schema(example = "2025-01-01 00:00:00")]
    pub created_at: String,
}

impl From<Repo> for RepoResponse {
    fn from(r: Repo) -> Self {
        Self {
            id: r.id,
            remote: r.remote,
            path: r.path,
            tags: r.tags,
            project_ids: r.project_ids,
            created_at: r.created_at,
        }
    }
}

/// Create repo request DTO
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRepoRequest {
    /// Remote URL (e.g., "github:user/project")
    #[schema(example = "github:user/project")]
    pub remote: String,
    /// Local filesystem path
    #[schema(example = "/home/user/project")]
    pub path: Option<String>,
    /// Tags for categorization
    #[schema(example = json!(["work", "active"]))]
    #[serde(default)]
    pub tags: Vec<String>,
    /// Linked project IDs (M:N relationship via project_repo)
    #[schema(example = json!(["proj123a", "proj456b"]))]
    #[serde(default)]
    pub project_ids: Vec<String>,
}

/// Update repo request DTO
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRepoRequest {
    /// Remote URL (e.g., "github:user/project")
    #[schema(example = "github:user/project")]
    pub remote: String,
    /// Local filesystem path
    #[schema(example = "/home/user/project")]
    pub path: Option<String>,
    /// Tags for categorization
    #[schema(example = json!(["work", "active"]))]
    #[serde(default)]
    pub tags: Vec<String>,
    /// Linked project IDs (M:N relationship via project_repo)
    #[schema(example = json!(["proj123a", "proj456b"]))]
    #[serde(default)]
    pub project_ids: Vec<String>,
}

/// Patch repo request DTO (partial update)
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct PatchRepoRequest {
    /// Remote URL
    #[schema(example = "github:user/project")]
    pub remote: Option<String>,
    /// Local filesystem path
    #[schema(example = "/home/user/project")]
    pub path: Option<String>,
    /// Tags for categorization
    #[schema(example = json!(["work", "active"]))]
    pub tags: Option<Vec<String>>,
    /// Linked project IDs
    #[schema(example = json!(["proj123a", "proj456b"]))]
    pub project_ids: Option<Vec<String>>,
}

impl PatchRepoRequest {
    fn merge_into(self, target: &mut Repo) {
        if let Some(remote) = self.remote {
            target.remote = remote;
        }
        if let Some(path) = self.path {
            target.path = Some(path);
        }
        if let Some(tags) = self.tags {
            target.tags = tags;
        }
        if let Some(project_ids) = self.project_ids {
            target.project_ids = project_ids;
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListReposQuery {
    /// Filter by project ID
    #[param(example = "a1b2c3d4")]
    pub project_id: Option<String>,
    /// Maximum number of items to return
    #[param(example = 20)]
    pub limit: Option<usize>,
    /// Number of items to skip
    #[param(example = 0)]
    pub offset: Option<usize>,
    /// Field to sort by (remote, created_at)
    #[param(example = "created_at")]
    pub sort: Option<String>,
    /// Sort order (asc, desc)
    #[param(example = "desc")]
    pub order: Option<String>,
    /// Filter by tags (comma-separated)
    #[param(example = "work,active")]
    pub tags: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedRepos {
    pub items: Vec<RepoResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

// =============================================================================
// Handlers
// =============================================================================

/// List all repos
///
/// Returns a paginated list of repositories with optional sorting
#[utoipa::path(
    get,
    path = "/api/v1/repos",
    tag = "repos",
    params(ListReposQuery),
    responses(
        (status = 200, description = "Paginated list of repos", body = PaginatedRepos),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn list_repos<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Query(query): Query<ListReposQuery>,
) -> Result<Json<PaginatedRepos>, (StatusCode, Json<ErrorResponse>)> {
    // Parse tags from comma-separated string
    let tags = query
        .tags
        .as_ref()
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    // Build database query
    let db_query = RepoQuery {
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
        project_id: query.project_id.clone(),
    };

    let result = state
        .db()
        .repos()
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

    let items: Vec<RepoResponse> = result.items.into_iter().map(RepoResponse::from).collect();

    Ok(Json(PaginatedRepos {
        items,
        total: result.total,
        limit: result.limit.unwrap_or(50),
        offset: result.offset,
    }))
}

/// Get a repo by ID
///
/// Returns a single repository by its ID
#[utoipa::path(
    get,
    path = "/api/v1/repos/{id}",
    tag = "repos",
    params(
        ("id" = String, Path, description = "Repo ID (8-character hex)")
    ),
    responses(
        (status = 200, description = "Repo found", body = RepoResponse),
        (status = 404, description = "Repo not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn get_repo<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<Json<RepoResponse>, (StatusCode, Json<ErrorResponse>)> {
    let repo = state.db().repos().get(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Repo '{}' not found", id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    Ok(Json(RepoResponse::from(repo)))
}

/// Create a new repo
///
/// Registers a new repository and returns it
#[utoipa::path(
    post,
    path = "/api/v1/repos",
    tag = "repos",
    request_body = CreateRepoRequest,
    responses(
        (status = 201, description = "Repo created", body = RepoResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn create_repo<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Json(req): Json<CreateRepoRequest>,
) -> Result<(StatusCode, Json<RepoResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Create repo with placeholder values - repository will generate ID and timestamps
    let repo = Repo {
        id: String::new(), // Repository will generate this
        remote: req.remote,
        path: req.path,
        tags: req.tags,
        project_ids: req.project_ids,
        created_at: String::new(), // Repository will generate this
    };

    let created_repo = state.db().repos().create(&repo).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(RepoResponse::from(created_repo))))
}

/// Update a repo
///
/// Updates an existing repository
#[utoipa::path(
    put,
    path = "/api/v1/repos/{id}",
    tag = "repos",
    params(
        ("id" = String, Path, description = "Repo ID (8-character hex)")
    ),
    request_body = UpdateRepoRequest,
    responses(
        (status = 200, description = "Repo updated", body = RepoResponse),
        (status = 404, description = "Repo not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn update_repo<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRepoRequest>,
) -> Result<Json<RepoResponse>, (StatusCode, Json<ErrorResponse>)> {
    // First get the existing repo
    let mut repo = state.db().repos().get(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Repo '{}' not found", id),
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
    repo.remote = req.remote;
    repo.path = req.path;
    repo.tags = req.tags;
    repo.project_ids = req.project_ids;

    state.db().repos().update(&repo).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(RepoResponse::from(repo)))
}

/// Partially update a repo
///
/// Updates only the fields provided in the request (PATCH semantics)
#[utoipa::path(
    patch,
    path = "/api/v1/repos/{id}",
    tag = "repos",
    params(
        ("id" = String, Path, description = "Repo ID (8-character hex)")
    ),
    request_body = PatchRepoRequest,
    responses(
        (status = 200, description = "Repo updated", body = RepoResponse),
        (status = 404, description = "Repo not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn patch_repo<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
    Json(req): Json<PatchRepoRequest>,
) -> Result<Json<RepoResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Fetch existing repo
    let mut repo = state.db().repos().get(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Repo '{}' not found", id),
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
    req.merge_into(&mut repo);

    // Save
    state.db().repos().update(&repo).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(RepoResponse::from(repo)))
}

/// Delete a repo
///
/// Deletes a repository by its ID
#[utoipa::path(
    delete,
    path = "/api/v1/repos/{id}",
    tag = "repos",
    params(
        ("id" = String, Path, description = "Repo ID (8-character hex)")
    ),
    responses(
        (status = 204, description = "Repo deleted"),
        (status = 404, description = "Repo not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn delete_repo<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state.db().repos().delete(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Repo '{}' not found", id),
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
