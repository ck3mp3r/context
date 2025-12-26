//! Repo management handlers.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

use crate::api::AppState;
use crate::db::{Database, DbError, ListQuery, Repo, RepoRepository, SortOrder};

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
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListReposQuery {
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
    path = "/v1/repos",
    tag = "repos",
    params(ListReposQuery),
    responses(
        (status = 200, description = "Paginated list of repos", body = PaginatedRepos),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn list_repos<D: Database>(
    State(state): State<AppState<D>>,
    Query(query): Query<ListReposQuery>,
) -> Result<Json<PaginatedRepos>, (StatusCode, Json<ErrorResponse>)> {
    // Build database query
    let db_query = ListQuery {
        limit: query.limit,
        offset: query.offset,
        sort_by: query.sort.clone(),
        sort_order: match query.order.as_deref() {
            Some("desc") => Some(SortOrder::Desc),
            Some("asc") => Some(SortOrder::Asc),
            _ => None,
        },
        ..Default::default()
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
    path = "/v1/repos/{id}",
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
pub async fn get_repo<D: Database>(
    State(state): State<AppState<D>>,
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
    path = "/v1/repos",
    tag = "repos",
    request_body = CreateRepoRequest,
    responses(
        (status = 201, description = "Repo created", body = RepoResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn create_repo<D: Database>(
    State(state): State<AppState<D>>,
    Json(req): Json<CreateRepoRequest>,
) -> Result<(StatusCode, Json<RepoResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Generate 8-character hex ID
    let id = format!("{:08x}", rand_id());
    let now = chrono_now();

    let repo = Repo {
        id: id.clone(),
        remote: req.remote,
        path: req.path,
        created_at: now,
    };

    state.db().repos().create(&repo).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(RepoResponse::from(repo))))
}

/// Update a repo
///
/// Updates an existing repository
#[utoipa::path(
    put,
    path = "/v1/repos/{id}",
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
pub async fn update_repo<D: Database>(
    State(state): State<AppState<D>>,
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
    path = "/v1/repos/{id}",
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
pub async fn delete_repo<D: Database>(
    State(state): State<AppState<D>>,
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

// =============================================================================
// Helpers
// =============================================================================

/// Generate a random 32-bit ID
fn rand_id() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (duration.as_secs() as u32) ^ (duration.subsec_nanos())
}

/// Get current datetime as string
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
