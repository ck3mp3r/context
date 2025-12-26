//! API server module.
//!
//! Provides REST API endpoints for managing context data.

mod handlers;
mod routes;
mod state;

use std::net::IpAddr;

use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub use state::AppState;

use crate::db::Database;

/// API server configuration
pub struct Config {
    /// Host address to bind to
    pub host: IpAddr,
    /// Port to listen on
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".parse().unwrap(),
            port: 3000,
        }
    }
}

/// Initialize tracing subscriber with env filter
fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "context=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Run the API server with the given configuration and database.
///
/// The caller is responsible for creating and migrating the database.
/// This keeps the API layer agnostic of the concrete database implementation.
pub async fn run<D: Database + 'static>(
    config: Config,
    db: D,
) -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    // Create application state
    let state = AppState::new(db);

    let app = routes::create_router(state).layer(TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("API server listening on http://{}", addr);
    info!("API docs available at http://{}/docs", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
