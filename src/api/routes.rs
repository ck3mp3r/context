use axum::{Router, routing::get};

use super::handlers;

pub fn create_router() -> Router {
    Router::new()
        .route("/", get(handlers::root))
        .route("/health", get(handlers::health))
}
