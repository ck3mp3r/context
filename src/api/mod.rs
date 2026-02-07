//! API server module.
//!
//! Provides REST API endpoints for managing context data.

mod handlers;
#[cfg(test)]
mod mod_test;
pub mod notifier;
#[cfg(test)]
mod notifier_test;
pub(crate) mod routes;
mod state;
pub mod static_assets;
#[cfg(test)]
mod static_assets_test;
pub mod v1;
mod websocket;
#[cfg(test)]
mod websocket_test;

use std::net::IpAddr;
use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub use state::AppState;

use crate::db::Database;
use crate::sync::get_data_dir;

#[cfg(debug_assertions)]
const DEFAULT_API_PORT: u16 = 3738;

#[cfg(not(debug_assertions))]
const DEFAULT_API_PORT: u16 = 3737;

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
    /// Skills cache directory (where attachments are extracted)
    pub skills_dir: PathBuf,
}

impl Config {
    /// Create new Config reading from environment variables
    pub fn new() -> Self {
        Self {
            host: "0.0.0.0".parse().unwrap(),
            port: DEFAULT_API_PORT,
            verbosity: 0,
            enable_docs: false,
            skills_dir: match std::env::var("C5T_SKILLS_DIR") {
                Ok(dir) => PathBuf::from(dir),
                Err(_) => get_data_dir().join("skills"),
            },
        }
    }

    /// Builder method to override skills_dir (CLI flag > env var)
    pub fn with_skills_dir(mut self, skills_dir: PathBuf) -> Self {
        self.skills_dir = skills_dir;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".parse().unwrap(),
            port: DEFAULT_API_PORT,
            verbosity: 0,
            enable_docs: false,
            skills_dir: get_data_dir().join("skills"),
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

    // Create sync manager (uses RealGit for production)
    let sync_manager = crate::sync::SyncManager::new(crate::sync::RealGit::new());

    // Create change notifier for WebSocket pub/sub
    let notifier = notifier::ChangeNotifier::new();

    // Create application state
    let state = AppState::new(db, sync_manager, notifier, config.skills_dir);

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
