//! Update status and trigger endpoints.
//!
//! `GET /api/v1/update/status` reports the persisted update state. The POST
//! endpoints forward to the injected flow and return at once: the flow runs the
//! work on its own task and owns every `/ws` broadcast, so these handlers never
//! notify.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use context_core::Database;
use context_core::update::{UpdateFlow, UpdateState, UpdateStateError, UpdateStatus};
use context_sync::GitOps;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utoipa::ToSchema;

use crate::api::AppState;

use super::ErrorResponse;

// =============================================================================
// DTOs
// =============================================================================

/// Persisted update state.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateStatusResponse {
    /// Version this build runs.
    #[schema(example = "0.8.0")]
    pub current_version: String,
    /// Phase of the update flow.
    #[schema(value_type = String, example = "idle")]
    pub status: UpdateStatus,
    /// Version of the candidate awaiting consent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version: Option<String>,
    /// Release tag the candidate came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_tag: Option<String>,
    /// Release notes of the candidate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_notes: Option<String>,
    /// Whether the candidate is a pre-release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_is_prerelease: Option<bool>,
    /// Detail for the current status, such as a failure reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// RFC 3339 timestamp of the last state change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Body of `POST /api/v1/update/check`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct UpdateCheckRequest {
    /// Whether the user asked for this check.
    #[serde(default)]
    pub manual: bool,
}

// =============================================================================
// Conversions
// =============================================================================

impl From<UpdateState> for UpdateStatusResponse {
    fn from(state: UpdateState) -> Self {
        Self {
            current_version: state.current_version,
            status: state.status,
            candidate_version: state.candidate_version,
            candidate_tag: state.candidate_tag,
            candidate_notes: state.candidate_notes,
            candidate_is_prerelease: state.candidate_is_prerelease,
            message: state.message,
            updated_at: state.updated_at,
        }
    }
}

// =============================================================================
// Handlers
// =============================================================================

/// Report the persisted update state.
///
/// Always answers with a state: an absent, unreadable, or unparsable file reports
/// the default state, so a polling client never sees an update error.
#[utoipa::path(
    get,
    path = "/api/v1/update/status",
    tag = "updates",
    responses(
        (status = 200, description = "Persisted update state; an absent file reports the default state", body = UpdateStatusResponse)
    )
)]
#[instrument(skip(state))]
pub async fn get_update_status<D: Database, G: GitOps + Send + Sync, U: UpdateFlow>(
    State(state): State<AppState<D, G, U>>,
) -> Json<UpdateStatusResponse> {
    Json(UpdateStatusResponse::from(state.update().state()))
}

/// Start an update check.
#[utoipa::path(
    post,
    path = "/api/v1/update/check",
    tag = "updates",
    request_body = UpdateCheckRequest,
    responses(
        (status = 202, description = "Check started; progress arrives through /ws"),
        (status = 409, description = "A check or install already runs", body = ErrorResponse),
        (status = 503, description = "No update flow is attached", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn check_for_update<D: Database, G: GitOps + Send + Sync, U: UpdateFlow>(
    State(state): State<AppState<D, G, U>>,
    Json(request): Json<UpdateCheckRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .update()
        .check(request.manual)
        .map_err(trigger_error)?;

    Ok(StatusCode::ACCEPTED)
}

/// Start installing the candidate that waits for consent.
#[utoipa::path(
    post,
    path = "/api/v1/update/install",
    tag = "updates",
    responses(
        (status = 202, description = "Install started; progress arrives through /ws"),
        (status = 409, description = "A check or install already runs", body = ErrorResponse),
        (status = 503, description = "No update flow is attached", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn install_update<D: Database, G: GitOps + Send + Sync, U: UpdateFlow>(
    State(state): State<AppState<D, G, U>>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state.update().install().map_err(trigger_error)?;

    Ok(StatusCode::ACCEPTED)
}

/// Restart onto the bundle an install staged.
///
/// The flow spawns the swapped service, respawns the GUI and exits the old
/// process. Nothing here touches launchd.
#[utoipa::path(
    post,
    path = "/api/v1/update/restart",
    tag = "updates",
    responses(
        (status = 202, description = "Restart started; the GUI reopens and the new service takes over"),
        (status = 409, description = "No installed update waits, or a run is in flight", body = ErrorResponse),
        (status = 503, description = "No update flow is attached", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn restart_update<D: Database, G: GitOps + Send + Sync, U: UpdateFlow>(
    State(state): State<AppState<D, G, U>>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state.update().restart().map_err(trigger_error)?;

    Ok(StatusCode::ACCEPTED)
}

// =============================================================================
// Support
// =============================================================================

/// Map a flow rejection to its HTTP status.
fn trigger_error(error: UpdateStateError) -> (StatusCode, Json<ErrorResponse>) {
    let status = match &error {
        UpdateStateError::Busy | UpdateStateError::Invalid { .. } => StatusCode::CONFLICT,
        UpdateStateError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        UpdateStateError::Io { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    };

    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}
