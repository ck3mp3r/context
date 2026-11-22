//! Tauri desktop shell for c5t.
//!
//! Runs the REST and MCP server in-process for the current build profile, then
//! loads the SPA that talks to it. When a server already answers the health
//! endpoint, the app attaches to it instead of starting a second one.

use std::net::IpAddr;
use std::time::Duration;

use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use context_core::{Database, get_db_path};
use context_db::SqliteDatabase;
use context_server::api::{self, Config};

#[cfg(test)]
#[path = "../test/endpoint.rs"]
mod endpoint_test;

/// Dev server port. `trunk serve` proxies `/dev` and `/dev/ws` here.
const DEBUG_API_PORT: u16 = 3738;
/// Production port. MCP clients are configured for `localhost:3737/mcp`.
const RELEASE_API_PORT: u16 = 3737;
/// Debug SPA origin, served by `trunk serve` on 8080.
const DEBUG_SPA_ORIGIN: &str = "http://127.0.0.1:8080/";
/// Release SPA origin, served by the in-process server.
const RELEASE_SPA_ORIGIN: &str = "http://127.0.0.1:3737/";

/// How long to wait for the server to answer `/health` before giving up.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);
/// Delay between health probes while waiting for the server.
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(200);
/// Timeout of a single health probe.
const HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

fn main() -> tauri::Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let port = api_port();
            let health_url = health_url(port);

            if let Err(message) = tauri::async_runtime::block_on(start_server(port, &health_url)) {
                eprintln!("context: {message}");
                app.dialog()
                    .message(message)
                    .title("context")
                    .kind(MessageDialogKind::Error)
                    .blocking_show();
                std::process::exit(1);
            }

            let origin: tauri::Url = spa_origin().parse()?;
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(origin))
                .title("context")
                .inner_size(1280.0, 800.0)
                .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
}

/// Port selected by the build profile.
///
/// Uses `cfg!(debug_assertions)` — the same predicate the data directory uses —
/// so the app attaches to a server of its own profile and opens its own
/// database. A debug build never touches the production port.
fn api_port() -> u16 {
    if cfg!(debug_assertions) {
        DEBUG_API_PORT
    } else {
        RELEASE_API_PORT
    }
}

/// SPA origin selected by the build profile.
fn spa_origin() -> &'static str {
    if cfg!(debug_assertions) {
        DEBUG_SPA_ORIGIN
    } else {
        RELEASE_SPA_ORIGIN
    }
}

/// Loopback health endpoint of the server on `port`.
fn health_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/health")
}

/// Build the client used for health probes.
fn health_client() -> Result<reqwest::Client, String> {
    // Install ring as the default crypto provider for rustls. Workspace builds
    // enable rustls without a provider, and building a client then panics.
    let _ = rustls::crypto::ring::default_provider().install_default();

    reqwest::Client::builder()
        .timeout(HEALTH_REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("Failed to create the health probe client: {error}"))
}

/// Attach to a running server, or start one and wait until it is healthy.
///
/// Returns an error message for the operator when the server cannot be
/// reached and cannot be started.
async fn start_server(port: u16, health_url: &str) -> Result<(), String> {
    let client = health_client()?;

    if probe_health(&client, health_url).await {
        return Ok(());
    }

    let db_path = get_db_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let db = SqliteDatabase::open(&db_path)
        .await
        .map_err(|error| format!("Failed to open {}: {error}", db_path.display()))?;
    db.migrate()
        .map_err(|error| format!("Failed to migrate {}: {error}", db_path.display()))?;

    let config = Config {
        host: IpAddr::from([127, 0, 0, 1]),
        port,
        ..Config::new()
    };

    tauri::async_runtime::spawn(async move {
        if let Err(error) = api::run(config, db).await {
            eprintln!("context: API server stopped: {error}");
        }
    });

    let deadline = tokio::time::Instant::now() + HEALTH_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        if probe_health(&client, health_url).await {
            return Ok(());
        }
        tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
    }

    Err(format!(
        "The context server did not answer {health_url} within {} seconds.",
        HEALTH_TIMEOUT.as_secs()
    ))
}

/// Report whether `url` answers HTTP 200.
async fn probe_health(client: &reqwest::Client, url: &str) -> bool {
    matches!(
        client.get(url).send().await,
        Ok(response) if response.status() == reqwest::StatusCode::OK
    )
}
