//! System health and status handlers.

use axum::Json;
use serde::Serialize;
use tracing::instrument;
use utoipa::ToSchema;

/// Health check response
#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    /// Service status
    #[schema(example = "ok")]
    pub status: String,
}

/// Root endpoint
///
/// Returns the API name
#[utoipa::path(
    get,
    path = "/",
    tag = "system",
    responses(
        (status = 200, description = "API name", body = String)
    )
)]
#[instrument]
pub async fn root() -> &'static str {
    "c5t-api"
}

/// Health check endpoint
///
/// Returns the current health status of the API
#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    responses(
        (status = 200, description = "Health check successful", body = HealthResponse)
    )
)]
#[instrument]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}
