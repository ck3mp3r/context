//! Sync API endpoints.
//!
//! Provides REST API access to git-based sync operations.

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::state::AppState;
use crate::db::Database;
use crate::sync::GitOps;

use super::ErrorResponse;

/// Request to initialize sync
#[derive(Debug, Deserialize, ToSchema)]
pub struct InitSyncRequest {
    /// Git remote URL (optional)
    #[schema(example = "git@github.com:user/c5t-sync.git")]
    pub remote_url: Option<String>,
}

/// Request to export sync data
#[derive(Debug, Deserialize, ToSchema)]
pub struct ExportSyncRequest {
    /// Commit message (optional)
    #[schema(example = "Update from laptop")]
    pub message: Option<String>,

    /// Push to remote after export (optional, default: false)
    #[serde(default)]
    #[schema(example = false)]
    pub remote: bool,
}

/// Request to import sync data
#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportSyncRequest {
    /// Pull from remote before import (optional, default: false)
    #[serde(default)]
    #[schema(example = false)]
    pub remote: bool,
}

/// Response from sync operations
#[derive(Debug, Serialize, ToSchema)]
pub struct SyncResponse {
    /// Operation status
    pub status: String,
    /// Human-readable message
    pub message: String,
    /// Optional data (varies by operation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Initialize sync repository
#[utoipa::path(
    post,
    path = "/api/v1/sync/init",
    tag = "sync",
    request_body = InitSyncRequest,
    responses(
        (status = 201, description = "Sync created successfully", body = SyncResponse),
        (status = 200, description = "Sync already initialized", body = SyncResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
pub async fn init_sync<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Json(req): Json<InitSyncRequest>,
) -> Result<(StatusCode, Json<SyncResponse>), (StatusCode, Json<ErrorResponse>)> {
    let result = state
        .sync_manager()
        .init(req.remote_url.clone())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    use crate::sync::InitResult;
    let (status_code, message) = match result {
        InitResult::Created => (StatusCode::CREATED, "Sync initialized successfully"),
        InitResult::AlreadyInitialized => (StatusCode::OK, "Sync already initialized"),
    };

    Ok((
        status_code,
        Json(SyncResponse {
            status: "success".to_string(),
            message: message.to_string(),
            data: Some(serde_json::json!({
                "sync_dir": crate::sync::get_sync_dir().display().to_string(),
                "remote_url": req.remote_url,
                "created": matches!(result, InitResult::Created),
            })),
        }),
    ))
}

/// Export database to sync
#[utoipa::path(
    post,
    path = "/api/v1/sync/export",
    tag = "sync",
    request_body = ExportSyncRequest,
    responses(
        (status = 200, description = "Export completed successfully", body = SyncResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
pub async fn export_sync<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Json(req): Json<ExportSyncRequest>,
) -> Result<Json<SyncResponse>, (StatusCode, Json<ErrorResponse>)> {
    let summary = state
        .sync_manager()
        .export(state.db(), req.message, req.remote)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(SyncResponse {
        status: "success".to_string(),
        message: "Export completed".to_string(),
        data: Some(serde_json::json!({
            "exported": {
                "repos": summary.repos,
                "projects": summary.projects,
                "task_lists": summary.task_lists,
                "tasks": summary.tasks,
                "notes": summary.notes,
                "total": summary.total(),
            }
        })),
    }))
}

/// Import sync data to database
#[utoipa::path(
    post,
    path = "/api/v1/sync/import",
    tag = "sync",
    request_body = ImportSyncRequest,
    responses(
        (status = 200, description = "Import completed successfully", body = SyncResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
pub async fn import_sync<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Json(req): Json<ImportSyncRequest>,
) -> Result<Json<SyncResponse>, (StatusCode, Json<ErrorResponse>)> {
    let summary = state
        .sync_manager()
        .import(state.db(), req.remote)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(SyncResponse {
        status: "success".to_string(),
        message: "Import completed".to_string(),
        data: Some(serde_json::json!({
            "imported": {
                "repos": summary.repos,
                "projects": summary.projects,
                "task_lists": summary.task_lists,
                "tasks": summary.tasks,
                "notes": summary.notes,
                "total": summary.total(),
            }
        })),
    }))
}

/// Get sync status
#[utoipa::path(
    get,
    path = "/api/v1/sync/status",
    tag = "sync",
    responses(
        (status = 200, description = "Sync status retrieved", body = SyncResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
pub async fn get_sync_status<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
) -> Result<Json<SyncResponse>, (StatusCode, Json<ErrorResponse>)> {
    let status = state.sync_manager().status(state.db()).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    if !status.initialized {
        return Ok(Json(SyncResponse {
            status: "not_initialized".to_string(),
            message: "Sync not initialized. Run init operation first.".to_string(),
            data: Some(serde_json::json!({
                "initialized": false,
            })),
        }));
    }

    let data = serde_json::json!({
        "initialized": true,
        "remote_url": status.remote_url,
        "git": status.git_status.as_ref().map(|git_status| serde_json::json!({
            "clean": git_status.clean,
            "status": if git_status.clean { "No changes" } else { &git_status.status_output },
        })),
        "database": status.db_counts.as_ref().map(|counts| serde_json::json!({
            "repos": counts.repos,
            "projects": counts.projects,
            "task_lists": counts.task_lists,
            "tasks": counts.tasks,
            "notes": counts.notes,
            "total": counts.total(),
        })),
        "sync_files": status.jsonl_counts.as_ref().map(|counts| serde_json::json!({
            "repos": counts.repos,
            "projects": counts.projects,
            "task_lists": counts.task_lists,
            "tasks": counts.tasks,
            "notes": counts.notes,
            "total": counts.total(),
        })),
    });

    Ok(Json(SyncResponse {
        status: "success".to_string(),
        message: "Sync status retrieved".to_string(),
        data: Some(data),
    }))
}
