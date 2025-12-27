//! API server module.
//!
//! Provides REST API endpoints for managing context data.

mod handlers;
pub(crate) mod routes;
mod state;
pub mod v1;

use std::net::IpAddr;

use miette::Diagnostic;
use thiserror::Error;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub use state::AppState;

use crate::db::Database;

/// API server errors.
#[derive(Error, Diagnostic, Debug)]
pub enum ApiError {
    #[error("Failed to bind to address {addr}: {source}")]
    #[diagnostic(code(c5t::api::bind_failed))]
    BindFailed {
        addr: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Server error: {0}")]
    #[diagnostic(code(c5t::api::server_error))]
    ServerError(#[from] std::io::Error),
}

/// API server configuration
pub struct Config {
    /// Host address to bind to
    pub host: IpAddr,
    /// Port to listen on
    pub port: u16,
    /// Logging verbosity (0=warn, 1=info, 2=debug, 3=trace)
    pub verbosity: u8,
    /// Enable OpenAPI documentation endpoint at /docs
    pub enable_docs: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".parse().unwrap(),
            port: 3000,
            verbosity: 0,
            enable_docs: false,
        }
    }
}

/// Initialize tracing subscriber with verbosity level
fn init_tracing(verbosity: u8) {
    let level = match verbosity {
        0 => "context=warn,tower_http=warn",
        1 => "context=info,tower_http=info",
        2 => "context=debug,tower_http=debug",
        _ => "context=trace,tower_http=trace",
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| level.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Run the API server with the given configuration and database.
///
/// The caller is responsible for creating and migrating the database.
/// This keeps the API layer agnostic of the concrete database implementation.
pub async fn run<D: Database + 'static>(config: Config, db: D) -> Result<(), ApiError> {
    init_tracing(config.verbosity);

    // Create application state
    let state = AppState::new(db);

    let app = routes::create_router(state, config.enable_docs).layer(TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.host, config.port);
    let listener =
        tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| ApiError::BindFailed {
                addr: addr.clone(),
                source: e,
            })?;
    info!("API server listening on http://{}", addr);
    info!("API docs available at http://{}/docs", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
