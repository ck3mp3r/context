//! Job management handlers.

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
use crate::db::Database;
use crate::jobs::JobStatus;

// Reuse ErrorResponse from projects module (defined in all v1 modules)
use super::projects::ErrorResponse;

// =============================================================================
// DTOs (Data Transfer Objects)
// =============================================================================

/// Job response DTO
#[derive(Serialize, ToSchema)]
pub struct JobResponse {
    /// Unique job ID (UUID)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub job_id: String,
    /// Job type (e.g., "analyze_repository")
    #[schema(example = "analyze_repository")]
    pub job_type: String,
    /// Job status
    #[schema(example = "running")]
    pub status: String,
    /// Job parameters (JSON)
    pub params: serde_json::Value,
    /// Progress (optional)
    pub progress: Option<ProgressInfo>,
    /// Result (when completed)
    pub result: Option<serde_json::Value>,
    /// Error message (when failed)
    pub error: Option<String>,
    /// Creation timestamp
    #[schema(example = "2026-03-22 12:00:00")]
    pub created_at: String,
    /// Start timestamp
    pub started_at: Option<String>,
    /// Completion timestamp
    pub completed_at: Option<String>,
}

/// Progress information
#[derive(Serialize, ToSchema)]
pub struct ProgressInfo {
    /// Current progress value
    pub current: usize,
    /// Total progress value
    pub total: usize,
}

/// Create job request DTO
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateJobRequest {
    /// Job type
    #[schema(example = "analyze_repository")]
    pub job_type: String,
    /// Job parameters (JSON)
    #[schema(example = json!({"repo_id": "abc123", "path": "/path/to/repo"}))]
    pub params: serde_json::Value,
}

/// List jobs query parameters
#[derive(Debug, Deserialize, IntoParams)]
pub struct ListJobsQuery {
    /// Filter by status
    #[param(example = "running")]
    pub status: Option<String>,
    /// Filter by job type
    #[param(example = "analyze_repository")]
    pub job_type: Option<String>,
    /// Limit results
    #[param(example = 20)]
    pub limit: Option<usize>,
    /// Offset for pagination
    #[param(example = 0)]
    pub offset: Option<usize>,
}

/// Paginated jobs response
#[derive(Serialize, ToSchema)]
pub struct PaginatedJobs {
    /// List of jobs
    pub items: Vec<JobResponse>,
    /// Total count
    pub total: usize,
    /// Limit used
    pub limit: usize,
    /// Offset used
    pub offset: usize,
}

// =============================================================================
// Helper functions
// =============================================================================

impl From<JobStatus> for JobResponse {
    fn from(job: JobStatus) -> Self {
        Self {
            job_id: job.job_id,
            job_type: job.job_type,
            status: job.status.to_string(),
            params: job.params,
            progress: job
                .progress
                .map(|(current, total)| ProgressInfo { current, total }),
            result: job.result,
            error: job.error,
            created_at: job.created_at.to_rfc3339(),
            started_at: job.started_at.map(|dt| dt.to_rfc3339()),
            completed_at: job.completed_at.map(|dt| dt.to_rfc3339()),
        }
    }
}

// =============================================================================
// Handlers
// =============================================================================

/// Create a new job
#[utoipa::path(
    post,
    path = "/api/v1/jobs",
    tag = "jobs",
    request_body = CreateJobRequest,
    responses(
        (status = 201, description = "Job created successfully", body = JobResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
    )
)]
#[instrument(skip(state))]
pub async fn create_job<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Json(req): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<JobResponse>), (StatusCode, Json<ErrorResponse>)> {
    use crate::db::utils::generate_entity_id;
    let job_id = generate_entity_id();

    state
        .job_queue()
        .create(job_id.clone(), req.job_type, req.params)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    state
        .job_executor()
        .execute_job(&job_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let job = state.job_queue().get(&job_id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(job.into())))
}

/// Get job by ID
#[utoipa::path(
    get,
    path = "/api/v1/jobs/{id}",
    tag = "jobs",
    params(
        ("id" = String, Path, description = "Job ID")
    ),
    responses(
        (status = 200, description = "Job found", body = JobResponse),
        (status = 404, description = "Job not found", body = ErrorResponse),
    )
)]
#[instrument(skip(state))]
pub async fn get_job<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<Json<JobResponse>, (StatusCode, Json<ErrorResponse>)> {
    let job = state.job_queue().get(&id).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(job.into()))
}

/// List jobs with optional filtering
#[utoipa::path(
    get,
    path = "/api/v1/jobs",
    tag = "jobs",
    params(ListJobsQuery),
    responses(
        (status = 200, description = "Jobs list", body = PaginatedJobs),
    )
)]
#[instrument(skip(state))]
pub async fn list_jobs<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Query(query): Query<ListJobsQuery>,
) -> Result<Json<PaginatedJobs>, (StatusCode, Json<ErrorResponse>)> {
    let status_filter = query.status.as_deref();
    let type_filter = query.job_type.as_deref();

    let all_jobs = state.job_queue().list(status_filter, type_filter);

    let limit = query.limit.unwrap_or(10).min(100);
    let offset = query.offset.unwrap_or(0);

    let total = all_jobs.len();
    let items: Vec<JobResponse> = all_jobs
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|j| j.into())
        .collect();

    Ok(Json(PaginatedJobs {
        items,
        total,
        limit,
        offset,
    }))
}

/// Cancel a job
#[utoipa::path(
    delete,
    path = "/api/v1/jobs/{id}",
    tag = "jobs",
    params(
        ("id" = String, Path, description = "Job ID")
    ),
    responses(
        (status = 200, description = "Job cancelled", body = JobResponse),
        (status = 404, description = "Job not found", body = ErrorResponse),
        (status = 400, description = "Job cannot be cancelled", body = ErrorResponse),
    )
)]
#[instrument(skip(state))]
pub async fn cancel_job<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<Json<JobResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .job_queue()
        .update_status(&id, crate::jobs::Status::Cancelled)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let job = state.job_queue().get(&id).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(job.into()))
}

#[cfg(test)]
#[path = "jobs_test.rs"]
mod jobs_test;
