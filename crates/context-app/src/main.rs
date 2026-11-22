//! Tauri desktop shell for c5t.
//!
//! Runs the REST and MCP server in-process for the current build profile, then
//! loads the SPA that talks to it. When a server already answers the health
//! endpoint, the app attaches to it instead of starting a second one.

use std::net::IpAddr;
use std::time::Duration;

use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use context_core::{Database, get_db_path};
use context_db::SqliteDatabase;
use context_server::api::{self, Config};

mod lifecycle;
mod menu;
mod tray;

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
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            lifecycle::request_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .on_menu_event(|app, event| {
            if event.id() == menu::QUIT_ITEM_ID {
                app.exit(0);
            } else if event.id() == tray::SHOW_ITEM_ID {
                lifecycle::request_main_window(app);
            }
        })
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

            lifecycle::create_main_window(app.handle())?;
            menu::build(app.handle())?;
            tray::build(app.handle())?;

            Ok(())
        })
        .build(tauri::generate_context!())?;

    app.run(lifecycle::handle_run_event);

    Ok(())
}

/// Port selected by the build profile.
///
/// Uses `cfg!(debug_assertions)` — the same predicate the data directory uses —
/// so the app attaches to a server of its own profile and opens its own
/// database. A debug build never touches the production port.
pub(crate) fn api_port() -> u16 {
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

    let (failure_tx, mut failure_rx) = tokio::sync::mpsc::unbounded_channel();
    tauri::async_runtime::spawn(async move {
        let failure = match api::run(config, db).await {
            Ok(()) => "The context server stopped before it became ready.".to_string(),
            Err(error) => {
                eprintln!("context: API server stopped: {error}");
                error.to_string()
            }
        };
        let _ = failure_tx.send(failure);
    });

    await_health(&client, health_url, HEALTH_TIMEOUT, &mut failure_rx).await
}

/// Wait for the server to answer `/health`, or report why it never will.
///
/// Stops early with the server's own error, so the operator sees the real
/// cause — a bind failure, for example — instead of a timeout message.
async fn await_health(
    client: &reqwest::Client,
    health_url: &str,
    timeout: Duration,
    failure_rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if probe_health(client, health_url).await {
            return Ok(());
        }

        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        tokio::select! {
            failure = failure_rx.recv() => {
                let cause = failure
                    .unwrap_or_else(|| "The context server task ended unexpectedly.".to_string());
                return Err(format!("Failed to start the context server: {cause}"));
            }
            () = tokio::time::sleep(HEALTH_POLL_INTERVAL.min(remaining)) => {}
        }
    }

    Err(format!(
        "The context server did not answer {health_url} within {} seconds.",
        timeout.as_secs_f64()
    ))
}

/// Report whether `url` answers HTTP 200.
async fn probe_health(client: &reqwest::Client, url: &str) -> bool {
    matches!(
        client.get(url).send().await,
        Ok(response) if response.status() == reqwest::StatusCode::OK
    )
}
