use axum::Json;
use serde::Serialize;
use tracing::instrument;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

#[instrument]
pub async fn root() -> &'static str {
    crate::hello()
}

#[instrument]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}
