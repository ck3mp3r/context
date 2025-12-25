mod handlers;
mod routes;

use std::net::IpAddr;

use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

/// Run the API server with the given configuration
pub async fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let app = routes::create_router().layer(TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("API server listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
